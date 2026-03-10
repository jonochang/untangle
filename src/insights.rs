use crate::config::{OverrideEntry, ResolvedRules};
use crate::graph::ir::DepGraph;
use crate::metrics::entropy::shannon_entropy;
use crate::metrics::fanout::{fan_in, fan_out};
use crate::metrics::scc::SccInfo;
use crate::metrics::summary::Summary;
use petgraph::Direction;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightSeverity {
    Info,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightCategory {
    GodModule,
    HighFanout,
    CircularDependency,
    DeepChain,
    HighEntropy,
}

#[derive(Debug, Clone, Serialize)]
pub struct Insight {
    pub category: InsightCategory,
    pub severity: InsightSeverity,
    pub module: String,
    pub message: String,
    pub metrics: InsightMetrics,
}

#[derive(Debug, Clone, Serialize)]
pub struct InsightMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fanout: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fanin: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entropy: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scc_id: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scc_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<usize>,
}

#[derive(Debug, Clone)]
struct NodeMetrics {
    name: String,
    file_path: Option<String>,
    fanout: usize,
    fanin: usize,
    entropy: f64,
}

#[derive(Debug, Clone)]
struct EvaluatedNode {
    metrics: NodeMetrics,
    rules: ResolvedRules,
}

pub fn generate_insights(graph: &DepGraph, summary: &Summary, sccs: &[SccInfo]) -> Vec<Insight> {
    generate_insights_internal(
        collect_node_metrics(graph),
        summary,
        sccs,
        &ResolvedRules::default(),
        &[],
    )
}

/// Generate insights using resolved configuration rules and per-path overrides.
pub fn generate_insights_with_config(
    graph: &DepGraph,
    summary: &Summary,
    sccs: &[SccInfo],
    rules: &ResolvedRules,
    overrides: &[(globset::GlobMatcher, OverrideEntry)],
) -> Vec<Insight> {
    generate_insights_internal(collect_node_metrics(graph), summary, sccs, rules, overrides)
}

fn collect_node_metrics(graph: &DepGraph) -> Vec<NodeMetrics> {
    graph
        .node_indices()
        .map(|idx| {
            let edge_weights: Vec<usize> = graph
                .edges_directed(idx, Direction::Outgoing)
                .map(|e| e.weight().weight)
                .collect();
            NodeMetrics {
                name: graph[idx].name.clone(),
                file_path: Some(graph[idx].path.to_string_lossy().to_string()),
                fanout: fan_out(graph, idx),
                fanin: fan_in(graph, idx),
                entropy: shannon_entropy(&edge_weights),
            }
        })
        .collect()
}

fn generate_insights_internal(
    node_metrics: Vec<NodeMetrics>,
    summary: &Summary,
    sccs: &[SccInfo],
    rules: &ResolvedRules,
    overrides: &[(globset::GlobMatcher, OverrideEntry)],
) -> Vec<Insight> {
    let evaluated = evaluate_nodes(node_metrics, rules, overrides);
    let mut insights = Vec::new();
    let mut god_modules: HashSet<String> = HashSet::new();
    insights.extend(evaluate_god_modules(&evaluated, summary, &mut god_modules));
    insights.extend(evaluate_high_fanout(&evaluated, summary, &god_modules));
    insights.extend(evaluate_circular_dependencies(sccs, rules));
    if let Some(insight) = evaluate_deep_chain(summary, rules) {
        insights.push(insight);
    }
    insights.extend(evaluate_high_entropy(&evaluated, &god_modules));
    sort_insights(&mut insights);
    insights
}

fn evaluate_nodes(
    node_metrics: Vec<NodeMetrics>,
    rules: &ResolvedRules,
    overrides: &[(globset::GlobMatcher, OverrideEntry)],
) -> Vec<EvaluatedNode> {
    node_metrics
        .into_iter()
        .filter_map(|metrics| {
            let (effective_rules, enabled) =
                crate::config::overrides::apply_overrides_with_file_path(
                    &metrics.name,
                    metrics.file_path.as_deref(),
                    rules,
                    overrides,
                );
            enabled.then_some(EvaluatedNode {
                metrics,
                rules: effective_rules,
            })
        })
        .collect()
}

fn evaluate_god_modules(
    nodes: &[EvaluatedNode],
    summary: &Summary,
    god_modules: &mut HashSet<String>,
) -> Vec<Insight> {
    let mut insights = Vec::new();
    for node in nodes {
        let rule = &node.rules.god_module;
        if !rule.enabled {
            continue;
        }
        let fo_check = if rule.relative_to_p90 {
            node.metrics.fanout > summary.p90_fanout && node.metrics.fanout >= rule.min_fanout
        } else {
            node.metrics.fanout >= rule.min_fanout
        };
        let fi_check = if rule.relative_to_p90 {
            node.metrics.fanin > summary.p90_fanin && node.metrics.fanin >= rule.min_fanin
        } else {
            node.metrics.fanin >= rule.min_fanin
        };
        if fo_check && fi_check {
            god_modules.insert(node.metrics.name.clone());
            insights.push(build_god_module_insight(&node.metrics));
        }
    }
    insights
}

fn evaluate_high_fanout(
    nodes: &[EvaluatedNode],
    summary: &Summary,
    god_modules: &HashSet<String>,
) -> Vec<Insight> {
    let mut insights = Vec::new();
    for node in nodes {
        if god_modules.contains(&node.metrics.name) {
            continue;
        }
        let rule = &node.rules.high_fanout;
        if !rule.enabled {
            continue;
        }
        let triggers = if rule.relative_to_p90 {
            node.metrics.fanout > summary.p90_fanout && node.metrics.fanout >= rule.min_fanout
        } else {
            node.metrics.fanout >= rule.min_fanout
        };
        if triggers {
            let severity = if node.metrics.fanout >= rule.warning_multiplier * summary.p90_fanout {
                InsightSeverity::Warning
            } else {
                InsightSeverity::Info
            };
            insights.push(build_high_fanout_insight(
                &node.metrics,
                summary.p90_fanout,
                severity,
            ));
        }
    }
    insights
}

fn evaluate_circular_dependencies(sccs: &[SccInfo], rules: &ResolvedRules) -> Vec<Insight> {
    if !rules.circular_dependency.enabled {
        return Vec::new();
    }
    sccs.iter()
        .map(|scc| {
            let severity = if scc.size >= rules.circular_dependency.warning_min_size {
                InsightSeverity::Warning
            } else {
                InsightSeverity::Info
            };
            build_circular_dependency_insight(scc, severity)
        })
        .collect()
}

fn evaluate_deep_chain(summary: &Summary, rules: &ResolvedRules) -> Option<Insight> {
    let rule = &rules.deep_chain;
    if !rule.enabled {
        return None;
    }
    (summary.max_depth >= rule.absolute_depth
        || (summary.max_depth as f64 > rule.relative_multiplier * summary.avg_depth
            && summary.max_depth >= rule.relative_min_depth))
        .then(|| build_deep_chain_insight(summary))
}

fn evaluate_high_entropy(nodes: &[EvaluatedNode], god_modules: &HashSet<String>) -> Vec<Insight> {
    let mut insights = Vec::new();
    for node in nodes {
        if god_modules.contains(&node.metrics.name) {
            continue;
        }
        let rule = &node.rules.high_entropy;
        if !rule.enabled {
            continue;
        }
        if node.metrics.entropy > rule.min_entropy && node.metrics.fanout >= rule.min_fanout {
            insights.push(build_high_entropy_insight(&node.metrics));
        }
    }
    insights
}

fn build_god_module_insight(metrics: &NodeMetrics) -> Insight {
    Insight {
        category: InsightCategory::GodModule,
        severity: InsightSeverity::Warning,
        module: metrics.name.clone(),
        message: format!(
            "Module '{}' has both high fan-out ({}) and high fan-in ({}), \
             suggesting it may be acting as a central hub. \
             Consider decomposing it to reduce coupling.",
            metrics.name, metrics.fanout, metrics.fanin
        ),
        metrics: InsightMetrics {
            fanout: Some(metrics.fanout),
            fanin: Some(metrics.fanin),
            entropy: None,
            scc_id: None,
            scc_size: None,
            depth: None,
        },
    }
}

fn build_high_fanout_insight(
    metrics: &NodeMetrics,
    p90_fanout: usize,
    severity: InsightSeverity,
) -> Insight {
    Insight {
        category: InsightCategory::HighFanout,
        severity,
        module: metrics.name.clone(),
        message: format!(
            "Module '{}' has a fan-out of {} (p90={}). \
             Consider whether it has too many responsibilities \
             and might benefit from being split.",
            metrics.name, metrics.fanout, p90_fanout
        ),
        metrics: InsightMetrics {
            fanout: Some(metrics.fanout),
            fanin: None,
            entropy: None,
            scc_id: None,
            scc_size: None,
            depth: None,
        },
    }
}

fn build_circular_dependency_insight(scc: &SccInfo, severity: InsightSeverity) -> Insight {
    let members_str = scc.members.join(", ");
    Insight {
        category: InsightCategory::CircularDependency,
        severity,
        module: "(graph-level)".to_string(),
        message: format!(
            "Modules {} form a circular dependency (SCC #{}, {} modules). \
             Consider introducing an interface to break this cycle.",
            members_str, scc.id, scc.size
        ),
        metrics: InsightMetrics {
            fanout: None,
            fanin: None,
            entropy: None,
            scc_id: Some(scc.id),
            scc_size: Some(scc.size),
            depth: None,
        },
    }
}

fn build_deep_chain_insight(summary: &Summary) -> Insight {
    Insight {
        category: InsightCategory::DeepChain,
        severity: InsightSeverity::Info,
        module: "(graph-level)".to_string(),
        message: format!(
            "The longest dependency chain is {} levels deep (avg: {:.1}). \
             Deep chains may increase build times. \
             Consider consolidating intermediate modules.",
            summary.max_depth, summary.avg_depth
        ),
        metrics: InsightMetrics {
            fanout: None,
            fanin: None,
            entropy: None,
            scc_id: None,
            scc_size: None,
            depth: Some(summary.max_depth),
        },
    }
}

fn build_high_entropy_insight(metrics: &NodeMetrics) -> Insight {
    Insight {
        category: InsightCategory::HighEntropy,
        severity: InsightSeverity::Info,
        module: metrics.name.clone(),
        message: format!(
            "Module '{}' has high dependency entropy ({:.2}), \
             meaning its {} dependencies are spread broadly. \
             Consider consolidating behind a facade.",
            metrics.name, metrics.entropy, metrics.fanout
        ),
        metrics: InsightMetrics {
            fanout: Some(metrics.fanout),
            fanin: None,
            entropy: Some((metrics.entropy * 100.0).round() / 100.0),
            scc_id: None,
            scc_size: None,
            depth: None,
        },
    }
}

fn sort_insights(insights: &mut [Insight]) {
    insights.sort_by(|a, b| {
        let sev_a = matches!(a.severity, InsightSeverity::Warning);
        let sev_b = matches!(b.severity, InsightSeverity::Warning);
        sev_b
            .cmp(&sev_a)
            .then(a.category.cmp(&b.category))
            .then(a.module.cmp(&b.module))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HighFanoutRule;
    use crate::graph::ir::{EdgeKind, GraphEdge, GraphNode, NodeKind};
    use crate::metrics::scc::find_non_trivial_sccs;
    use crate::metrics::summary::Summary;
    use globset::Glob;
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
    fn no_insights_on_trivial_graph() {
        // 3-node linear chain → empty
        let mut graph = DepGraph::new();
        let a = graph.add_node(make_node("a"));
        let b = graph.add_node(make_node("b"));
        let c = graph.add_node(make_node("c"));
        graph.add_edge(a, b, make_edge());
        graph.add_edge(b, c, make_edge());

        let summary = Summary::from_graph(&graph);
        let sccs = find_non_trivial_sccs(&graph);
        let insights = generate_insights(&graph, &summary, &sccs);
        assert!(
            insights.is_empty(),
            "Expected no insights, got: {insights:?}"
        );
    }

    #[test]
    fn high_fanout_insight() {
        // Hub with fan-out 8, plus extra isolated nodes to keep p90 low.
        // 20 nodes total: fan-outs = [8, 0, 0, ..., 0] (sorted: 19 zeros then 8).
        // p90_idx = ceil(20*0.9)-1 = 17 → p90 = 0.  8 > 0 && 8 >= 5 → triggers.
        let mut graph = DepGraph::new();
        let hub = graph.add_node(make_node("hub"));
        for i in 0..8 {
            let t = graph.add_node(make_node(&format!("t{i}")));
            graph.add_edge(hub, t, make_edge());
        }
        // Add extra isolated nodes to dilute p90
        for i in 0..11 {
            graph.add_node(make_node(&format!("extra{i}")));
        }

        let summary = Summary::from_graph(&graph);
        let sccs = find_non_trivial_sccs(&graph);
        let insights = generate_insights(&graph, &summary, &sccs);

        assert!(
            insights
                .iter()
                .any(|i| i.category == InsightCategory::HighFanout && i.module == "hub"),
            "Expected HighFanout insight for 'hub', got: {insights:?}"
        );
    }

    #[test]
    fn god_module_insight() {
        // Node with high fan-in AND fan-out → GodModule, no separate HighFanout
        let mut graph = DepGraph::new();
        let god = graph.add_node(make_node("god"));

        // High fan-out: god → t0..t7
        for i in 0..8 {
            let t = graph.add_node(make_node(&format!("t{i}")));
            graph.add_edge(god, t, make_edge());
        }
        // High fan-in: s0..s7 → god
        for i in 0..8 {
            let s = graph.add_node(make_node(&format!("s{i}")));
            graph.add_edge(s, god, make_edge());
        }

        let summary = Summary::from_graph(&graph);
        let sccs = find_non_trivial_sccs(&graph);
        let insights = generate_insights(&graph, &summary, &sccs);

        assert!(
            insights
                .iter()
                .any(|i| i.category == InsightCategory::GodModule && i.module == "god"),
            "Expected GodModule insight for 'god'"
        );
        assert!(
            !insights
                .iter()
                .any(|i| i.category == InsightCategory::HighFanout && i.module == "god"),
            "GodModule should supersede HighFanout for 'god'"
        );
        assert!(
            !insights
                .iter()
                .any(|i| i.category == InsightCategory::HighEntropy && i.module == "god"),
            "GodModule should supersede HighEntropy for 'god'"
        );
    }

    #[test]
    fn circular_dependency_insight() {
        // 3-node cycle → one CircularDependency insight
        let mut graph = DepGraph::new();
        let a = graph.add_node(make_node("a"));
        let b = graph.add_node(make_node("b"));
        let c = graph.add_node(make_node("c"));
        graph.add_edge(a, b, make_edge());
        graph.add_edge(b, c, make_edge());
        graph.add_edge(c, a, make_edge());

        let summary = Summary::from_graph(&graph);
        let sccs = find_non_trivial_sccs(&graph);
        let insights = generate_insights(&graph, &summary, &sccs);

        let circ: Vec<_> = insights
            .iter()
            .filter(|i| i.category == InsightCategory::CircularDependency)
            .collect();
        assert_eq!(circ.len(), 1, "Expected one CircularDependency insight");
        assert_eq!(circ[0].severity, InsightSeverity::Info);
    }

    #[test]
    fn large_scc_warning_severity() {
        // 5-node cycle → Warning severity
        let mut graph = DepGraph::new();
        let nodes: Vec<_> = (0..5)
            .map(|i| graph.add_node(make_node(&format!("n{i}"))))
            .collect();
        for i in 0..5 {
            graph.add_edge(nodes[i], nodes[(i + 1) % 5], make_edge());
        }

        let summary = Summary::from_graph(&graph);
        let sccs = find_non_trivial_sccs(&graph);
        let insights = generate_insights(&graph, &summary, &sccs);

        let circ: Vec<_> = insights
            .iter()
            .filter(|i| i.category == InsightCategory::CircularDependency)
            .collect();
        assert_eq!(circ.len(), 1);
        assert_eq!(circ[0].severity, InsightSeverity::Warning);
        assert_eq!(circ[0].metrics.scc_size, Some(5));
    }

    #[test]
    fn high_entropy_insight() {
        // 8 equally-weighted outgoing edges → entropy = log2(8) = 3.0 > 2.5
        let mut graph = DepGraph::new();
        let hub = graph.add_node(make_node("hub"));
        for i in 0..8 {
            let t = graph.add_node(make_node(&format!("t{i}")));
            graph.add_edge(hub, t, make_edge());
        }

        let summary = Summary::from_graph(&graph);
        let sccs = find_non_trivial_sccs(&graph);
        let insights = generate_insights(&graph, &summary, &sccs);

        // hub should have HighEntropy (entropy=3.0, fanout=8)
        // It might also have HighFanout. The key test is HighEntropy is present.
        assert!(
            insights
                .iter()
                .any(|i| i.category == InsightCategory::HighEntropy && i.module == "hub"),
            "Expected HighEntropy insight for 'hub', got: {insights:?}"
        );
    }

    #[test]
    fn deep_chain_insight() {
        // 10-node linear chain → max_depth = 9
        let mut graph = DepGraph::new();
        let nodes: Vec<_> = (0..10)
            .map(|i| graph.add_node(make_node(&format!("n{i}"))))
            .collect();
        for i in 0..9 {
            graph.add_edge(nodes[i], nodes[i + 1], make_edge());
        }

        let summary = Summary::from_graph(&graph);
        let sccs = find_non_trivial_sccs(&graph);
        let insights = generate_insights(&graph, &summary, &sccs);

        let deep: Vec<_> = insights
            .iter()
            .filter(|i| i.category == InsightCategory::DeepChain)
            .collect();
        assert_eq!(deep.len(), 1, "Expected one DeepChain insight");
        assert_eq!(deep[0].metrics.depth, Some(9));
    }

    #[test]
    fn suggestive_language() {
        // Build a graph that triggers multiple insights and verify language
        let mut graph = DepGraph::new();
        let hub = graph.add_node(make_node("hub"));
        for i in 0..8 {
            let t = graph.add_node(make_node(&format!("t{i}")));
            graph.add_edge(hub, t, make_edge());
        }

        let summary = Summary::from_graph(&graph);
        let sccs = find_non_trivial_sccs(&graph);
        let insights = generate_insights(&graph, &summary, &sccs);

        for insight in &insights {
            let msg = &insight.message;
            assert!(
                !msg.contains("broken") && !msg.contains("bad") && !msg.contains("must"),
                "Message should use suggestive language, found definitive: {msg}"
            );
            assert!(
                msg.contains("consider")
                    || msg.contains("Consider")
                    || msg.contains("may")
                    || msg.contains("might")
                    || msg.contains("suggest"),
                "Message should contain suggestive words: {msg}"
            );
        }
    }

    #[test]
    fn config_aware_defaults_match_default_generation() {
        let mut graph = DepGraph::new();
        let hub = graph.add_node(make_node("hub"));
        for i in 0..8 {
            let t = graph.add_node(make_node(&format!("t{i}")));
            graph.add_edge(hub, t, make_edge());
        }
        let summary = Summary::from_graph(&graph);
        let sccs = find_non_trivial_sccs(&graph);

        let direct = generate_insights(&graph, &summary, &sccs);
        let config_aware =
            generate_insights_with_config(&graph, &summary, &sccs, &ResolvedRules::default(), &[]);

        assert_eq!(
            serde_json::to_value(&direct).unwrap(),
            serde_json::to_value(&config_aware).unwrap()
        );
    }

    #[test]
    fn config_aware_overrides_can_disable_matching_file_paths() {
        let mut graph = DepGraph::new();
        let hub = graph.add_node(GraphNode {
            kind: NodeKind::Module,
            path: PathBuf::from("src/vendor/hub.rs"),
            name: "hub".to_string(),
            span: None,
            language: None,
        });
        for i in 0..8 {
            let t = graph.add_node(make_node(&format!("t{i}")));
            graph.add_edge(hub, t, make_edge());
        }
        let summary = Summary::from_graph(&graph);
        let sccs = find_non_trivial_sccs(&graph);
        let overrides = vec![(
            Glob::new("**/vendor/**").unwrap().compile_matcher(),
            OverrideEntry {
                enabled: false,
                rules: None,
            },
        )];

        let insights = generate_insights_with_config(
            &graph,
            &summary,
            &sccs,
            &ResolvedRules::default(),
            &overrides,
        );

        assert!(
            insights.is_empty(),
            "Expected vendor path to be fully skipped"
        );
    }

    #[test]
    fn config_aware_threshold_overrides_change_trigger_behavior() {
        let mut graph = DepGraph::new();
        let hub = graph.add_node(GraphNode {
            kind: NodeKind::Module,
            path: PathBuf::from("src/legacy/hub.rs"),
            name: "hub".to_string(),
            span: None,
            language: None,
        });
        for i in 0..6 {
            let t = graph.add_node(make_node(&format!("t{i}")));
            graph.add_edge(hub, t, make_edge());
        }
        for i in 0..9 {
            graph.add_node(make_node(&format!("extra{i}")));
        }
        let summary = Summary::from_graph(&graph);
        let sccs = find_non_trivial_sccs(&graph);
        let overrides = vec![(
            Glob::new("src/legacy/**").unwrap().compile_matcher(),
            OverrideEntry {
                enabled: true,
                rules: Some(ResolvedRules {
                    high_fanout: HighFanoutRule {
                        enabled: true,
                        min_fanout: 10,
                        relative_to_p90: false,
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            },
        )];

        let insights = generate_insights_with_config(
            &graph,
            &summary,
            &sccs,
            &ResolvedRules::default(),
            &overrides,
        );

        assert!(
            !insights
                .iter()
                .any(|insight| insight.category == InsightCategory::HighFanout),
            "Expected override threshold to suppress high-fanout insight"
        );
    }
}
