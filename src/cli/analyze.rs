use crate::analysis_report::{self, AnalysisReportRequest};
use crate::cli::architecture;
use crate::cli::common::{RuntimeArgs, TargetArgs};
use crate::cli::graph;
use crate::errors::Result;
use crate::formats::AnalyzeReportFormat;
use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Args)]
pub struct AnalyzeArgs {
    #[command(subcommand)]
    pub command: AnalyzeCommand,
}

#[derive(Debug, Subcommand)]
pub enum AnalyzeCommand {
    /// Generate the default structural report
    Report(ReportArgs),
    /// Export the raw dependency graph
    Graph(graph::GraphArgs),
    /// Export the projected architecture view
    Architecture(architecture::ArchitectureArgs),
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum InsightsMode {
    Auto,
    On,
    Off,
}

#[derive(Debug, Args)]
pub struct ReportArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[command(flatten)]
    pub runtime: RuntimeArgs,

    /// Output format
    #[arg(long)]
    pub format: Option<AnalyzeReportFormat>,

    /// Number of top hotspots to report
    #[arg(long)]
    pub top: Option<usize>,

    /// Fan-out threshold for reporting
    #[arg(long)]
    pub threshold_fanout: Option<usize>,

    /// Fan-out threshold for SCC size
    #[arg(long)]
    pub threshold_scc: Option<usize>,

    /// Insight rendering mode
    #[arg(long, default_value = "auto")]
    pub insights: InsightsMode,

    /// Deprecated alias for `--insights off`
    #[arg(long, hide = true)]
    pub no_insights: bool,
}

pub fn run(args: &AnalyzeArgs) -> Result<()> {
    match &args.command {
        AnalyzeCommand::Report(args) => analysis_report::run_report(AnalysisReportRequest {
            path: args.target.path.clone().unwrap_or_else(|| ".".into()),
            lang: args.target.lang,
            quiet: args.runtime.quiet,
            include_tests: args.target.include_tests,
            include: args.target.include.clone(),
            exclude: args.target.exclude.clone(),
            format: args.format,
            top: args.top,
            threshold_fanout: args.threshold_fanout,
            threshold_scc: args.threshold_scc,
            insights_disabled: args.no_insights || matches!(args.insights, InsightsMode::Off),
        }),
        AnalyzeCommand::Graph(args) => graph::run(args),
        AnalyzeCommand::Architecture(args) => architecture::run(args),
    }
}
