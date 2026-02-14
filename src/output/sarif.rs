use crate::errors::Result;
use crate::graph::ir::DepGraph;
use crate::metrics::scc::SccInfo;
use crate::output::json::Metadata;
use serde::Serialize;
use std::io::Write;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifLog {
    #[serde(rename = "$schema")]
    schema: String,
    version: String,
    runs: Vec<SarifRun>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifDriver {
    name: String,
    version: String,
    information_uri: String,
    rules: Vec<SarifRule>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRule {
    id: String,
    name: String,
    short_description: SarifMessage,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifResult {
    rule_id: String,
    level: String,
    message: SarifMessage,
    locations: Vec<SarifLocation>,
}

#[derive(Debug, Serialize)]
struct SarifMessage {
    text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifLocation {
    physical_location: SarifPhysicalLocation,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifPhysicalLocation {
    artifact_location: SarifArtifactLocation,
    region: Option<SarifRegion>,
}

#[derive(Debug, Serialize)]
struct SarifArtifactLocation {
    uri: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRegion {
    start_line: usize,
    start_column: Option<usize>,
}

/// Write analyze output as SARIF 2.1.0.
pub fn write_sarif<W: Write>(
    writer: &mut W,
    graph: &DepGraph,
    sccs: &[SccInfo],
    _metadata: &Metadata,
    threshold_fanout: Option<usize>,
) -> Result<()> {
    let mut results = Vec::new();

    let scc_map = crate::metrics::scc::node_scc_map(graph);

    // Report high fan-out nodes
    for idx in graph.node_indices() {
        let fanout = crate::metrics::fanout::fan_out(graph, idx);
        let threshold = threshold_fanout.unwrap_or(10);
        if fanout >= threshold {
            let node = &graph[idx];
            results.push(SarifResult {
                rule_id: "untangle/high-fanout".to_string(),
                level: "warning".to_string(),
                message: SarifMessage {
                    text: format!(
                        "Module '{}' has fan-out of {} (threshold: {})",
                        node.name, fanout, threshold
                    ),
                },
                locations: vec![SarifLocation {
                    physical_location: SarifPhysicalLocation {
                        artifact_location: SarifArtifactLocation {
                            uri: node.path.to_string_lossy().to_string(),
                        },
                        region: None,
                    },
                }],
            });
        }
    }

    // Report SCC membership
    for idx in graph.node_indices() {
        if let Some(&scc_id) = scc_map.get(&idx) {
            let node = &graph[idx];
            let scc = &sccs[scc_id];
            results.push(SarifResult {
                rule_id: "untangle/circular-dependency".to_string(),
                level: "warning".to_string(),
                message: SarifMessage {
                    text: format!(
                        "Module '{}' is part of a circular dependency (SCC #{}, {} members)",
                        node.name, scc_id, scc.size
                    ),
                },
                locations: vec![SarifLocation {
                    physical_location: SarifPhysicalLocation {
                        artifact_location: SarifArtifactLocation {
                            uri: node.path.to_string_lossy().to_string(),
                        },
                        region: None,
                    },
                }],
            });
        }
    }

    let log = SarifLog {
        schema: "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json".to_string(),
        version: "2.1.0".to_string(),
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "untangle".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    information_uri: "https://github.com/user/untangle".to_string(),
                    rules: vec![
                        SarifRule {
                            id: "untangle/high-fanout".to_string(),
                            name: "HighFanOut".to_string(),
                            short_description: SarifMessage {
                                text: "Module has excessive fan-out (too many dependencies)".to_string(),
                            },
                        },
                        SarifRule {
                            id: "untangle/circular-dependency".to_string(),
                            name: "CircularDependency".to_string(),
                            short_description: SarifMessage {
                                text: "Module is part of a circular dependency".to_string(),
                            },
                        },
                    ],
                },
            },
            results,
        }],
    };

    serde_json::to_writer_pretty(writer, &log)?;
    Ok(())
}
