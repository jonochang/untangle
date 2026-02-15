use crate::config::resolve::{resolve_config, CliOverrides};
use crate::errors::{Result, UntangleError};
use crate::graph::builder::{GraphBuilder, ResolvedImport};
use crate::metrics::scc::find_non_trivial_sccs;
use crate::metrics::summary::Summary;
use crate::output::json::{LanguageStats, Metadata};
use crate::output::OutputFormat;
use crate::parse::common::{ImportConfidence, RawImport, SourceLocation};
use crate::parse::factory;
use crate::parse::go::GoFrontend;
use crate::parse::rust::RustFrontend;
use crate::parse::ParseFrontend;
use crate::walk::{self, Language};
use clap::Args;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

#[derive(Debug, Args)]
pub struct AnalyzeArgs {
    /// Path to analyze
    pub path: PathBuf,

    /// Language to analyze
    #[arg(long, value_parser = parse_language)]
    pub lang: Option<Language>,

    /// Output format
    #[arg(long)]
    pub format: Option<OutputFormat>,

    /// Number of top hotspots to report
    #[arg(long)]
    pub top: Option<usize>,

    /// Fan-out threshold for reporting
    #[arg(long)]
    pub threshold_fanout: Option<usize>,

    /// Fan-out threshold for SCC size
    #[arg(long)]
    pub threshold_scc: Option<usize>,

    /// Include test files (Go: *_test.go)
    #[arg(long)]
    pub include_tests: bool,

    /// Include glob patterns
    #[arg(long)]
    pub include: Vec<String>,

    /// Exclude glob patterns
    #[arg(long)]
    pub exclude: Vec<String>,

    /// Suppress progress output
    #[arg(long)]
    pub quiet: bool,

    /// Suppress insights from output
    #[arg(long)]
    pub no_insights: bool,
}

impl AnalyzeArgs {
    fn to_cli_overrides(&self) -> CliOverrides {
        CliOverrides {
            lang: self.lang,
            format: self.format,
            quiet: self.quiet,
            top: self.top,
            include_tests: self.include_tests,
            no_insights: self.no_insights,
            include: self.include.clone(),
            exclude: self.exclude.clone(),
            threshold_fanout: self.threshold_fanout,
            threshold_scc: self.threshold_scc,
            ..Default::default()
        }
    }
}

fn parse_language(s: &str) -> std::result::Result<Language, String> {
    s.parse()
}

/// Result of parsing a single file (collected from parallel workers).
struct FileParseResult {
    source_module: PathBuf,
    imports: Vec<RawImport>,
    language: Language,
}

pub fn run(args: &AnalyzeArgs) -> Result<()> {
    let start = Instant::now();

    let root = args
        .path
        .canonicalize()
        .map_err(|_| UntangleError::NoFiles {
            path: args.path.clone(),
        })?;

    // Resolve config
    let overrides = args.to_cli_overrides();
    let config = resolve_config(&root, &overrides)?;

    // Determine format from resolved config
    let format: OutputFormat = config.format.parse().unwrap_or_default();

    // Merge ignore_patterns into exclude list
    let mut exclude = config.exclude.clone();
    exclude.extend(config.ignore_patterns.iter().cloned());

    // Determine languages and discover files
    let (langs, files_by_lang): (Vec<Language>, HashMap<Language, Vec<PathBuf>>) = match config.lang
    {
        Some(l) => {
            // Single-language mode
            let files =
                walk::discover_files(&root, l, &config.include, &exclude, config.include_tests)?;
            let mut map = HashMap::new();
            if !files.is_empty() {
                map.insert(l, files);
            }
            (vec![l], map)
        }
        None => {
            // Multi-language mode: detect all languages
            let map =
                walk::discover_files_multi(&root, &config.include, &exclude, config.include_tests)?;
            let mut langs: Vec<Language> = map.keys().copied().collect();
            // Sort by file count descending for deterministic output
            langs.sort_by(|a, b| {
                map.get(b)
                    .map(|v| v.len())
                    .unwrap_or(0)
                    .cmp(&map.get(a).map(|v| v.len()).unwrap_or(0))
            });
            (langs, map)
        }
    };

    // Flatten all files for processing
    let all_files: Vec<(Language, PathBuf)> = langs
        .iter()
        .flat_map(|&lang| {
            files_by_lang
                .get(&lang)
                .map(|files| files.iter().map(move |f| (lang, f.clone())))
                .into_iter()
                .flatten()
        })
        .collect();

    if all_files.is_empty() {
        return Err(UntangleError::NoFiles { path: root });
    }

    // Read go.mod for Go projects (needed by all parser instances)
    let go_module_path = if langs.contains(&Language::Go) {
        GoFrontend::read_go_mod(&root)
    } else {
        None
    };

    // Read Cargo.toml for Rust projects
    let rust_crate_name = if langs.contains(&Language::Rust) {
        RustFrontend::read_cargo_toml(&root)
    } else {
        None
    };

    let files_skipped = AtomicUsize::new(0);

    // Track per-language file counts
    let per_lang_files_parsed: HashMap<Language, usize> = files_by_lang
        .iter()
        .map(|(&lang, files)| (lang, files.len()))
        .collect();

    // Progress bar for parsing phase
    let progress = if !config.quiet {
        let pb = indicatif::ProgressBar::new(all_files.len() as u64);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} files ({eta})")
                .unwrap()
                .progress_chars("#>-"),
        );
        Some(pb)
    } else {
        None
    };

    // Parallel parse: each thread creates its own Parser (Parser is not Send)
    let parse_results: Vec<FileParseResult> = all_files
        .par_iter()
        .filter_map(|(lang, file_path)| {
            let source = match std::fs::read(file_path) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Skipping {}: {}", file_path.display(), e);
                    files_skipped.fetch_add(1, Ordering::Relaxed);
                    return None;
                }
            };

            // Each thread creates its own frontend (which creates its own Parser)
            let frontend: Box<dyn ParseFrontend> =
                factory::create_frontend(*lang, &config, &go_module_path, &rust_crate_name);

            let imports = frontend.extract_imports(&source, file_path);
            let source_module = factory::source_module_path(file_path, &root, *lang);

            if let Some(ref pb) = progress {
                pb.inc(1);
            }

            Some(FileParseResult {
                source_module,
                imports,
                language: *lang,
            })
        })
        .collect();

    if let Some(pb) = progress {
        pb.finish_and_clear();
    }

    let files_parsed = parse_results.len();
    let files_skipped = files_skipped.load(Ordering::Relaxed);

    // Sequential graph building (not parallelizable due to shared mutable state)
    let mut builder = GraphBuilder::new();
    let mut unresolved_imports = 0usize;

    // Create resolvers per language
    let resolvers: HashMap<Language, Box<dyn ParseFrontend>> = langs
        .iter()
        .map(|&lang| {
            let frontend =
                factory::create_frontend(lang, &config, &go_module_path, &rust_crate_name);
            (lang, frontend)
        })
        .collect();

    // Build per-language file lists for resolution
    let files_by_lang_for_resolve: HashMap<Language, Vec<PathBuf>> = files_by_lang
        .iter()
        .map(|(&lang, files)| (lang, files.clone()))
        .collect();

    for result in &parse_results {
        let resolver = match resolvers.get(&result.language) {
            Some(r) => r,
            None => continue,
        };
        let lang_files = files_by_lang_for_resolve
            .get(&result.language)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        for raw in &result.imports {
            if raw.confidence == ImportConfidence::External
                || raw.confidence == ImportConfidence::Dynamic
                || raw.confidence == ImportConfidence::Unresolvable
            {
                unresolved_imports += 1;
                continue;
            }

            match resolver.resolve(raw, &root, lang_files) {
                Some(target_module) => {
                    builder.add_import(&ResolvedImport {
                        source_module: result.source_module.clone(),
                        target_module,
                        location: SourceLocation {
                            file: result.source_module.clone(),
                            line: raw.line,
                            column: raw.column,
                        },
                        language: Some(result.language),
                    });
                }
                None => {
                    unresolved_imports += 1;
                }
            }
        }
    }

    let graph = builder.build();
    let summary = Summary::from_graph(&graph);
    let sccs = find_non_trivial_sccs(&graph);

    let insights = if config.no_insights {
        None
    } else {
        Some(crate::insights::generate_insights_with_config(
            &graph,
            &summary,
            &sccs,
            &config.rules,
            &config.overrides,
        ))
    };

    let elapsed = start.elapsed();
    let elapsed_ms = elapsed.as_millis() as u64;
    let node_count = graph.node_count();
    let edge_count = graph.edge_count();
    let modules_per_second = if elapsed_ms > 0 {
        node_count as f64 / (elapsed_ms as f64 / 1000.0)
    } else {
        0.0
    };

    let edge_density = if node_count > 1 {
        edge_count as f64 / (node_count as f64 * (node_count as f64 - 1.0))
    } else {
        0.0
    };

    // Build language string and per-language stats
    let language_str = if langs.len() == 1 {
        langs[0].to_string()
    } else {
        langs
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join(",")
    };

    // Count nodes per language from the graph
    let mut nodes_per_lang: HashMap<Language, usize> = HashMap::new();
    for idx in graph.node_indices() {
        if let Some(lang) = graph[idx].language {
            *nodes_per_lang.entry(lang).or_insert(0) += 1;
        }
    }

    let languages = if langs.len() > 1 {
        Some(
            langs
                .iter()
                .map(|&lang| LanguageStats {
                    language: lang.to_string(),
                    files_parsed: per_lang_files_parsed.get(&lang).copied().unwrap_or(0),
                    nodes: nodes_per_lang.get(&lang).copied().unwrap_or(0),
                })
                .collect(),
        )
    } else {
        None
    };

    let metadata = Metadata {
        language: language_str,
        granularity: "module".to_string(),
        root: root.clone(),
        node_count,
        edge_count,
        edge_density: (edge_density * 10000.0).round() / 10000.0,
        files_parsed,
        files_skipped,
        unresolved_imports,
        timestamp: chrono_now(),
        elapsed_ms,
        modules_per_second: (modules_per_second * 10.0).round() / 10.0,
        languages,
    };

    let mut stdout = std::io::stdout();

    match format {
        OutputFormat::Json => {
            crate::output::json::write_analyze_json(
                &mut stdout,
                &graph,
                &summary,
                &sccs,
                metadata,
                config.top,
                insights,
            )?;
        }
        OutputFormat::Text => {
            crate::output::text::write_analyze_text(
                &mut stdout,
                &graph,
                &summary,
                &sccs,
                &metadata,
                config.top,
                insights.as_deref(),
            )?;
        }
        OutputFormat::Dot => {
            crate::output::dot::write_dot(&mut stdout, &graph)?;
            if !config.quiet {
                eprintln!(
                    "Completed in {:.2}s ({:.0} modules/sec)",
                    elapsed_ms as f64 / 1000.0,
                    modules_per_second
                );
            }
        }
        OutputFormat::Sarif => {
            crate::output::sarif::write_sarif(
                &mut stdout,
                &graph,
                &sccs,
                &metadata,
                args.threshold_fanout,
            )?;
        }
    }

    if !config.quiet && format != OutputFormat::Dot {
        eprintln!(
            "Analyzed {} modules ({} edges) in {:.2}s ({:.0} modules/sec)",
            node_count,
            edge_count,
            elapsed_ms as f64 / 1000.0,
            modules_per_second
        );
    }

    Ok(())
}

fn chrono_now() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Civil date algorithm from UNIX timestamp (no external dependency)
    let days = (secs / 86400) as i64;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;
    // Days since 0000-03-01 (shifted epoch for leap year handling)
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}
