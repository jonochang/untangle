use crate::config::ResolvedConfig;
use crate::errors::{Result, UntangleError};
use crate::graph::builder::{GraphBuilder, ResolvedImport};
use crate::metrics::scc;
use crate::metrics::summary::Summary;
use crate::parse::common::{ImportConfidence, RawImport, SourceLocation};
use crate::parse::factory;
use crate::parse::go::GoFrontend;
use crate::parse::rust::RustFrontend;
use crate::parse::ParseFrontend;
use crate::quality::complexity::go::GoComplexity;
use crate::quality::complexity::python::PythonComplexity;
use crate::quality::complexity::ruby::RubyComplexity;
use crate::quality::complexity::rust::RustComplexity;
use crate::quality::complexity::ComplexityFrontend;
use crate::quality::coverage::lcov::parse_lcov;
use crate::quality::metrics::metric_for;
use crate::quality::{
    CrapMetricSummary, FunctionInfo, QualityMetadata, QualityMetricKind, QualityOverallSummary,
    QualityReport, QualityResult, UntangleHotspot, UntangleMetricSummary,
};
use crate::walk::{self, Language};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct QualityRunConfig {
    pub root: PathBuf,
    pub lang: Option<Language>,
    pub metric: QualityMetricKind,
    pub coverage_file: Option<PathBuf>,
    pub top: Option<usize>,
    pub min_cc: usize,
    pub min_score: f64,
    pub include_tests: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub quiet: bool,
}

pub struct OverallRunConfig {
    pub root: PathBuf,
    pub lang: Option<Language>,
    pub coverage_file: Option<PathBuf>,
    pub top: Option<usize>,
    pub min_cc: usize,
    pub min_score: f64,
    pub quiet: bool,
    pub resolved: ResolvedConfig,
}

fn frontend_for(lang: Language) -> Box<dyn ComplexityFrontend> {
    match lang {
        Language::Go => Box::new(GoComplexity),
        Language::Python => Box::new(PythonComplexity),
        Language::Ruby => Box::new(RubyComplexity),
        Language::Rust => Box::new(RustComplexity),
    }
}

pub fn run(config: QualityRunConfig) -> Result<QualityReport> {
    let start = Instant::now();

    let root = config
        .root
        .canonicalize()
        .map_err(|_| UntangleError::NoFiles {
            path: config.root.clone(),
        })?;

    let metric_impl = metric_for(config.metric);
    if metric_impl.requires_coverage() && config.coverage_file.is_none() {
        return Err(UntangleError::Config(
            "--coverage is required for this metric".to_string(),
        ));
    }

    let coverage = if let Some(ref file) = config.coverage_file {
        Some(parse_lcov(file, &root)?)
    } else {
        None
    };

    let mut exclude = config.exclude.clone();
    exclude.extend(config.ignore_patterns.iter().cloned());

    let (files_by_lang, _langs) = discover_files_by_lang(
        &root,
        config.lang,
        &config.include,
        &exclude,
        config.include_tests,
    )?;

    let (all_functions, files_parsed, mut languages) = collect_functions(&root, &files_by_lang);

    let mut results: Vec<QualityResult> = metric_impl.compute(&all_functions, coverage.as_ref());

    results.retain(|r| r.cyclomatic_complexity >= config.min_cc);
    results.retain(|r| r.score >= config.min_score);
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if let Some(n) = config.top {
        results.truncate(n);
    }

    languages.sort();

    let metadata = QualityMetadata {
        root,
        metric: config.metric,
        coverage_file: config.coverage_file,
        languages,
        files_parsed,
        functions: all_functions.len(),
        timestamp: chrono_now(),
        elapsed_ms: start.elapsed().as_millis() as u64,
    };

    Ok(QualityReport {
        metadata,
        results,
        overall: None,
    })
}

pub fn run_overall(config: OverallRunConfig) -> Result<QualityReport> {
    let start = Instant::now();

    let root = config
        .root
        .canonicalize()
        .map_err(|_| UntangleError::NoFiles {
            path: config.root.clone(),
        })?;

    if config.coverage_file.is_none() {
        return Err(UntangleError::Config(
            "--coverage is required for this metric".to_string(),
        ));
    }

    let coverage = parse_lcov(config.coverage_file.as_ref().unwrap(), &root)?;

    let mut exclude = config.resolved.exclude.clone();
    exclude.extend(config.resolved.ignore_patterns.iter().cloned());

    let (files_by_lang, langs) = discover_files_by_lang(
        &root,
        config.lang,
        &config.resolved.include,
        &exclude,
        config.resolved.include_tests,
    )?;

    let (all_functions, files_parsed, mut languages) = collect_functions(&root, &files_by_lang);

    let metric_impl = metric_for(QualityMetricKind::Crap);
    let mut results: Vec<QualityResult> = metric_impl.compute(&all_functions, Some(&coverage));

    results.retain(|r| r.cyclomatic_complexity >= config.min_cc);
    results.retain(|r| r.score >= config.min_score);
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let crap_summary = summarize_crap(&results);

    if let Some(n) = config.top {
        results.truncate(n);
    }

    languages.sort();

    let hotspot_limit = config.top.unwrap_or(20);
    let untangle_summary = compute_untangle_summary(
        &root,
        &files_by_lang,
        &langs,
        &config.resolved,
        hotspot_limit,
    )?;

    let metadata = QualityMetadata {
        root,
        metric: QualityMetricKind::Overall,
        coverage_file: config.coverage_file,
        languages,
        files_parsed,
        functions: all_functions.len(),
        timestamp: chrono_now(),
        elapsed_ms: start.elapsed().as_millis() as u64,
    };

    Ok(QualityReport {
        metadata,
        results,
        overall: Some(QualityOverallSummary {
            untangle: untangle_summary,
            crap: crap_summary,
        }),
    })
}

fn discover_files_by_lang(
    root: &Path,
    lang: Option<Language>,
    include: &[String],
    exclude: &[String],
    include_tests: bool,
) -> Result<(HashMap<Language, Vec<PathBuf>>, Vec<Language>)> {
    let files_by_lang: HashMap<Language, Vec<PathBuf>> = match lang {
        Some(lang) => {
            let files = walk::discover_files(root, lang, include, exclude, include_tests)?;
            let mut map = HashMap::new();
            if !files.is_empty() {
                map.insert(lang, files);
            }
            map
        }
        None => walk::discover_files_multi(root, include, exclude, include_tests)?,
    };

    if files_by_lang.is_empty() {
        return Err(UntangleError::NoFiles {
            path: root.to_path_buf(),
        });
    }

    let mut langs: Vec<Language> = files_by_lang.keys().copied().collect();
    langs.sort_by(|a, b| a.to_string().cmp(&b.to_string()));

    Ok((files_by_lang, langs))
}

fn collect_functions(
    root: &Path,
    files_by_lang: &HashMap<Language, Vec<PathBuf>>,
) -> (Vec<FunctionInfo>, usize, Vec<String>) {
    let mut all_functions: Vec<FunctionInfo> = Vec::new();
    let mut files_parsed = 0usize;

    for (lang, files) in files_by_lang {
        let fe = frontend_for(*lang);
        for file in files {
            let Ok(source) = std::fs::read(file) else {
                continue;
            };
            let relative = file.strip_prefix(root).unwrap_or(file).to_path_buf();
            let fns = fe.extract_functions(&source, &relative);
            if !fns.is_empty() {
                all_functions.extend(fns);
            }
            files_parsed += 1;
        }
    }

    let languages: Vec<String> = files_by_lang.keys().map(|l| l.to_string()).collect();

    (all_functions, files_parsed, languages)
}

fn summarize_crap(results: &[QualityResult]) -> CrapMetricSummary {
    if results.is_empty() {
        return CrapMetricSummary {
            functions_reported: 0,
            mean_score: 0.0,
            p90_score: 0.0,
            max_score: 0.0,
            high_risk: 0,
            moderate_risk: 0,
            low_risk: 0,
        };
    }

    let mut scores: Vec<f64> = results.iter().map(|r| r.score).collect();
    scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mean = scores.iter().sum::<f64>() / scores.len() as f64;
    let p90_idx = (scores.len() as f64 * 0.9).ceil() as usize;
    let p90_idx = p90_idx.min(scores.len()) - 1;
    let p90 = scores[p90_idx];
    let max = *scores.last().unwrap_or(&0.0);

    let mut high_risk = 0usize;
    let mut moderate_risk = 0usize;
    let mut low_risk = 0usize;

    for result in results {
        match result.risk_band.as_deref() {
            Some("high") => high_risk += 1,
            Some("moderate") => moderate_risk += 1,
            Some("low") => low_risk += 1,
            _ => {}
        }
    }

    CrapMetricSummary {
        functions_reported: results.len(),
        mean_score: (mean * 100.0).round() / 100.0,
        p90_score: (p90 * 100.0).round() / 100.0,
        max_score: (max * 100.0).round() / 100.0,
        high_risk,
        moderate_risk,
        low_risk,
    }
}

struct FileParseResult {
    source_module: PathBuf,
    imports: Vec<RawImport>,
    language: Language,
    original_file: PathBuf,
}

fn compute_untangle_summary(
    root: &Path,
    files_by_lang: &HashMap<Language, Vec<PathBuf>>,
    langs: &[Language],
    config: &ResolvedConfig,
    hotspot_limit: usize,
) -> Result<UntangleMetricSummary> {
    let go_modules = if langs.contains(&Language::Go) {
        walk::discover_go_modules(root)
    } else {
        HashMap::new()
    };

    let go_module_path = go_modules.get(root).cloned().or_else(|| {
        if langs.contains(&Language::Go) {
            GoFrontend::read_go_mod(root)
        } else {
            None
        }
    });

    let rust_crate_name = if langs.contains(&Language::Rust) {
        RustFrontend::read_cargo_toml(root)
    } else {
        None
    };

    let mut parse_results: Vec<FileParseResult> = Vec::new();

    for (lang, files) in files_by_lang {
        for file_path in files {
            let Ok(source) = std::fs::read(file_path) else {
                continue;
            };

            let file_go_module = if *lang == Language::Go {
                walk::find_go_module_root(file_path, &go_modules)
                    .map(|(_, mp)| mp.to_string())
                    .or_else(|| go_module_path.clone())
            } else {
                go_module_path.clone()
            };

            let frontend: Box<dyn ParseFrontend> =
                factory::create_frontend(*lang, config, &file_go_module, &rust_crate_name);
            let imports = frontend.extract_imports(&source, file_path);
            let source_module = factory::source_module_path(file_path, root, *lang);

            parse_results.push(FileParseResult {
                source_module,
                imports,
                language: *lang,
                original_file: file_path.clone(),
            });
        }
    }

    let files_parsed = parse_results.len();

    let mut builder = GraphBuilder::new();

    let resolvers: HashMap<Language, Box<dyn ParseFrontend>> = langs
        .iter()
        .filter(|&&lang| lang != Language::Go)
        .map(|&lang| {
            let frontend =
                factory::create_frontend(lang, config, &go_module_path, &rust_crate_name);
            (lang, frontend)
        })
        .collect();

    let go_resolvers: HashMap<PathBuf, Box<dyn ParseFrontend>> = go_modules
        .iter()
        .map(|(mod_root, mod_path)| {
            let fe = GoFrontend::with_module_path(mod_path.clone())
                .with_exclude_stdlib(config.go.exclude_stdlib);
            (mod_root.clone(), Box::new(fe) as Box<dyn ParseFrontend>)
        })
        .collect();

    let fallback_go_resolver: Box<dyn ParseFrontend> =
        factory::create_frontend(Language::Go, config, &go_module_path, &rust_crate_name);

    let go_files_by_module: HashMap<PathBuf, Vec<PathBuf>> = if langs.contains(&Language::Go) {
        let mut by_module: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        if let Some(go_files) = files_by_lang.get(&Language::Go) {
            for f in go_files {
                let mod_root = walk::find_go_module_root(f, &go_modules)
                    .map(|(r, _)| r.to_path_buf())
                    .unwrap_or_else(|| root.to_path_buf());
                by_module.entry(mod_root).or_default().push(f.clone());
            }
        }
        by_module
    } else {
        HashMap::new()
    };

    let files_by_lang_for_resolve: HashMap<Language, Vec<PathBuf>> = files_by_lang
        .iter()
        .map(|(&lang, files)| (lang, files.clone()))
        .collect();

    for result in &parse_results {
        let (resolver, lang_files): (&dyn ParseFrontend, Vec<PathBuf>) =
            if result.language == Language::Go {
                let mod_root = walk::find_go_module_root(&result.original_file, &go_modules)
                    .map(|(r, _)| r.to_path_buf())
                    .unwrap_or_else(|| root.to_path_buf());
                let resolver = go_resolvers
                    .get(&mod_root)
                    .map(|r| r.as_ref())
                    .unwrap_or(fallback_go_resolver.as_ref());
                let files = go_files_by_module
                    .get(&mod_root)
                    .cloned()
                    .unwrap_or_default();
                (resolver, files)
            } else {
                let resolver = match resolvers.get(&result.language) {
                    Some(r) => r.as_ref(),
                    None => continue,
                };
                let files = files_by_lang_for_resolve
                    .get(&result.language)
                    .cloned()
                    .unwrap_or_default();
                (resolver, files)
            };

        for raw in &result.imports {
            if raw.confidence == ImportConfidence::External
                || raw.confidence == ImportConfidence::Dynamic
                || raw.confidence == ImportConfidence::Unresolvable
            {
                continue;
            }

            if let Some(target_module) = resolver.resolve(raw, root, &lang_files) {
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
        }
    }

    let graph = builder.build();
    let summary = Summary::from_graph(&graph);
    let scc_map = scc::node_scc_map(&graph);

    let mut nodes: Vec<_> = graph
        .node_indices()
        .map(|idx| {
            let fanout = crate::metrics::fanout::fan_out(&graph, idx);
            let fanin = crate::metrics::fanout::fan_in(&graph, idx);
            (idx, fanout, fanin)
        })
        .collect();
    nodes.sort_by(|a, b| b.1.cmp(&a.1));

    let mut hotspots = Vec::new();
    let limit = hotspot_limit.min(nodes.len());
    for &(idx, fanout, fanin) in nodes.iter().take(limit) {
        let module = graph[idx].name.clone();
        let path = graph[idx].path.clone();
        let scc = scc_map.get(&idx).copied();
        hotspots.push(UntangleHotspot {
            module,
            path,
            fanout,
            fanin,
            scc,
        });
    }

    let node_count = graph.node_count();
    let edge_count = graph.edge_count();
    let edge_density = if node_count > 1 {
        edge_count as f64 / (node_count as f64 * (node_count as f64 - 1.0))
    } else {
        0.0
    };

    Ok(UntangleMetricSummary {
        nodes: node_count,
        edges: edge_count,
        edge_density: (edge_density * 10000.0).round() / 10000.0,
        files_parsed,
        summary,
        hotspots,
    })
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
