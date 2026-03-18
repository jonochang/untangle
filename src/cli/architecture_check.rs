use crate::analysis_context::{canonicalize_root, resolve_project_root};
use crate::architecture::policy;
use crate::cli::common::{RuntimeArgs, TargetArgs};
use crate::config::resolve::{resolve_config, CliOverrides};
use crate::errors::Result;
use crate::formats::ArchitectureCheckFormat;
use crate::graph::load::load_dependency_graph;
use clap::Args;

#[derive(Debug, Args)]
pub struct ArchitectureCheckArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[command(flatten)]
    pub runtime: RuntimeArgs,

    /// Output format
    #[arg(long)]
    pub format: Option<ArchitectureCheckFormat>,

    /// Hierarchy depth to project
    #[arg(long)]
    pub level: Option<usize>,
}

impl ArchitectureCheckArgs {
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

pub fn run(args: &ArchitectureCheckArgs) -> Result<()> {
    let path = args.target.path.clone().unwrap_or_else(|| ".".into());
    let scan_root = canonicalize_root(&path)?;
    let project_root = resolve_project_root(&scan_root, args.target.lang);
    let config = resolve_config(&project_root, &args.to_cli_overrides())?;
    let graph = load_dependency_graph(&scan_root, &project_root, &config)?;
    let result = policy::check_graph(
        &graph,
        &project_root,
        &config.analyze_architecture,
        args.level,
    );

    let mut stdout = std::io::stdout();
    match args
        .format
        .unwrap_or(config.analyze_architecture.check_format)
    {
        ArchitectureCheckFormat::Json => policy::write_check_json(&mut stdout, &result)?,
        ArchitectureCheckFormat::Text => policy::write_check_text(&mut stdout, &result)?,
    }

    if result.summary.verdict == policy::ArchitectureVerdict::Fail {
        std::process::exit(1);
    }

    Ok(())
}
