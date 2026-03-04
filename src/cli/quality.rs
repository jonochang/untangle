use crate::config::resolve::{resolve_config, CliOverrides};
use crate::errors::{Result, UntangleError};
use crate::output::OutputFormat;
use crate::quality::engine::{self, OverallRunConfig, QualityRunConfig};
use crate::quality::output::{json::write_quality_json, text::write_quality_text};
use crate::quality::QualityMetricKind;
use crate::walk::Language;
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct QualityArgs {
    /// Path to analyze
    pub path: PathBuf,

    /// Metric to compute
    #[arg(long, default_value = "crap", value_parser = parse_metric)]
    pub metric: QualityMetricKind,

    /// LCOV coverage file
    #[arg(long)]
    pub coverage: Option<PathBuf>,

    /// Language to analyze
    #[arg(long, value_parser = parse_language)]
    pub lang: Option<Language>,

    /// Output format
    #[arg(long)]
    pub format: Option<OutputFormat>,

    /// Show only top N results
    #[arg(long)]
    pub top: Option<usize>,

    /// Minimum cyclomatic complexity
    #[arg(long, default_value_t = 2)]
    pub min_cc: usize,

    /// Minimum metric score
    #[arg(long, default_value_t = 0.0)]
    pub min_score: f64,

    /// Include test files
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
}

fn parse_language(s: &str) -> std::result::Result<Language, String> {
    s.parse()
}

fn parse_metric(s: &str) -> std::result::Result<QualityMetricKind, String> {
    s.parse()
}

pub fn run(args: &QualityArgs) -> Result<()> {
    let root = args
        .path
        .canonicalize()
        .map_err(|_| UntangleError::NoFiles {
            path: args.path.clone(),
        })?;

    let overrides = CliOverrides {
        lang: args.lang,
        format: args.format,
        quiet: args.quiet,
        top: args.top,
        include_tests: args.include_tests,
        include: args.include.clone(),
        exclude: args.exclude.clone(),
        ..Default::default()
    };

    let resolved = resolve_config(&root, &overrides)?;
    let format: OutputFormat = args
        .format
        .or_else(|| resolved.format.parse().ok())
        .unwrap_or_default();

    if matches!(format, OutputFormat::Dot | OutputFormat::Sarif) {
        return Err(UntangleError::Config(
            "quality output supports only json or text".to_string(),
        ));
    }

    let report = if args.metric == QualityMetricKind::Overall {
        engine::run_overall(OverallRunConfig {
            root,
            lang: args.lang,
            coverage_file: args.coverage.clone(),
            top: args.top,
            min_cc: args.min_cc,
            min_score: args.min_score,
            quiet: args.quiet,
            resolved,
        })?
    } else {
        engine::run(QualityRunConfig {
            root,
            lang: args.lang,
            metric: args.metric,
            coverage_file: args.coverage.clone(),
            top: args.top,
            min_cc: args.min_cc,
            min_score: args.min_score,
            include_tests: resolved.include_tests,
            include: resolved.include,
            exclude: resolved.exclude,
            ignore_patterns: resolved.ignore_patterns,
            quiet: args.quiet,
        })?
    };

    let mut stdout = std::io::stdout();
    match format {
        OutputFormat::Json => write_quality_json(&mut stdout, &report)?,
        OutputFormat::Text => write_quality_text(&mut stdout, &report)?,
        _ => unreachable!(),
    }

    Ok(())
}
