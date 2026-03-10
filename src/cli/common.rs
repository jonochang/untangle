use crate::walk::Language;
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Clone, Args)]
pub struct TargetArgs {
    /// Path to analyze (defaults to current directory)
    pub path: Option<PathBuf>,

    /// Language to analyze
    #[arg(long, value_parser = parse_language)]
    pub lang: Option<Language>,

    /// Include test files
    #[arg(long)]
    pub include_tests: bool,

    /// Include glob patterns
    #[arg(long)]
    pub include: Vec<String>,

    /// Exclude glob patterns
    #[arg(long)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Args)]
pub struct RuntimeArgs {
    /// Suppress progress output
    #[arg(long)]
    pub quiet: bool,
}

pub fn parse_language(s: &str) -> std::result::Result<Language, String> {
    s.parse()
}

pub fn resolve_path(path: &Option<PathBuf>) -> PathBuf {
    path.clone().unwrap_or_else(|| PathBuf::from("."))
}
