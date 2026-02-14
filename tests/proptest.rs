use petgraph::graph::DiGraph;
use proptest::prelude::*;
use std::path::PathBuf;

// Import types from our crate
type DepGraph = DiGraph<GraphNode, GraphEdge>;

#[derive(Debug, Clone)]
struct GraphNode {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    path: PathBuf,
}

#[derive(Debug, Clone)]
struct GraphEdge {
    #[allow(dead_code)]
    weight: usize,
}

fn build_graph_from_edges(edges: &[(usize, usize)], max_nodes: usize) -> DepGraph {
    let mut graph = DepGraph::new();
    let mut node_indices = Vec::new();

    for i in 0..max_nodes {
        node_indices.push(graph.add_node(GraphNode {
            name: format!("mod_{i}"),
            path: PathBuf::from(format!("mod_{i}.py")),
        }));
    }

    for &(from, to) in edges {
        if from < max_nodes && to < max_nodes && from != to {
            // Avoid duplicate edges
            let from_idx = node_indices[from];
            let to_idx = node_indices[to];
            if graph.find_edge(from_idx, to_idx).is_none() {
                graph.add_edge(from_idx, to_idx, GraphEdge { weight: 1 });
            }
        }
    }

    graph
}

fn shannon_entropy(edge_weights: &[usize]) -> f64 {
    let total: f64 = edge_weights.iter().sum::<usize>() as f64;
    if total == 0.0 {
        return 0.0;
    }
    edge_weights
        .iter()
        .filter(|&&w| w > 0)
        .map(|&w| {
            let p = w as f64 / total;
            -p * p.log2()
        })
        .sum()
}

proptest! {
    #[test]
    fn fanout_equals_out_degree(
        edges in prop::collection::vec((0usize..50, 0usize..50), 0..200)
    ) {
        let graph = build_graph_from_edges(&edges, 50);
        for node in graph.node_indices() {
            let computed_fanout = graph.edges_directed(node, petgraph::Direction::Outgoing).count();
            let actual_out_degree = graph.edges_directed(node, petgraph::Direction::Outgoing).count();
            prop_assert_eq!(computed_fanout, actual_out_degree);
        }
    }

    #[test]
    fn entropy_is_non_negative(weights in prop::collection::vec(1usize..100, 1..20)) {
        let h = shannon_entropy(&weights);
        prop_assert!(h >= 0.0, "entropy was {}", h);
    }

    #[test]
    fn entropy_bounded_by_log_n(weights in prop::collection::vec(1usize..100, 1..20)) {
        let h = shannon_entropy(&weights);
        let max_h = (weights.len() as f64).log2();
        prop_assert!(h <= max_h + 1e-10, "entropy {} exceeds max {} for {} weights", h, max_h, weights.len());
    }

    #[test]
    fn entropy_zero_for_single_weight(w in 1usize..1000) {
        let h = shannon_entropy(&[w]);
        prop_assert!((h - 0.0).abs() < 1e-10, "entropy of single weight should be 0, got {}", h);
    }

    #[test]
    fn entropy_maximized_for_uniform(n in 2usize..20) {
        let weights: Vec<usize> = vec![1; n];
        let h = shannon_entropy(&weights);
        let expected = (n as f64).log2();
        prop_assert!((h - expected).abs() < 1e-10, "uniform entropy should be log2({}), got {}", n, h);
    }

    #[test]
    fn scc_members_have_paths_between_them(
        edges in prop::collection::vec((0usize..30, 0usize..30), 0..100)
    ) {
        let graph = build_graph_from_edges(&edges, 30);
        let sccs = petgraph::algo::tarjan_scc(&graph);
        for scc in &sccs {
            if scc.len() > 1 {
                // Every pair in SCC should be mutually reachable
                for &a in scc {
                    for &b in scc {
                        if a != b {
                            prop_assert!(
                                petgraph::algo::has_path_connecting(&graph, a, b, None),
                                "nodes {:?} and {:?} in same SCC but no path from a to b", a, b
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fan_in_sum_equals_edge_count(
        edges in prop::collection::vec((0usize..30, 0usize..30), 0..100)
    ) {
        let graph = build_graph_from_edges(&edges, 30);
        let total_fan_in: usize = graph
            .node_indices()
            .map(|n| graph.edges_directed(n, petgraph::Direction::Incoming).count())
            .sum();
        prop_assert_eq!(total_fan_in, graph.edge_count());
    }

    #[test]
    fn fan_out_sum_equals_edge_count(
        edges in prop::collection::vec((0usize..30, 0usize..30), 0..100)
    ) {
        let graph = build_graph_from_edges(&edges, 30);
        let total_fan_out: usize = graph
            .node_indices()
            .map(|n| graph.edges_directed(n, petgraph::Direction::Outgoing).count())
            .sum();
        prop_assert_eq!(total_fan_out, graph.edge_count());
    }
}
