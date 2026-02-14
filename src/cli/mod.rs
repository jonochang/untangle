pub mod analyze;
pub mod diff;
pub mod graph;

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
    /// Export raw dependency graph
    Graph(graph::GraphArgs),
}

/// Dispatch to the appropriate command handler.
pub fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Analyze(args) => analyze::run(&args),
        Commands::Diff(args) => diff::run(&args),
        Commands::Graph(args) => graph::run(&args),
    }
}
