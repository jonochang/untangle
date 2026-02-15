use petgraph::Direction;

use crate::graph::ir::DepGraph;

/// Compute fan-out (out-degree) for a node.
pub fn fan_out(graph: &DepGraph, node: petgraph::graph::NodeIndex) -> usize {
    graph.edges_directed(node, Direction::Outgoing).count()
}

/// Compute fan-in (in-degree) for a node.
pub fn fan_in(graph: &DepGraph, node: petgraph::graph::NodeIndex) -> usize {
    graph.edges_directed(node, Direction::Incoming).count()
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
    fn fan_out_counts_outgoing_edges() {
        let mut graph = DepGraph::new();
        let a = graph.add_node(make_node("a"));
        let b = graph.add_node(make_node("b"));
        let c = graph.add_node(make_node("c"));
        graph.add_edge(a, b, make_edge());
        graph.add_edge(a, c, make_edge());
        assert_eq!(fan_out(&graph, a), 2);
        assert_eq!(fan_out(&graph, b), 0);
    }

    #[test]
    fn fan_in_counts_incoming_edges() {
        let mut graph = DepGraph::new();
        let a = graph.add_node(make_node("a"));
        let b = graph.add_node(make_node("b"));
        let c = graph.add_node(make_node("c"));
        graph.add_edge(a, c, make_edge());
        graph.add_edge(b, c, make_edge());
        assert_eq!(fan_in(&graph, c), 2);
        assert_eq!(fan_in(&graph, a), 0);
    }
}
