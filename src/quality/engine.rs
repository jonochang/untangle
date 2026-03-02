use crate::errors::{Result, UntangleError};
use crate::quality::complexity::go::GoComplexity;
use crate::quality::complexity::python::PythonComplexity;
use crate::quality::complexity::ruby::RubyComplexity;
use crate::quality::complexity::rust::RustComplexity;
use crate::quality::complexity::ComplexityFrontend;
use crate::quality::coverage::lcov::parse_lcov;
use crate::quality::metrics::metric_for;
use crate::quality::{
    FunctionInfo, QualityMetadata, QualityMetricKind, QualityReport, QualityResult,
};
use crate::walk::{self, Language};
use std::collections::HashMap;
use std::path::PathBuf;
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

    let files_by_lang: HashMap<Language, Vec<PathBuf>> = match config.lang {
        Some(lang) => {
            let files = walk::discover_files(
                &root,
                lang,
                &config.include,
                &exclude,
                config.include_tests,
            )?;
            let mut map = HashMap::new();
            if !files.is_empty() {
                map.insert(lang, files);
            }
            map
        }
        None => walk::discover_files_multi(&root, &config.include, &exclude, config.include_tests)?,
    };

    if files_by_lang.is_empty() {
        return Err(UntangleError::NoFiles { path: root });
    }

    let mut all_functions: Vec<FunctionInfo> = Vec::new();
    let mut files_parsed = 0usize;

    for (lang, files) in &files_by_lang {
        let fe = frontend_for(*lang);
        for file in files {
            let Ok(source) = std::fs::read(file) else {
                continue;
            };
            let relative = file.strip_prefix(&root).unwrap_or(file).to_path_buf();
            let fns = fe.extract_functions(&source, &relative);
            if !fns.is_empty() {
                all_functions.extend(fns);
            }
            files_parsed += 1;
        }
    }

    let mut results: Vec<QualityResult> = metric_impl.compute(&all_functions, coverage.as_ref());

    results.retain(|r| r.cyclomatic_complexity >= config.min_cc);
    results.retain(|r| r.score >= config.min_score);
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    if let Some(n) = config.top {
        results.truncate(n);
    }

    let mut languages: Vec<String> = files_by_lang.keys().map(|l| l.to_string()).collect();
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

    Ok(QualityReport { metadata, results })
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
