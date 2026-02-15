use crate::config::resolve::{resolve_config, CliOverrides};
use crate::config::ResolvedService;
use crate::errors::{Result, UntangleError};
use crate::output::OutputFormat;
use crate::parse::common::SourceLocation;
use crate::parse::graphql;
use crate::parse::graphql_client;
use crate::parse::openapi;
use crate::parse::rest_client;
use crate::walk;
use clap::Args;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Args)]
pub struct ServiceGraphArgs {
    /// Path to the project root
    pub path: PathBuf,

    /// Output format (json, text, dot)
    #[arg(long, default_value = "json")]
    pub format: OutputFormat,
}

/// A cross-service dependency edge.
#[derive(Debug, Clone, Serialize)]
pub struct CrossServiceEdge {
    pub from_service: String,
    pub to_service: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    pub source_locations: Vec<SourceLocation>,
}

/// Service-graph output.
#[derive(Debug, Serialize)]
pub struct ServiceGraphOutput {
    pub services: Vec<ServiceInfo>,
    pub cross_service_edges: Vec<CrossServiceEdge>,
}

#[derive(Debug, Serialize)]
pub struct ServiceInfo {
    pub name: String,
    pub root: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub file_count: usize,
}

pub fn run(args: &ServiceGraphArgs) -> Result<()> {
    let root = args
        .path
        .canonicalize()
        .map_err(|e| UntangleError::Io(std::io::Error::new(e.kind(), e.to_string())))?;

    let cli = CliOverrides::default();
    let config = resolve_config(&root, &cli)?;

    if config.services.is_empty() {
        return Err(UntangleError::Config(
            "No [services] configured in .untangle.toml. Add service declarations to use service-graph.".to_string(),
        ));
    }

    let mut service_infos = Vec::new();
    let mut cross_service_edges = Vec::new();

    // Parse GraphQL schemas from all services
    let mut graphql_schemas: Vec<(String, graphql::GraphqlSchema)> = Vec::new();
    // Parse OpenAPI specs from all services
    let mut openapi_services: Vec<(String, Vec<String>, Vec<openapi::OpenApiEndpoint>)> =
        Vec::new();

    for svc in &config.services {
        let svc_root = root.join(&svc.root);

        // Count files for this service
        let file_count = count_service_files(&svc_root, svc);

        service_infos.push(ServiceInfo {
            name: svc.name.clone(),
            root: svc.root.clone(),
            language: svc.lang.map(|l| l.to_string()),
            file_count,
        });

        // Parse GraphQL schemas
        for schema_path in &svc.graphql_schemas {
            let full_path = root.join(schema_path);
            if full_path.exists() {
                match graphql::parse_graphql_schema(&full_path) {
                    Ok(schema) => graphql_schemas.push((svc.name.clone(), schema)),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse GraphQL schema {}: {}",
                            full_path.display(),
                            e
                        );
                    }
                }
            }
        }

        // Parse OpenAPI specs
        let mut endpoints = Vec::new();
        for spec_path in &svc.openapi_specs {
            let full_path = root.join(spec_path);
            if full_path.exists() {
                match openapi::parse_openapi_spec(&full_path) {
                    Ok(spec) => endpoints.extend(spec.endpoints),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse OpenAPI spec {}: {}",
                            full_path.display(),
                            e
                        );
                    }
                }
            }
        }
        if !svc.base_urls.is_empty() || !endpoints.is_empty() {
            openapi_services.push((svc.name.clone(), svc.base_urls.clone(), endpoints));
        }
    }

    // Scan source code in each service for cross-service client usage
    for svc in &config.services {
        let svc_root = root.join(&svc.root);
        if !svc_root.exists() {
            continue;
        }

        let source_files = collect_source_files(&svc_root, svc);

        for file_path in &source_files {
            let source = match std::fs::read(file_path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            // Check for GraphQL client usage
            let gql_usages = graphql_client::detect_graphql_usage(&source, file_path);
            let gql_matches =
                graphql_client::match_usages_to_schemas(&gql_usages, &graphql_schemas);

            for (usage, target_service) in &gql_matches {
                if *target_service != svc.name {
                    cross_service_edges.push(CrossServiceEdge {
                        from_service: svc.name.clone(),
                        to_service: target_service.clone(),
                        kind: "graphql_query".to_string(),
                        operation: usage.operation_name.clone(),
                        source_locations: vec![usage.location.clone()],
                    });
                }
            }

            // Check for REST client usage
            let rest_usages = rest_client::detect_rest_usage(&source, file_path);
            let rest_matches =
                rest_client::match_usages_to_services(&rest_usages, &openapi_services);

            for (usage, target_service, endpoint) in &rest_matches {
                if *target_service != svc.name {
                    cross_service_edges.push(CrossServiceEdge {
                        from_service: svc.name.clone(),
                        to_service: target_service.clone(),
                        kind: "rest_call".to_string(),
                        operation: endpoint.clone(),
                        source_locations: vec![usage.location.clone()],
                    });
                }
            }
        }
    }

    // Deduplicate edges: merge edges with same from/to/kind/operation
    let edges = deduplicate_edges(cross_service_edges);

    let output = ServiceGraphOutput {
        services: service_infos,
        cross_service_edges: edges,
    };

    match args.format {
        OutputFormat::Json => {
            serde_json::to_writer_pretty(std::io::stdout(), &output)?;
            println!();
        }
        OutputFormat::Text => {
            write_service_graph_text(&output);
        }
        OutputFormat::Dot => {
            write_service_graph_dot(&output);
        }
        OutputFormat::Sarif => {
            return Err(UntangleError::Config(
                "SARIF format is not supported for service-graph".to_string(),
            ));
        }
    }

    Ok(())
}

fn count_service_files(svc_root: &Path, svc: &ResolvedService) -> usize {
    if !svc_root.exists() {
        return 0;
    }
    match svc.lang {
        Some(lang) => walk::discover_files(svc_root, lang, &[], &[], false)
            .map(|f| f.len())
            .unwrap_or(0),
        None => walk::discover_files_multi(svc_root, &[], &[], false)
            .map(|m| m.values().map(|v| v.len()).sum())
            .unwrap_or(0),
    }
}

fn collect_source_files(svc_root: &Path, svc: &ResolvedService) -> Vec<PathBuf> {
    match svc.lang {
        Some(lang) => walk::discover_files(svc_root, lang, &[], &[], false).unwrap_or_default(),
        None => walk::discover_files_multi(svc_root, &[], &[], false)
            .map(|m| m.into_values().flatten().collect())
            .unwrap_or_default(),
    }
}

fn deduplicate_edges(edges: Vec<CrossServiceEdge>) -> Vec<CrossServiceEdge> {
    let mut map: HashMap<(String, String, String, Option<String>), CrossServiceEdge> =
        HashMap::new();

    for edge in edges {
        let key = (
            edge.from_service.clone(),
            edge.to_service.clone(),
            edge.kind.clone(),
            edge.operation.clone(),
        );
        map.entry(key)
            .and_modify(|existing| {
                existing
                    .source_locations
                    .extend(edge.source_locations.clone());
            })
            .or_insert(edge);
    }

    map.into_values().collect()
}

fn write_service_graph_text(output: &ServiceGraphOutput) {
    println!("=== Service Graph ===\n");
    println!("Services ({}):", output.services.len());
    for svc in &output.services {
        let lang = svc.language.as_deref().unwrap_or("auto-detect");
        println!(
            "  {} ({}) - {} files at {}",
            svc.name,
            lang,
            svc.file_count,
            svc.root.display()
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
            let op = edge.operation.as_deref().unwrap_or("(unknown)");
            println!(
                "  {} -> {} [{}] {}",
                edge.from_service, edge.to_service, edge.kind, op
            );
            for loc in &edge.source_locations {
                println!("    at {}:{}", loc.file.display(), loc.line);
            }
        }
    }
}

fn write_service_graph_dot(output: &ServiceGraphOutput) {
    println!("digraph service_dependencies {{");
    println!("    rankdir=LR;");
    println!("    node [shape=box, style=filled];");
    println!();

    // Service nodes with language-based colors
    for svc in &output.services {
        let color = match svc.language.as_deref() {
            Some("go") => "lightblue",
            Some("python") => "lightyellow",
            Some("ruby") => "lightcoral",
            Some("rust") => "lightsalmon",
            _ => "white",
        };
        println!(
            "    \"{}\" [label=\"{}\\n({})\\n{} files\", fillcolor={}];",
            svc.name,
            svc.name,
            svc.language.as_deref().unwrap_or("auto"),
            svc.file_count,
            color
        );
    }

    println!();

    // Cross-service edges
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
