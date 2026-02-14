use criterion::{black_box, criterion_group, criterion_main, Criterion};
use petgraph::graph::DiGraph;

fn build_linear_graph(n: usize) -> DiGraph<String, usize> {
    let mut graph = DiGraph::new();
    let mut indices = Vec::new();
    for i in 0..n {
        indices.push(graph.add_node(format!("mod_{i}")));
    }
    for i in 0..n.saturating_sub(1) {
        graph.add_edge(indices[i], indices[i + 1], 1);
    }
    graph
}

fn build_dense_graph(n: usize) -> DiGraph<String, usize> {
    let mut graph = DiGraph::new();
    let mut indices = Vec::new();
    for i in 0..n {
        indices.push(graph.add_node(format!("mod_{i}")));
    }
    for i in 0..n {
        for j in 0..n {
            if i != j {
                graph.add_edge(indices[i], indices[j], 1);
            }
        }
    }
    graph
}

fn shannon_entropy(weights: &[usize]) -> f64 {
    let total: f64 = weights.iter().sum::<usize>() as f64;
    if total == 0.0 {
        return 0.0;
    }
    weights
        .iter()
        .filter(|&&w| w > 0)
        .map(|&w| {
            let p = w as f64 / total;
            -p * p.log2()
        })
        .sum()
}

fn bench_scc_linear(c: &mut Criterion) {
    let graph = build_linear_graph(500);
    c.bench_function("scc_linear_500", |b| {
        b.iter(|| {
            let sccs = petgraph::algo::tarjan_scc(black_box(&graph));
            black_box(sccs.len())
        })
    });
}

fn bench_scc_dense(c: &mut Criterion) {
    let graph = build_dense_graph(100);
    c.bench_function("scc_dense_100", |b| {
        b.iter(|| {
            let sccs = petgraph::algo::tarjan_scc(black_box(&graph));
            black_box(sccs.len())
        })
    });
}

fn bench_entropy_computation(c: &mut Criterion) {
    let weights: Vec<usize> = (1..=1000).collect();
    c.bench_function("entropy_1000_weights", |b| {
        b.iter(|| black_box(shannon_entropy(black_box(&weights))))
    });
}

fn bench_graph_build_1000(c: &mut Criterion) {
    c.bench_function("graph_build_1000_nodes", |b| {
        b.iter(|| black_box(build_linear_graph(1000)))
    });
}

criterion_group!(
    benches,
    bench_scc_linear,
    bench_scc_dense,
    bench_entropy_computation,
    bench_graph_build_1000
);
criterion_main!(benches);
