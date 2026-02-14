use crate::errors::Result;
use crate::graph::diff::DiffResult;
use crate::graph::ir::DepGraph;
use crate::metrics::scc::SccInfo;
use crate::metrics::summary::Summary;
use crate::parse::common::SourceLocation;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::Serialize;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct AnalyzeOutput {
    pub metadata: Metadata,
    pub summary: Summary,
    pub hotspots: Vec<Hotspot>,
    pub sccs: Vec<SccInfo>,
}

#[derive(Debug, Serialize)]
pub struct Metadata {
    pub language: String,
    pub granularity: String,
    pub root: PathBuf,
    pub node_count: usize,
    pub edge_count: usize,
    pub edge_density: f64,
    pub files_parsed: usize,
    pub files_skipped: usize,
    pub unresolved_imports: usize,
    pub timestamp: String,
    pub elapsed_ms: u64,
    pub modules_per_second: f64,
}

#[derive(Debug, Serialize)]
pub struct Hotspot {
    pub node: String,
    pub fanout: usize,
    pub fanin: usize,
    pub entropy: f64,
    pub scc_id: Option<usize>,
    pub scc_adjusted_entropy: f64,
    pub fanout_edges: Vec<FanoutEdge>,
}

#[derive(Debug, Serialize)]
pub struct FanoutEdge {
    pub to: String,
    pub source_locations: Vec<SourceLocation>,
}

/// Write analyze output as JSON.
pub fn write_analyze_json<W: Write>(
    writer: &mut W,
    graph: &DepGraph,
    summary: &Summary,
    sccs: &[SccInfo],
    metadata: Metadata,
    top_n: Option<usize>,
) -> Result<()> {
    let scc_map = crate::metrics::scc::node_scc_map(graph);

    let mut hotspots: Vec<Hotspot> = graph
        .node_indices()
        .map(|idx| {
            let node = &graph[idx];
            let fanout = crate::metrics::fanout::fan_out(graph, idx);
            let fanin = crate::metrics::fanout::fan_in(graph, idx);

            // Collect outgoing edge weights for entropy
            let edge_weights: Vec<usize> = graph
                .edges_directed(idx, Direction::Outgoing)
                .map(|e| e.weight().weight)
                .collect();
            let entropy = crate::metrics::entropy::shannon_entropy(&edge_weights);

            let scc_id = scc_map.get(&idx).copied();
            let scc_size = scc_id
                .and_then(|id| sccs.iter().find(|s| s.id == id))
                .map(|s| s.size)
                .unwrap_or(1);
            let scc_adjusted = crate::metrics::entropy::scc_adjusted_entropy(entropy, scc_size);

            let fanout_edges: Vec<FanoutEdge> = graph
                .edges_directed(idx, Direction::Outgoing)
                .map(|e| {
                    let target = &graph[e.target()];
                    FanoutEdge {
                        to: target.name.clone(),
                        source_locations: e.weight().source_locations.clone(),
                    }
                })
                .collect();

            Hotspot {
                node: node.name.clone(),
                fanout,
                fanin,
                entropy: (entropy * 100.0).round() / 100.0,
                scc_id,
                scc_adjusted_entropy: (scc_adjusted * 100.0).round() / 100.0,
                fanout_edges,
            }
        })
        .collect();

    // Sort by fan-out descending
    hotspots.sort_by(|a, b| b.fanout.cmp(&a.fanout).then(b.fanin.cmp(&a.fanin)));

    if let Some(n) = top_n {
        hotspots.truncate(n);
    }

    let output = AnalyzeOutput {
        metadata,
        summary: summary.clone(),
        hotspots,
        sccs: sccs.to_vec(),
    };

    serde_json::to_writer_pretty(writer, &output)?;
    Ok(())
}

/// Write diff output as JSON.
pub fn write_diff_json<W: Write>(writer: &mut W, diff: &DiffResult) -> Result<()> {
    serde_json::to_writer_pretty(writer, diff)?;
    Ok(())
}
