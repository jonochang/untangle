#![allow(dead_code)]

mod analysis_context;
mod architecture;
mod cli;
mod config;
mod errors;
mod formats;
mod git;
mod graph;
mod insights;
mod metrics;
mod output;
mod parse;
mod quality;
mod walk;

use clap::Parser;
use miette::Result;

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = cli::Cli::parse();
    cli::dispatch(cli).map_err(|e| miette::miette!("{e}"))?;
    Ok(())
}
