use crate::config::ResolvedConfig;
use crate::errors::{Result, UntangleError};
use crate::quality::coverage::lcov::parse_lcov;
use crate::quality::functions::{collect_functions, discover_files_by_lang};
use crate::quality::metrics::metric_for;
use crate::quality::untangle::compute_untangle_summary;
use crate::quality::{
    CrapMetricSummary, QualityMetadata, QualityMetricKind, QualityOverallSummary, QualityReport,
    QualityResult,
};
use std::path::PathBuf;
use std::time::Instant;

pub struct QualityRunConfig {
    pub root: PathBuf,
    pub lang: Option<crate::walk::Language>,
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
    pub lang: Option<crate::walk::Language>,
    pub coverage_file: Option<PathBuf>,
    pub top: Option<usize>,
    pub min_cc: usize,
    pub min_score: f64,
    pub quiet: bool,
    pub resolved: ResolvedConfig,
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

    let (files_by_lang, _) = discover_files_by_lang(
        &root,
        config.lang,
        &config.include,
        &exclude,
        config.include_tests,
    )?;

    let (all_functions, files_parsed, mut languages) = collect_functions(&root, &files_by_lang);
    let mut results = apply_metric(
        metric_impl.compute(&all_functions, coverage.as_ref()),
        config.min_cc,
        config.min_score,
        config.top,
    );

    languages.sort();

    Ok(QualityReport {
        metadata: QualityMetadata {
            root,
            metric: config.metric,
            coverage_file: config.coverage_file,
            languages,
            files_parsed,
            functions: all_functions.len(),
            timestamp: chrono_now(),
            elapsed_ms: start.elapsed().as_millis() as u64,
        },
        results: std::mem::take(&mut results),
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

    let coverage_file = config.coverage_file.clone().ok_or_else(|| {
        UntangleError::Config("--coverage is required for this metric".to_string())
    })?;
    let coverage = parse_lcov(&coverage_file, &root)?;

    let mut exclude = config.resolved.exclude.clone();
    exclude.extend(config.resolved.ignore_patterns.iter().cloned());

    let (files_by_lang, _) = discover_files_by_lang(
        &root,
        config.lang,
        &config.resolved.include,
        &exclude,
        config.resolved.include_tests,
    )?;

    let (all_functions, files_parsed, mut languages) = collect_functions(&root, &files_by_lang);
    let crap_results = metric_for(QualityMetricKind::Crap).compute(&all_functions, Some(&coverage));
    let crap_summary = summarize_crap(&crap_results);
    let results = apply_metric(crap_results, config.min_cc, config.min_score, config.top);
    let untangle_summary =
        compute_untangle_summary(&root, &config.resolved, config.top.unwrap_or(20))?;

    languages.sort();

    Ok(QualityReport {
        metadata: QualityMetadata {
            root,
            metric: QualityMetricKind::Overall,
            coverage_file: Some(coverage_file),
            languages,
            files_parsed,
            functions: all_functions.len(),
            timestamp: chrono_now(),
            elapsed_ms: start.elapsed().as_millis() as u64,
        },
        results,
        overall: Some(QualityOverallSummary {
            untangle: untangle_summary,
            crap: crap_summary,
        }),
    })
}

fn apply_metric(
    mut results: Vec<QualityResult>,
    min_cc: usize,
    min_score: f64,
    top: Option<usize>,
) -> Vec<QualityResult> {
    results.retain(|result| result.cyclomatic_complexity >= min_cc);
    results.retain(|result| result.score >= min_score);
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if let Some(limit) = top {
        results.truncate(limit);
    }

    results
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

    let mut scores: Vec<f64> = results.iter().map(|result| result.score).collect();
    scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mean = scores.iter().sum::<f64>() / scores.len() as f64;
    let p90_idx = (scores.len() as f64 * 0.9).ceil() as usize;
    let p90 = scores[p90_idx.min(scores.len()) - 1];
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
