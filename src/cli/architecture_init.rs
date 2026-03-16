use crate::analysis_context::{canonicalize_root, resolve_project_root};
use crate::architecture::policy;
use crate::cli::common::{RuntimeArgs, TargetArgs};
use crate::config::resolve::{resolve_config, CliOverrides};
use crate::errors::Result;
use crate::graph::load::load_dependency_graph;
use clap::Args;

#[derive(Debug, Args)]
pub struct ArchitectureInitArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[command(flatten)]
    pub runtime: RuntimeArgs,

    /// Hierarchy depth to project
    #[arg(long)]
    pub level: Option<usize>,

    /// Replace an existing architecture policy section
    #[arg(long)]
    pub force: bool,
}

impl ArchitectureInitArgs {
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

pub fn run(args: &ArchitectureInitArgs) -> Result<()> {
    let path = args.target.path.clone().unwrap_or_else(|| ".".into());
    let scan_root = canonicalize_root(&path)?;
    let project_root = resolve_project_root(&scan_root, args.target.lang);
    let config = resolve_config(&project_root, &args.to_cli_overrides())?;
    let graph = load_dependency_graph(&scan_root, &project_root, &config)?;
    let policy = policy::infer_starter_policy(
        &graph,
        &project_root,
        &config.analyze_architecture,
        args.level,
    );
    let config_path = project_root.join(".untangle.toml");
    policy::write_starter_policy_file(&config_path, &policy, args.force)?;
    println!(
        "Wrote starter architecture policy to {}",
        config_path.display()
    );
    Ok(())
}
