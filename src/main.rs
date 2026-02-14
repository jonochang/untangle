#![allow(dead_code)]

mod cli;
mod config;
mod errors;
mod git;
mod graph;
mod metrics;
mod output;
mod parse;
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
