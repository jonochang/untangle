use crate::cli::common::{RuntimeArgs, TargetArgs};
use crate::config::resolve::{resolve_config, CliOverrides};
use crate::errors::{Result, UntangleError};
use crate::formats::QualityFormat;
use crate::quality::engine::{self, OverallRunConfig, QualityRunConfig};
use crate::quality::output::{json::write_quality_json, text::write_quality_text};
use crate::quality::QualityMetricKind;
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct QualityArgs {
    #[command(subcommand)]
    pub command: QualityCommand,
}

#[derive(Debug, Subcommand)]
pub enum QualityCommand {
    /// Function-level quality metrics
    Functions(FunctionQualityArgs),
    /// Project-level quality summary
    Project(ProjectQualityArgs),
}

#[derive(Debug, Args)]
pub struct FunctionQualityArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[command(flatten)]
    pub runtime: RuntimeArgs,

    /// Metric to compute
    #[arg(long, default_value = "crap", value_parser = parse_metric)]
    pub metric: QualityMetricKind,

    /// LCOV coverage file
    #[arg(long)]
    pub coverage: Option<PathBuf>,

    /// Output format
    #[arg(long)]
    pub format: Option<QualityFormat>,

    /// Show only top N results
    #[arg(long)]
    pub top: Option<usize>,

    /// Minimum cyclomatic complexity
    #[arg(long, default_value_t = 2)]
    pub min_cc: usize,

    /// Minimum metric score
    #[arg(long, default_value_t = 0.0)]
    pub min_score: f64,
}

#[derive(Debug, Args)]
pub struct ProjectQualityArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[command(flatten)]
    pub runtime: RuntimeArgs,

    /// LCOV coverage file
    #[arg(long)]
    pub coverage: Option<PathBuf>,

    /// Output format
    #[arg(long)]
    pub format: Option<QualityFormat>,

    /// Show only top N results
    #[arg(long)]
    pub top: Option<usize>,

    /// Minimum cyclomatic complexity
    #[arg(long, default_value_t = 2)]
    pub min_cc: usize,

    /// Minimum metric score
    #[arg(long, default_value_t = 0.0)]
    pub min_score: f64,
}

impl FunctionQualityArgs {
    fn to_cli_overrides(&self) -> CliOverrides {
        CliOverrides {
            lang: self.target.lang,
            quiet: self.runtime.quiet,
            include_tests: self.target.include_tests,
            include: self.target.include.clone(),
            exclude: self.target.exclude.clone(),
            ..Default::default()
        }
    }
}

impl ProjectQualityArgs {
    fn to_cli_overrides(&self) -> CliOverrides {
        CliOverrides {
            lang: self.target.lang,
            quiet: self.runtime.quiet,
            include_tests: self.target.include_tests,
            include: self.target.include.clone(),
            exclude: self.target.exclude.clone(),
            ..Default::default()
        }
    }
}

fn parse_metric(s: &str) -> std::result::Result<QualityMetricKind, String> {
    s.parse()
}

pub fn run(args: &QualityArgs) -> Result<()> {
    match &args.command {
        QualityCommand::Functions(args) => run_functions(args),
        QualityCommand::Project(args) => run_project(args),
    }
}

fn run_functions(args: &FunctionQualityArgs) -> Result<()> {
    if args.metric == QualityMetricKind::Overall {
        return Err(UntangleError::Config(
            "`quality functions` supports only function-level metrics".to_string(),
        ));
    }

    let path = args.target.path.clone().unwrap_or_else(|| ".".into());
    let root = path
        .canonicalize()
        .map_err(|_| UntangleError::NoFiles { path })?;

    let resolved = resolve_config(&root, &args.to_cli_overrides())?;
    let format = args.format.unwrap_or(resolved.quality_functions.format);

    let report = engine::run(QualityRunConfig {
        root,
        lang: args.target.lang,
        metric: args.metric,
        coverage_file: args.coverage.clone(),
        top: args.top.or(resolved.quality_functions.top),
        min_cc: args.min_cc,
        min_score: args.min_score,
        include_tests: resolved.include_tests,
        include: resolved.include,
        exclude: resolved.exclude,
        ignore_patterns: resolved.ignore_patterns,
        quiet: args.runtime.quiet,
    })?;

    let mut stdout = std::io::stdout();
    match format {
        QualityFormat::Json => write_quality_json(&mut stdout, &report)?,
        QualityFormat::Text => write_quality_text(&mut stdout, &report)?,
    }

    Ok(())
}

fn run_project(args: &ProjectQualityArgs) -> Result<()> {
    let path = args.target.path.clone().unwrap_or_else(|| ".".into());
    let root = path
        .canonicalize()
        .map_err(|_| UntangleError::NoFiles { path })?;

    let resolved = resolve_config(&root, &args.to_cli_overrides())?;
    let format = args.format.unwrap_or(resolved.quality_project.format);

    let report = engine::run_overall(OverallRunConfig {
        root,
        lang: args.target.lang,
        coverage_file: args.coverage.clone(),
        top: args.top.or(resolved.quality_project.top),
        min_cc: args.min_cc,
        min_score: args.min_score,
        quiet: args.runtime.quiet,
        resolved,
    })?;

    let mut stdout = std::io::stdout();
    match format {
        QualityFormat::Json => write_quality_json(&mut stdout, &report)?,
        QualityFormat::Text => write_quality_text(&mut stdout, &report)?,
    }

    Ok(())
}
