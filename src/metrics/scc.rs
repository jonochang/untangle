use crate::graph::ir::DepGraph;
use petgraph::algo::tarjan_scc;
use petgraph::visit::EdgeRef;
use serde::Serialize;

/// Information about a strongly connected component.
#[derive(Debug, Clone, Serialize)]
pub struct SccInfo {
    pub id: usize,
    pub size: usize,
    pub members: Vec<String>,
    pub internal_edges: usize,
}

/// Compute all non-trivial SCCs (size > 1 or self-loop) in the graph.
pub fn find_non_trivial_sccs(graph: &DepGraph) -> Vec<SccInfo> {
    let sccs = tarjan_scc(graph);
    let mut result = Vec::new();
    let mut id = 0;

    for scc in sccs {
        let is_self_loop = scc.len() == 1 && {
            let node = scc[0];
            graph
                .edges_directed(node, petgraph::Direction::Outgoing)
                .any(|e| e.target() == node)
        };

        if scc.len() <= 1 && !is_self_loop {
            continue;
        }

        let members: Vec<String> = scc.iter().map(|&idx| graph[idx].name.clone()).collect();

        // Count internal edges (edges where both endpoints are in this SCC)
        let scc_set: std::collections::HashSet<_> = scc.iter().copied().collect();
        let mut internal_edges = 0;
        for &node in &scc {
            for edge in graph.edges_directed(node, petgraph::Direction::Outgoing) {
                if scc_set.contains(&edge.target()) {
                    internal_edges += 1;
                }
            }
        }

        result.push(SccInfo {
            id,
            size: scc.len(),
            members,
            internal_edges,
        });
        id += 1;
    }

    result
}

/// Return the SCC id for each node (None if in a trivial SCC).
pub fn node_scc_map(
    graph: &DepGraph,
) -> std::collections::HashMap<petgraph::graph::NodeIndex, usize> {
    let sccs = tarjan_scc(graph);
    let mut map = std::collections::HashMap::new();
    let mut id = 0;

    for scc in sccs {
        let is_self_loop = scc.len() == 1 && {
            let node = scc[0];
            graph
                .edges_directed(node, petgraph::Direction::Outgoing)
                .any(|e| e.target() == node)
        };

        if scc.len() <= 1 && !is_self_loop {
            continue;
        }
        for &node in &scc {
            map.insert(node, id);
        }
        id += 1;
    }

    map
}

/// Return the SCC size for a node (1 if not in a non-trivial SCC).
pub fn node_scc_size(graph: &DepGraph, node: petgraph::graph::NodeIndex) -> usize {
    let sccs = tarjan_scc(graph);
    for scc in sccs {
        if scc.contains(&node) {
            return scc.len();
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::ir::{EdgeKind, GraphEdge, GraphNode, NodeKind};
    use std::path::PathBuf;

    fn make_node(name: &str) -> GraphNode {
        GraphNode {
            kind: NodeKind::Module,
            path: PathBuf::from(name),
            name: name.to_string(),
            span: None,
            language: None,
        }
    }

    fn make_edge() -> GraphEdge {
        GraphEdge {
            kind: EdgeKind::default(),
            source_locations: vec![],
            weight: 1,
        }
    }

    #[test]
    fn no_cycles_no_sccs() {
        let mut graph = DepGraph::new();
        let a = graph.add_node(make_node("a"));
        let b = graph.add_node(make_node("b"));
        graph.add_edge(a, b, make_edge());
        let sccs = find_non_trivial_sccs(&graph);
        assert!(sccs.is_empty());
    }

    #[test]
    fn simple_cycle_detected() {
        let mut graph = DepGraph::new();
        let a = graph.add_node(make_node("a"));
        let b = graph.add_node(make_node("b"));
        let c = graph.add_node(make_node("c"));
        graph.add_edge(a, b, make_edge());
        graph.add_edge(b, c, make_edge());
        graph.add_edge(c, a, make_edge());
        let sccs = find_non_trivial_sccs(&graph);
        assert_eq!(sccs.len(), 1);
        assert_eq!(sccs[0].size, 3);
        assert_eq!(sccs[0].internal_edges, 3);
    }

    #[test]
    fn two_separate_sccs() {
        let mut graph = DepGraph::new();
        let a = graph.add_node(make_node("a"));
        let b = graph.add_node(make_node("b"));
        let c = graph.add_node(make_node("c"));
        let d = graph.add_node(make_node("d"));
        // SCC 1: a <-> b
        graph.add_edge(a, b, make_edge());
        graph.add_edge(b, a, make_edge());
        // SCC 2: c <-> d
        graph.add_edge(c, d, make_edge());
        graph.add_edge(d, c, make_edge());
        let sccs = find_non_trivial_sccs(&graph);
        assert_eq!(sccs.len(), 2);
    }
}
