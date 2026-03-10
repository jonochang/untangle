use crate::cli::common::resolve_path;
use crate::config::resolve::{resolve_config, CliOverrides};
use crate::errors::{Result, UntangleError};
use crate::formats::ServiceGraphFormat;
use crate::service_graph::ServiceGraphOutput;
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct ServiceGraphArgs {
    /// Path to the project root (defaults to current directory)
    pub path: Option<PathBuf>,

    /// Output format (json, text, dot)
    #[arg(long)]
    pub format: Option<ServiceGraphFormat>,
}

pub fn run(args: &ServiceGraphArgs) -> Result<()> {
    let path = resolve_path(&args.path);
    let root = path
        .canonicalize()
        .map_err(|error| UntangleError::Io(std::io::Error::new(error.kind(), error.to_string())))?;
    let config = resolve_config(&root, &CliOverrides::default())?;
    let format = args.format.unwrap_or(config.service_graph.format);

    if config.services.is_empty() {
        return Err(UntangleError::Config(
            "No [services] configured in .untangle.toml. Add service declarations to use service-graph.".to_string(),
        ));
    }

    let output = crate::service_graph::analyze(&root, &config.services)?;
    match format {
        ServiceGraphFormat::Json => {
            serde_json::to_writer_pretty(
                std::io::stdout(),
                &serde_json::json!({
                    "kind": "service_graph",
                    "schema_version": 2,
                    "services": output.services,
                    "cross_service_edges": output.cross_service_edges,
                }),
            )?;
            println!();
        }
        ServiceGraphFormat::Text => write_service_graph_text(&output),
        ServiceGraphFormat::Dot => write_service_graph_dot(&output),
    }

    Ok(())
}

fn write_service_graph_text(output: &ServiceGraphOutput) {
    println!("=== Service Graph ===\n");
    println!("Services ({}):", output.services.len());
    for service in &output.services {
        let language = service.language.as_deref().unwrap_or("auto-detect");
        println!(
            "  {} ({}) - {} files at {}",
            service.name,
            language,
            service.file_count,
            service.root.display()
        );
    }

    println!(
        "\nCross-Service Dependencies ({}):",
        output.cross_service_edges.len()
    );
    if output.cross_service_edges.is_empty() {
        println!("  (none detected)");
    } else {
        for edge in &output.cross_service_edges {
            let operation = edge.operation.as_deref().unwrap_or("(unknown)");
            println!(
                "  {} -> {} [{}] {}",
                edge.from_service, edge.to_service, edge.kind, operation
            );
            for location in &edge.source_locations {
                println!("    at {}:{}", location.file.display(), location.line);
            }
        }
    }
}

fn write_service_graph_dot(output: &ServiceGraphOutput) {
    println!("digraph service_dependencies {{");
    println!("    rankdir=LR;");
    println!("    node [shape=box, style=filled];");
    println!();

    for service in &output.services {
        let color = match service.language.as_deref() {
            Some("go") => "lightblue",
            Some("python") => "lightyellow",
            Some("ruby") => "lightcoral",
            Some("rust") => "lightsalmon",
            _ => "white",
        };
        println!(
            "    \"{}\" [label=\"{}\\n({})\\n{} files\", fillcolor={}];",
            service.name,
            service.name,
            service.language.as_deref().unwrap_or("auto"),
            service.file_count,
            color
        );
    }

    println!();

    for edge in &output.cross_service_edges {
        let style = match edge.kind.as_str() {
            "graphql_query" => "dashed",
            "rest_call" => "dotted",
            _ => "solid",
        };
        let label = edge.operation.as_deref().unwrap_or(&edge.kind);
        println!(
            "    \"{}\" -> \"{}\" [label=\"{}\", style={}, color=red];",
            edge.from_service, edge.to_service, label, style
        );
    }

    println!("}}");
}
