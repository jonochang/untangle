use crate::graph::ir::DepGraph;
use petgraph::algo::condensation;
use petgraph::visit::EdgeRef;
use petgraph::Direction;

/// Compute depth metrics from the condensation DAG.
/// Returns (max_depth, avg_depth).
fn depth_metrics(graph: &DepGraph) -> (usize, f64) {
    let dag = condensation(graph.clone(), true);
    let n = dag.node_count();
    if n <= 1 {
        return (0, 0.0);
    }

    // Compute longest path FROM each node using reverse topological order.
    // dist_from[u] = length of longest path starting at u.
    let topo = reverse_topo_order(&dag);
    let mut dist_from = vec![0usize; n];

    for &u in &topo {
        for edge in dag.edges_directed(u, Direction::Outgoing) {
            let v = edge.target();
            let candidate = dist_from[v.index()] + 1;
            if candidate > dist_from[u.index()] {
                dist_from[u.index()] = candidate;
            }
        }
    }

    let max_depth = dist_from.iter().copied().max().unwrap_or(0);

    // Source nodes: in-degree = 0 in the condensation DAG
    let sources: Vec<_> = dag
        .node_indices()
        .filter(|&v| dag.edges_directed(v, Direction::Incoming).next().is_none())
        .collect();

    let avg_depth = if sources.is_empty() {
        0.0
    } else {
        let total: usize = sources.iter().map(|&s| dist_from[s.index()]).sum();
        let avg = total as f64 / sources.len() as f64;
        (avg * 100.0).round() / 100.0
    };

    (max_depth, avg_depth)
}

/// Compute the longest path in the graph after collapsing SCCs into single nodes.
/// Returns 0 for empty graphs or single-node graphs.
pub fn max_depth(graph: &DepGraph) -> usize {
    depth_metrics(graph).0
}

/// Compute the average longest path from each source node (in-degree=0) in the condensation DAG.
/// Returns 0.0 for empty graphs.
pub fn avg_depth(graph: &DepGraph) -> f64 {
    depth_metrics(graph).1
}

/// Reverse topological order via Kahn's algorithm on the reversed graph.
/// Nodes are returned such that for each edge u→v, v appears before u.
fn reverse_topo_order<N, E>(dag: &petgraph::Graph<N, E>) -> Vec<petgraph::graph::NodeIndex> {
    let n = dag.node_count();
    let mut out_degree = vec![0usize; n];
    for edge in dag.edge_references() {
        out_degree[edge.source().index()] += 1;
    }

    let mut queue: std::collections::VecDeque<petgraph::graph::NodeIndex> =
        std::collections::VecDeque::new();
    for node in dag.node_indices() {
        if out_degree[node.index()] == 0 {
            queue.push_back(node);
        }
    }

    let mut order = Vec::with_capacity(n);
    while let Some(u) = queue.pop_front() {
        order.push(u);
        for edge in dag.edges_directed(u, Direction::Incoming) {
            let v = edge.source();
            out_degree[v.index()] -= 1;
            if out_degree[v.index()] == 0 {
                queue.push_back(v);
            }
        }
    }

    order
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
    fn empty_graph_depth_zero() {
        let graph = DepGraph::new();
        assert_eq!(max_depth(&graph), 0);
        assert_eq!(avg_depth(&graph), 0.0);
    }

    #[test]
    fn single_node_depth_zero() {
        let mut graph = DepGraph::new();
        graph.add_node(make_node("a"));
        assert_eq!(max_depth(&graph), 0);
        assert_eq!(avg_depth(&graph), 0.0);
    }

    #[test]
    fn linear_chain_depth() {
        // A→B→C: max_depth = 2
        let mut graph = DepGraph::new();
        let a = graph.add_node(make_node("a"));
        let b = graph.add_node(make_node("b"));
        let c = graph.add_node(make_node("c"));
        graph.add_edge(a, b, make_edge());
        graph.add_edge(b, c, make_edge());
        assert_eq!(max_depth(&graph), 2);
        assert_eq!(avg_depth(&graph), 2.0);
    }

    #[test]
    fn diamond_graph_depth() {
        // A→B, A→C, B→D, C→D: max_depth = 2
        let mut graph = DepGraph::new();
        let a = graph.add_node(make_node("a"));
        let b = graph.add_node(make_node("b"));
        let c = graph.add_node(make_node("c"));
        let d = graph.add_node(make_node("d"));
        graph.add_edge(a, b, make_edge());
        graph.add_edge(a, c, make_edge());
        graph.add_edge(b, d, make_edge());
        graph.add_edge(c, d, make_edge());
        assert_eq!(max_depth(&graph), 2);
        assert_eq!(avg_depth(&graph), 2.0);
    }

    #[test]
    fn graph_with_cycle() {
        // A→B→C→A (SCC), C→D
        // After condensation: [A,B,C] → D, max_depth = 1
        let mut graph = DepGraph::new();
        let a = graph.add_node(make_node("a"));
        let b = graph.add_node(make_node("b"));
        let c = graph.add_node(make_node("c"));
        let d = graph.add_node(make_node("d"));
        graph.add_edge(a, b, make_edge());
        graph.add_edge(b, c, make_edge());
        graph.add_edge(c, a, make_edge());
        graph.add_edge(c, d, make_edge());
        assert_eq!(max_depth(&graph), 1);
        assert_eq!(avg_depth(&graph), 1.0);
    }

    #[test]
    fn wide_fan_out_depth() {
        // A→B, A→C, A→D: max_depth = 1
        let mut graph = DepGraph::new();
        let a = graph.add_node(make_node("a"));
        let b = graph.add_node(make_node("b"));
        let c = graph.add_node(make_node("c"));
        let d = graph.add_node(make_node("d"));
        graph.add_edge(a, b, make_edge());
        graph.add_edge(a, c, make_edge());
        graph.add_edge(a, d, make_edge());
        assert_eq!(max_depth(&graph), 1);
        assert_eq!(avg_depth(&graph), 1.0);
    }
}
