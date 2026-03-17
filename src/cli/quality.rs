use crate::cli::common::{RuntimeArgs, TargetArgs};
use crate::config::resolve::{resolve_config, CliOverrides};
use crate::errors::{Result, UntangleError};
use crate::formats::QualityFormat;
use crate::quality::engine::{self, OverallRunConfig, QualityRunConfig};
use crate::quality::output::{json::write_quality_json, text::write_quality_text};
use crate::quality::report::{self, UnifiedRunConfig};
use crate::quality::QualityMetricKind;
use crate::spec_quality::{self, SpecQualityRunConfig};
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
    /// Engineer-facing quality report with structural, function, and architecture analysis
    Report(ReportQualityArgs),
    /// Project-level quality summary
    Project(ProjectQualityArgs),
    /// Test/spec quality guidance
    Specs(SpecQualityArgs),
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

#[derive(Debug, Args)]
pub struct ReportQualityArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[command(flatten)]
    pub runtime: RuntimeArgs,

    /// LCOV coverage file. When omitted, function quality falls back to complexity.
    #[arg(long)]
    pub coverage: Option<PathBuf>,

    /// Output format
    #[arg(long)]
    pub format: Option<QualityFormat>,

    /// Show only top N hotspots, function results, and priority actions
    #[arg(long)]
    pub top: Option<usize>,

    /// Minimum cyclomatic complexity
    #[arg(long, default_value_t = 2)]
    pub min_cc: usize,

    /// Minimum metric score
    #[arg(long, default_value_t = 0.0)]
    pub min_score: f64,

    /// Hierarchy depth for the embedded architecture view
    #[arg(long)]
    pub architecture_level: Option<usize>,
}

#[derive(Debug, Args)]
pub struct SpecQualityArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[command(flatten)]
    pub runtime: RuntimeArgs,

    /// Output format
    #[arg(long)]
    pub format: Option<QualityFormat>,

    /// Show only top N worst cases
    #[arg(long)]
    pub top: Option<usize>,

    /// Write a baseline JSON report
    #[arg(long)]
    pub write_baseline: bool,

    /// Compare against an earlier baseline JSON report
    #[arg(long)]
    pub compare: Option<PathBuf>,
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

impl ReportQualityArgs {
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

impl SpecQualityArgs {
    fn to_cli_overrides(&self) -> CliOverrides {
        CliOverrides {
            lang: self.target.lang,
            quiet: self.runtime.quiet,
            include_tests: true,
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
        QualityCommand::Report(args) => run_report(args),
        QualityCommand::Project(args) => run_project(args),
        QualityCommand::Specs(args) => run_specs(args),
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

fn run_report(args: &ReportQualityArgs) -> Result<()> {
    let path = args.target.path.clone().unwrap_or_else(|| ".".into());
    let root = path
        .canonicalize()
        .map_err(|_| UntangleError::NoFiles { path })?;

    let resolved = resolve_config(&root, &args.to_cli_overrides())?;
    let format = args.format.unwrap_or(resolved.quality_project.format);

    let report = report::run(UnifiedRunConfig {
        root,
        lang: args.target.lang,
        coverage_file: args.coverage.clone(),
        top: args.top.or(resolved.quality_project.top),
        min_cc: args.min_cc,
        min_score: args.min_score,
        architecture_level: args.architecture_level,
        quiet: args.runtime.quiet,
        resolved,
    })?;

    let mut stdout = std::io::stdout();
    match format {
        QualityFormat::Json => report::write_json(&mut stdout, &report)?,
        QualityFormat::Text => report::write_text(&mut stdout, &report)?,
    }

    Ok(())
}

fn run_specs(args: &SpecQualityArgs) -> Result<()> {
    let path = args.target.path.clone().unwrap_or_else(|| ".".into());
    let root = path
        .canonicalize()
        .map_err(|_| UntangleError::NoFiles { path })?;

    let resolved = resolve_config(&root, &args.to_cli_overrides())?;
    let format = args.format.unwrap_or(resolved.quality_specs.format);
    let mut report = spec_quality::run(SpecQualityRunConfig {
        root,
        lang: args.target.lang,
        top: args.top.or(resolved.quality_specs.top),
        quiet: args.runtime.quiet,
        include: resolved.include.clone(),
        exclude: resolved.exclude.clone(),
        ignore_patterns: resolved.ignore_patterns.clone(),
        defaults: resolved.quality_specs.clone(),
    })?;

    if let Some(ref compare) = args.compare {
        spec_quality::attach_comparison(&mut report, compare)?;
    }

    if args.write_baseline {
        let baseline_path = spec_quality::write_baseline(&report, None)?;
        if !args.runtime.quiet {
            eprintln!("Wrote spec-quality baseline to {}", baseline_path.display());
        }
    }

    let mut stdout = std::io::stdout();
    match format {
        QualityFormat::Json => spec_quality::write_json(&mut stdout, &report)?,
        QualityFormat::Text => spec_quality::write_text(&mut stdout, &report)?,
    }

    Ok(())
}
