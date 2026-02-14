use crate::graph::ir::DepGraph;
use crate::metrics::fanout::{fan_in, fan_out};
use crate::metrics::scc::find_non_trivial_sccs;
use serde::Serialize;

/// Aggregate statistics for a dependency graph.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub mean_fanout: f64,
    pub p90_fanout: usize,
    pub max_fanout: usize,
    pub mean_fanin: f64,
    pub p90_fanin: usize,
    pub max_fanin: usize,
    pub scc_count: usize,
    pub largest_scc_size: usize,
    pub total_nodes_in_sccs: usize,
}

impl Summary {
    /// Compute summary statistics from a dependency graph.
    pub fn from_graph(graph: &DepGraph) -> Self {
        let node_count = graph.node_count();
        if node_count == 0 {
            return Self {
                mean_fanout: 0.0,
                p90_fanout: 0,
                max_fanout: 0,
                mean_fanin: 0.0,
                p90_fanin: 0,
                max_fanin: 0,
                scc_count: 0,
                largest_scc_size: 0,
                total_nodes_in_sccs: 0,
            };
        }

        let mut fanouts: Vec<usize> = graph.node_indices().map(|n| fan_out(graph, n)).collect();
        let mut fanins: Vec<usize> = graph.node_indices().map(|n| fan_in(graph, n)).collect();

        fanouts.sort_unstable();
        fanins.sort_unstable();

        let mean_fanout = fanouts.iter().sum::<usize>() as f64 / node_count as f64;
        let mean_fanin = fanins.iter().sum::<usize>() as f64 / node_count as f64;

        let p90_idx = (node_count as f64 * 0.9).ceil() as usize;
        let p90_idx = p90_idx.min(node_count) - 1;

        let sccs = find_non_trivial_sccs(graph);
        let scc_count = sccs.len();
        let largest_scc_size = sccs.iter().map(|s| s.size).max().unwrap_or(0);
        let total_nodes_in_sccs: usize = sccs.iter().map(|s| s.size).sum();

        Self {
            mean_fanout: (mean_fanout * 100.0).round() / 100.0,
            p90_fanout: fanouts[p90_idx],
            max_fanout: *fanouts.last().unwrap_or(&0),
            mean_fanin: (mean_fanin * 100.0).round() / 100.0,
            p90_fanin: fanins[p90_idx],
            max_fanin: *fanins.last().unwrap_or(&0),
            scc_count,
            largest_scc_size,
            total_nodes_in_sccs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::ir::{GraphEdge, GraphNode, NodeKind};
    use std::path::PathBuf;

    fn make_node(name: &str) -> GraphNode {
        GraphNode {
            kind: NodeKind::Module,
            path: PathBuf::from(name),
            name: name.to_string(),
            span: None,
        }
    }

    fn make_edge() -> GraphEdge {
        GraphEdge {
            source_locations: vec![],
            weight: 1,
        }
    }

    #[test]
    fn summary_empty_graph() {
        let graph = DepGraph::new();
        let summary = Summary::from_graph(&graph);
        assert_eq!(summary.mean_fanout, 0.0);
        assert_eq!(summary.scc_count, 0);
    }

    #[test]
    fn summary_linear_graph() {
        let mut graph = DepGraph::new();
        let a = graph.add_node(make_node("a"));
        let b = graph.add_node(make_node("b"));
        let c = graph.add_node(make_node("c"));
        graph.add_edge(a, b, make_edge());
        graph.add_edge(b, c, make_edge());
        let summary = Summary::from_graph(&graph);
        assert_eq!(summary.max_fanout, 1);
        assert_eq!(summary.max_fanin, 1);
        assert_eq!(summary.scc_count, 0);
    }
}
