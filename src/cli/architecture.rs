use crate::analysis_context::{canonicalize_root, resolve_project_root};
use crate::architecture;
use crate::cli::common::{RuntimeArgs, TargetArgs};
use crate::config::resolve::{resolve_config, CliOverrides};
use crate::errors::Result;
use crate::formats::ArchitectureFormat;
use crate::graph::load::load_dependency_graph;
use clap::Args;

#[derive(Debug, Args)]
pub struct ArchitectureArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[command(flatten)]
    pub runtime: RuntimeArgs,

    /// Output format
    #[arg(long)]
    pub format: Option<ArchitectureFormat>,

    /// Hierarchy depth to project
    #[arg(long)]
    pub level: Option<usize>,
}

impl ArchitectureArgs {
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

pub fn run(args: &ArchitectureArgs) -> Result<()> {
    let path = args.target.path.clone().unwrap_or_else(|| ".".into());
    let scan_root = canonicalize_root(&path)?;
    let project_root = resolve_project_root(&scan_root, args.target.lang);
    let config = resolve_config(&project_root, &args.to_cli_overrides())?;
    let format = args.format.unwrap_or(config.analyze_architecture.format);
    let graph = load_dependency_graph(&scan_root, &project_root, &config)?;
    let level = args
        .level
        .unwrap_or(config.analyze_architecture.level)
        .max(1);
    let architecture = architecture::project_architecture(&graph, &project_root, level);
    let mut stdout = std::io::stdout();

    match format {
        ArchitectureFormat::Json => serde_json::to_writer_pretty(
            &mut stdout,
            &serde_json::json!({
                "kind": "analyze.architecture",
                "schema_version": 2,
                "nodes": architecture.nodes,
                "edges": architecture.edges,
            }),
        )?,
        ArchitectureFormat::Dot => architecture::write_dot(&mut stdout, &architecture)?,
    }

    Ok(())
}
