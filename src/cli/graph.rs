use crate::analysis_context::{canonicalize_root, resolve_project_root};
use crate::cli::common::{RuntimeArgs, TargetArgs};
use crate::config::resolve::{resolve_config, CliOverrides};
use crate::errors::Result;
use crate::formats::GraphFormat;
use crate::graph::load::load_dependency_graph;
use clap::Args;

#[derive(Debug, Args)]
pub struct GraphArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[command(flatten)]
    pub runtime: RuntimeArgs,

    /// Output format
    #[arg(long)]
    pub format: Option<GraphFormat>,
}

impl GraphArgs {
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

pub fn run(args: &GraphArgs) -> Result<()> {
    let path = args.target.path.clone().unwrap_or_else(|| ".".into());
    let scan_root = canonicalize_root(&path)?;
    let project_root = resolve_project_root(&scan_root, args.target.lang);
    let config = resolve_config(&project_root, &args.to_cli_overrides())?;
    let format = args.format.unwrap_or(config.analyze_graph.format);
    let graph = load_dependency_graph(&scan_root, &project_root, &config)?;
    let mut stdout = std::io::stdout();

    match format {
        GraphFormat::Dot => crate::output::dot::write_dot(&mut stdout, &graph)?,
        GraphFormat::Json => {
            let nodes: Vec<_> = graph.node_indices().map(|i| &graph[i]).collect();
            let edges: Vec<_> = graph
                .edge_indices()
                .map(|e| {
                    let (s, t) = graph.edge_endpoints(e).unwrap();
                    serde_json::json!({
                        "from": graph[s].name,
                        "to": graph[t].name,
                        "source_locations": graph[e].source_locations,
                    })
                })
                .collect();
            serde_json::to_writer_pretty(
                &mut stdout,
                &serde_json::json!({
                    "kind": "analyze.graph",
                    "schema_version": 2,
                    "nodes": nodes,
                    "edges": edges,
                }),
            )?;
        }
    }

    Ok(())
}
