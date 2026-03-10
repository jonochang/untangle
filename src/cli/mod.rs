pub mod analyze;
pub mod architecture;
pub mod common;
pub mod config;
pub mod diff;
pub mod graph;
pub mod quality;
pub mod service_graph;

use crate::errors::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "untangle",
    version,
    about = "Module-level dependency graph analyzer"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Analyze dependency structure of a codebase
    Analyze(analyze::AnalyzeArgs),
    /// Compare dependency structure between two git refs
    Diff(diff::DiffArgs),
    /// Deprecated alias for `analyze graph`
    #[command(hide = true)]
    Graph(graph::GraphArgs),
    /// Show or explain configuration
    Config(config::ConfigArgs),
    /// Analyze cross-service dependencies
    ServiceGraph(service_graph::ServiceGraphArgs),
    /// Analyze code quality metrics
    Quality(quality::QualityArgs),
    /// Deprecated alias for `analyze architecture`
    #[command(hide = true)]
    Architecture(architecture::ArchitectureArgs),
}

/// Dispatch to the appropriate command handler.
pub fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Analyze(args) => analyze::run(&args),
        Commands::Diff(args) => diff::run(&args),
        Commands::Graph(args) => {
            eprintln!("Warning: `untangle graph` is deprecated; use `untangle analyze graph`");
            graph::run(&args)
        }
        Commands::Config(args) => config::run(&args),
        Commands::ServiceGraph(args) => service_graph::run(&args),
        Commands::Quality(args) => quality::run(&args),
        Commands::Architecture(args) => {
            eprintln!(
                "Warning: `untangle architecture` is deprecated; use `untangle analyze architecture`"
            );
            architecture::run(&args)
        }
    }
}
