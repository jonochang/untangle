use crate::config::ResolvedService;
use crate::errors::Result;
use crate::parse::common::SourceLocation;
use crate::parse::graphql;
use crate::parse::graphql_client;
use crate::parse::openapi;
use crate::parse::rest_client;
use crate::walk;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct CrossServiceEdge {
    pub from_service: String,
    pub to_service: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    pub source_locations: Vec<SourceLocation>,
}

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

pub fn analyze(root: &Path, services: &[ResolvedService]) -> Result<ServiceGraphOutput> {
    let mut service_infos = Vec::new();
    let mut cross_service_edges = Vec::new();
    let mut graphql_schemas: Vec<(String, graphql::GraphqlSchema)> = Vec::new();
    let mut openapi_services: Vec<(String, Vec<String>, Vec<openapi::OpenApiEndpoint>)> =
        Vec::new();

    for service in services {
        let service_root = root.join(&service.root);
        service_infos.push(ServiceInfo {
            name: service.name.clone(),
            root: service.root.clone(),
            language: service.lang.map(|lang| lang.to_string()),
            file_count: count_service_files(&service_root, service),
        });

        graphql_schemas.extend(load_graphql_schemas(root, service));

        let endpoints = load_openapi_endpoints(root, service);
        if !service.base_urls.is_empty() || !endpoints.is_empty() {
            openapi_services.push((service.name.clone(), service.base_urls.clone(), endpoints));
        }
    }

    for service in services {
        let service_root = root.join(&service.root);
        if !service_root.exists() {
            continue;
        }

        for file_path in collect_source_files(&service_root, service) {
            let source = match std::fs::read(&file_path) {
                Ok(source) => source,
                Err(_) => continue,
            };

            let graphql_edges = graphql_client::match_usages_to_schemas(
                &graphql_client::detect_graphql_usage(&source, &file_path),
                &graphql_schemas,
            );
            for (usage, target_service) in &graphql_edges {
                if *target_service != service.name {
                    cross_service_edges.push(CrossServiceEdge {
                        from_service: service.name.clone(),
                        to_service: target_service.clone(),
                        kind: "graphql_query".to_string(),
                        operation: usage.operation_name.clone(),
                        source_locations: vec![usage.location.clone()],
                    });
                }
            }

            let rest_edges = rest_client::match_usages_to_services(
                &rest_client::detect_rest_usage(&source, &file_path),
                &openapi_services,
            );
            for (usage, target_service, endpoint) in &rest_edges {
                if *target_service != service.name {
                    cross_service_edges.push(CrossServiceEdge {
                        from_service: service.name.clone(),
                        to_service: target_service.clone(),
                        kind: "rest_call".to_string(),
                        operation: endpoint.clone(),
                        source_locations: vec![usage.location.clone()],
                    });
                }
            }
        }
    }

    Ok(ServiceGraphOutput {
        services: service_infos,
        cross_service_edges: deduplicate_edges(cross_service_edges),
    })
}

fn load_graphql_schemas(
    root: &Path,
    service: &ResolvedService,
) -> Vec<(String, graphql::GraphqlSchema)> {
    let mut schemas = Vec::new();
    for schema_path in &service.graphql_schemas {
        let full_path = root.join(schema_path);
        if !full_path.exists() {
            continue;
        }

        match graphql::parse_graphql_schema(&full_path) {
            Ok(schema) => schemas.push((service.name.clone(), schema)),
            Err(error) => {
                tracing::warn!(
                    "Failed to parse GraphQL schema {}: {}",
                    full_path.display(),
                    error
                );
            }
        }
    }
    schemas
}

fn load_openapi_endpoints(root: &Path, service: &ResolvedService) -> Vec<openapi::OpenApiEndpoint> {
    let mut endpoints = Vec::new();
    for spec_path in &service.openapi_specs {
        let full_path = root.join(spec_path);
        if !full_path.exists() {
            continue;
        }

        match openapi::parse_openapi_spec(&full_path) {
            Ok(spec) => endpoints.extend(spec.endpoints),
            Err(error) => {
                tracing::warn!(
                    "Failed to parse OpenAPI spec {}: {}",
                    full_path.display(),
                    error
                );
            }
        }
    }
    endpoints
}

fn count_service_files(service_root: &Path, service: &ResolvedService) -> usize {
    if !service_root.exists() {
        return 0;
    }

    match service.lang {
        Some(lang) => walk::discover_files(service_root, lang, &[], &[], false)
            .map(|files| files.len())
            .unwrap_or(0),
        None => walk::discover_files_multi(service_root, &[], &[], false)
            .map(|files| files.values().map(|entries| entries.len()).sum())
            .unwrap_or(0),
    }
}

fn collect_source_files(service_root: &Path, service: &ResolvedService) -> Vec<PathBuf> {
    match service.lang {
        Some(lang) => walk::discover_files(service_root, lang, &[], &[], false).unwrap_or_default(),
        None => walk::discover_files_multi(service_root, &[], &[], false)
            .map(|files| files.into_values().flatten().collect())
            .unwrap_or_default(),
    }
}

fn deduplicate_edges(edges: Vec<CrossServiceEdge>) -> Vec<CrossServiceEdge> {
    let mut deduped: HashMap<(String, String, String, Option<String>), CrossServiceEdge> =
        HashMap::new();

    for edge in edges {
        let key = (
            edge.from_service.clone(),
            edge.to_service.clone(),
            edge.kind.clone(),
            edge.operation.clone(),
        );
        deduped
            .entry(key)
            .and_modify(|existing| {
                existing
                    .source_locations
                    .extend(edge.source_locations.clone());
            })
            .or_insert(edge);
    }

    deduped.into_values().collect()
}
