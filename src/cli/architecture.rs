use crate::architecture;
use crate::config::resolve::{resolve_config, CliOverrides};
use crate::errors::{Result, UntangleError};
use crate::graph::load::load_dependency_graph;
use crate::output::OutputFormat;
use crate::walk::Language;
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct ArchitectureArgs {
    /// Path to analyze
    pub path: PathBuf,

    /// Language to analyze
    #[arg(long, value_parser = parse_language)]
    pub lang: Option<Language>,

    /// Output format (json or dot)
    #[arg(long)]
    pub format: Option<OutputFormat>,

    /// Hierarchy depth to project
    #[arg(long, default_value_t = 1)]
    pub level: usize,

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

impl ArchitectureArgs {
    fn to_cli_overrides(&self) -> CliOverrides {
        CliOverrides {
            lang: self.lang,
            format: self.format,
            quiet: self.quiet,
            include_tests: self.include_tests,
            include: self.include.clone(),
            exclude: self.exclude.clone(),
            ..Default::default()
        }
    }
}

fn parse_language(s: &str) -> std::result::Result<Language, String> {
    s.parse()
}

pub fn run(args: &ArchitectureArgs) -> Result<()> {
    let root = args
        .path
        .canonicalize()
        .map_err(|_| UntangleError::NoFiles {
            path: args.path.clone(),
        })?;
    let config = resolve_config(&root, &args.to_cli_overrides())?;
    let format = config.format.parse().unwrap_or(OutputFormat::Dot);
    let graph = load_dependency_graph(&root, &config)?;
    let architecture = architecture::project_architecture(&graph, &root, args.level.max(1));
    let mut stdout = std::io::stdout();

    match format {
        OutputFormat::Json => serde_json::to_writer_pretty(&mut stdout, &architecture)?,
        OutputFormat::Dot => architecture::write_dot(&mut stdout, &architecture)?,
        _ => {
            return Err(crate::errors::UntangleError::Config(
                "architecture only supports --format json or --format dot".to_string(),
            ))
        }
    }

    Ok(())
}
