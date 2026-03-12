use crate::analysis_context::{build_analysis_context, canonicalize_root, resolve_project_root};
use crate::config::resolve::{resolve_config, CliOverrides};
use crate::config::ResolvedConfig;
use crate::errors::Result;
use crate::formats::AnalyzeReportFormat;
use crate::graph::builder::{GraphBuilder, ResolvedImport};
use crate::graph::ir::DepGraph;
use crate::insights::Insight;
use crate::metrics::scc::find_non_trivial_sccs;
use crate::metrics::scc::SccInfo;
use crate::metrics::summary::Summary;
use crate::output::json::{LanguageStats, Metadata};
use crate::parse::common::{ImportConfidence, RawImport, SourceLocation};
use crate::parse::factory;
use crate::parse::go::GoFrontend;
use crate::parse::ParseFrontend;
use crate::walk::{self, Language};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

pub struct AnalysisReportRequest {
    pub path: PathBuf,
    pub lang: Option<Language>,
    pub quiet: bool,
    pub include_tests: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub format: Option<AnalyzeReportFormat>,
    pub top: Option<usize>,
    pub threshold_fanout: Option<usize>,
    pub threshold_scc: Option<usize>,
    pub insights_disabled: bool,
}

struct FileParseResult {
    source_module: PathBuf,
    imports: Vec<RawImport>,
    language: Language,
    original_file: PathBuf,
}

pub struct AnalysisSnapshot {
    pub graph: DepGraph,
    pub summary: Summary,
    pub sccs: Vec<SccInfo>,
    pub metadata: Metadata,
    pub insights: Option<Vec<Insight>>,
}

pub fn run_report(request: AnalysisReportRequest) -> Result<()> {
    let scan_root = canonicalize_root(&request.path)?;
    let project_root = resolve_project_root(&scan_root, request.lang);
    let config = resolve_config(
        &project_root,
        &CliOverrides {
            lang: request.lang,
            quiet: request.quiet,
            include_tests: request.include_tests,
            include: request.include,
            exclude: request.exclude,
            fail_on: Vec::new(),
            threshold_fanout: request.threshold_fanout,
            threshold_scc: request.threshold_scc,
        },
    )?;
    let context = build_analysis_context(&scan_root, &project_root, &config)?;
    let format = request.format.unwrap_or(config.analyze_report.format);
    let snapshot = build_analysis_snapshot(
        &scan_root,
        &context.project_root,
        &config,
        request.insights_disabled,
    )?;

    let mut stdout = std::io::stdout();
    let top = request.top.or(config.analyze_report.top);
    match format {
        AnalyzeReportFormat::Json => crate::output::json::write_analyze_json(
            &mut stdout,
            &snapshot.graph,
            &snapshot.summary,
            &snapshot.sccs,
            snapshot.metadata.clone(),
            top,
            snapshot.insights.clone(),
        )?,
        AnalyzeReportFormat::Text => crate::output::text::write_analyze_text(
            &mut stdout,
            &snapshot.graph,
            &snapshot.summary,
            &snapshot.sccs,
            &snapshot.metadata,
            top,
            snapshot.insights.as_deref(),
        )?,
        AnalyzeReportFormat::Sarif => crate::output::sarif::write_sarif(
            &mut stdout,
            &snapshot.graph,
            &snapshot.sccs,
            &snapshot.metadata,
            request
                .threshold_fanout
                .or(config.analyze_report.threshold_fanout),
        )?,
    }

    Ok(())
}

pub fn build_analysis_snapshot(
    scan_root: &std::path::Path,
    project_root: &std::path::Path,
    config: &ResolvedConfig,
    insights_disabled: bool,
) -> Result<AnalysisSnapshot> {
    let start = Instant::now();
    let context = build_analysis_context(scan_root, project_root, config)?;
    let files_skipped = AtomicUsize::new(0);
    let per_lang_files_parsed: HashMap<Language, usize> = context
        .files_by_lang
        .iter()
        .map(|(&lang, files)| (lang, files.len()))
        .collect();

    let progress = if !config.quiet {
        let progress = indicatif::ProgressBar::new(context.all_files.len() as u64);
        progress.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} files ({eta})")
                .unwrap()
                .progress_chars("#>-"),
        );
        Some(progress)
    } else {
        None
    };

    let parse_results: Vec<FileParseResult> = context
        .all_files
        .par_iter()
        .filter_map(|(lang, file_path)| {
            let source = match std::fs::read(file_path) {
                Ok(source) => source,
                Err(error) => {
                    tracing::warn!("Skipping {}: {}", file_path.display(), error);
                    files_skipped.fetch_add(1, Ordering::Relaxed);
                    return None;
                }
            };

            let file_go_module = if *lang == Language::Go {
                walk::find_go_module_root(file_path, &context.go_modules)
                    .map(|(_, module_path)| module_path.to_string())
                    .or_else(|| context.go_module_path.clone())
            } else {
                context.go_module_path.clone()
            };

            let frontend =
                factory::create_frontend(*lang, config, &file_go_module, &context.rust_crate_name);
            let imports = frontend.extract_imports(&source, file_path);
            let source_module =
                factory::source_module_path(file_path, &context.project_root, *lang);

            if let Some(ref progress) = progress {
                progress.inc(1);
            }

            Some(FileParseResult {
                source_module,
                imports,
                language: *lang,
                original_file: file_path.clone(),
            })
        })
        .collect();

    if let Some(progress) = progress {
        progress.finish_and_clear();
    }

    let files_parsed = parse_results.len();
    let files_skipped = files_skipped.load(Ordering::Relaxed);

    let resolvers: HashMap<Language, Box<dyn ParseFrontend>> = context
        .langs
        .iter()
        .filter(|&&lang| lang != Language::Go)
        .map(|&lang| {
            let frontend = factory::create_frontend(
                lang,
                config,
                &context.go_module_path,
                &context.rust_crate_name,
            );
            (lang, frontend)
        })
        .collect();
    let go_resolvers: HashMap<PathBuf, Box<dyn ParseFrontend>> = context
        .go_modules
        .iter()
        .map(|(mod_root, mod_path)| {
            let frontend = GoFrontend::with_module_path(mod_path.clone())
                .with_exclude_stdlib(config.go.exclude_stdlib);
            (
                mod_root.clone(),
                Box::new(frontend) as Box<dyn ParseFrontend>,
            )
        })
        .collect();
    let fallback_go_resolver = factory::create_frontend(
        Language::Go,
        config,
        &context.go_module_path,
        &context.rust_crate_name,
    );
    let go_files_by_module = group_go_files_by_module(&context);
    let files_by_lang_for_resolve: HashMap<Language, Vec<PathBuf>> = context
        .files_by_lang
        .iter()
        .map(|(&lang, files)| (lang, files.clone()))
        .collect();

    let mut builder = GraphBuilder::new();
    let mut resolution_counts: HashMap<Language, (usize, usize)> = HashMap::new();
    for result in &parse_results {
        let (resolver, lang_files) = resolver_context(
            result,
            &context,
            &resolvers,
            &go_resolvers,
            fallback_go_resolver.as_ref(),
            &go_files_by_module,
            &files_by_lang_for_resolve,
        );

        let counts = resolution_counts.entry(result.language).or_insert((0, 0));
        for raw in &result.imports {
            if matches!(
                raw.confidence,
                ImportConfidence::External
                    | ImportConfidence::Dynamic
                    | ImportConfidence::Unresolvable
            ) {
                counts.1 += 1;
                continue;
            }

            match resolver.resolve(raw, &context.project_root, &lang_files) {
                Some(target_module) => {
                    counts.0 += 1;
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
                None => counts.1 += 1,
            }
        }
    }

    let graph = builder.build();
    let summary = Summary::from_graph(&graph);
    let sccs = find_non_trivial_sccs(&graph);
    let unresolved_imports: usize = resolution_counts
        .values()
        .map(|(_, unresolved)| unresolved)
        .sum();
    let metadata = metadata_for(
        &context,
        &graph,
        &resolution_counts,
        &per_lang_files_parsed,
        files_parsed,
        files_skipped,
        unresolved_imports,
        start.elapsed().as_millis() as u64,
    );
    let insights = if insights_disabled
        || matches!(
            config.analyze_report.insights,
            crate::config::InsightsConfig::Off
        ) {
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

    Ok(AnalysisSnapshot {
        graph,
        summary,
        sccs,
        metadata,
        insights,
    })
}

fn group_go_files_by_module(
    context: &crate::analysis_context::AnalysisContext,
) -> HashMap<PathBuf, Vec<PathBuf>> {
    if !context.langs.contains(&Language::Go) {
        return HashMap::new();
    }

    let mut grouped: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    if let Some(go_files) = context.files_by_lang.get(&Language::Go) {
        for file in go_files {
            let mod_root = walk::find_go_module_root(file, &context.go_modules)
                .map(|(root, _)| root.to_path_buf())
                .unwrap_or_else(|| context.project_root.clone());
            grouped.entry(mod_root).or_default().push(file.clone());
        }
    }
    grouped
}

fn resolver_context<'a>(
    result: &FileParseResult,
    context: &'a crate::analysis_context::AnalysisContext,
    resolvers: &'a HashMap<Language, Box<dyn ParseFrontend>>,
    go_resolvers: &'a HashMap<PathBuf, Box<dyn ParseFrontend>>,
    fallback_go_resolver: &'a dyn ParseFrontend,
    go_files_by_module: &'a HashMap<PathBuf, Vec<PathBuf>>,
    files_by_lang_for_resolve: &'a HashMap<Language, Vec<PathBuf>>,
) -> (&'a dyn ParseFrontend, Vec<PathBuf>) {
    if result.language == Language::Go {
        let mod_root = walk::find_go_module_root(&result.original_file, &context.go_modules)
            .map(|(root, _)| root.to_path_buf())
            .unwrap_or_else(|| context.project_root.clone());
        let resolver = go_resolvers
            .get(&mod_root)
            .map(|resolver| resolver.as_ref())
            .unwrap_or(fallback_go_resolver);
        let files = go_files_by_module
            .get(&mod_root)
            .cloned()
            .unwrap_or_default();
        (resolver, files)
    } else {
        let resolver = resolvers
            .get(&result.language)
            .map(|resolver| resolver.as_ref())
            .expect("resolver should exist for non-go languages");
        let files = files_by_lang_for_resolve
            .get(&result.language)
            .cloned()
            .unwrap_or_default();
        (resolver, files)
    }
}

fn metadata_for(
    context: &crate::analysis_context::AnalysisContext,
    graph: &crate::graph::ir::DepGraph,
    resolution_counts: &HashMap<Language, (usize, usize)>,
    per_lang_files_parsed: &HashMap<Language, usize>,
    files_parsed: usize,
    files_skipped: usize,
    unresolved_imports: usize,
    elapsed_ms: u64,
) -> Metadata {
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
    let language = if context.langs.len() == 1 {
        context.langs[0].to_string()
    } else {
        context
            .langs
            .iter()
            .map(|lang| lang.to_string())
            .collect::<Vec<_>>()
            .join(",")
    };

    let mut nodes_per_lang: HashMap<Language, usize> = HashMap::new();
    for idx in graph.node_indices() {
        if let Some(lang) = graph[idx].language {
            *nodes_per_lang.entry(lang).or_insert(0) += 1;
        }
    }

    let languages = if context.langs.len() > 1 {
        Some(
            context
                .langs
                .iter()
                .map(|&lang| {
                    let (resolved, unresolved) =
                        resolution_counts.get(&lang).copied().unwrap_or((0, 0));
                    LanguageStats {
                        language: lang.to_string(),
                        files_parsed: per_lang_files_parsed.get(&lang).copied().unwrap_or(0),
                        nodes: nodes_per_lang.get(&lang).copied().unwrap_or(0),
                        imports_resolved: resolved,
                        imports_unresolved: unresolved,
                    }
                })
                .collect(),
        )
    } else {
        None
    };

    Metadata {
        language,
        granularity: "module".to_string(),
        root: context.project_root.clone(),
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
    }
}

fn chrono_now() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let days = (secs / 86400) as i64;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;
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
