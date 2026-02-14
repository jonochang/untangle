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

pub fn generate_insights(graph: &DepGraph, summary: &Summary, sccs: &[SccInfo]) -> Vec<Insight> {
    let mut insights = Vec::new();

    // Track modules that qualify as god modules so we can dedup
    let mut god_modules: HashSet<String> = HashSet::new();

    // Collect per-node metrics
    let node_metrics: Vec<_> = graph
        .node_indices()
        .map(|idx| {
            let name = graph[idx].name.clone();
            let fo = fan_out(graph, idx);
            let fi = fan_in(graph, idx);
            let edge_weights: Vec<usize> = graph
                .edges_directed(idx, Direction::Outgoing)
                .map(|e| e.weight().weight)
                .collect();
            let entropy = shannon_entropy(&edge_weights);
            (name, fo, fi, entropy)
        })
        .collect();

    // God Module: high fan-out AND high fan-in, both above p90 and both >= 3
    for (name, fo, fi, _entropy) in &node_metrics {
        if *fo > summary.p90_fanout && *fi > summary.p90_fanin && *fo >= 3 && *fi >= 3 {
            god_modules.insert(name.clone());
            insights.push(Insight {
                category: InsightCategory::GodModule,
                severity: InsightSeverity::Warning,
                module: name.clone(),
                message: format!(
                    "Module '{}' has both high fan-out ({}) and high fan-in ({}), \
                     suggesting it may be acting as a central hub. \
                     Consider decomposing it to reduce coupling.",
                    name, fo, fi
                ),
                metrics: InsightMetrics {
                    fanout: Some(*fo),
                    fanin: Some(*fi),
                    entropy: None,
                    scc_id: None,
                    scc_size: None,
                    depth: None,
                },
            });
        }
    }

    // High Fan-out: fanout > p90 and fanout >= 5, skip god modules
    for (name, fo, _fi, _entropy) in &node_metrics {
        if god_modules.contains(name) {
            continue;
        }
        if *fo > summary.p90_fanout && *fo >= 5 {
            let severity = if *fo >= 2 * summary.p90_fanout {
                InsightSeverity::Warning
            } else {
                InsightSeverity::Info
            };
            insights.push(Insight {
                category: InsightCategory::HighFanout,
                severity,
                module: name.clone(),
                message: format!(
                    "Module '{}' has a fan-out of {} (p90={}). \
                     Consider whether it has too many responsibilities \
                     and might benefit from being split.",
                    name, fo, summary.p90_fanout
                ),
                metrics: InsightMetrics {
                    fanout: Some(*fo),
                    fanin: None,
                    entropy: None,
                    scc_id: None,
                    scc_size: None,
                    depth: None,
                },
            });
        }
    }

    // Circular Dependency: one insight per non-trivial SCC
    for scc in sccs {
        let severity = if scc.size >= 4 {
            InsightSeverity::Warning
        } else {
            InsightSeverity::Info
        };
        let members_str = scc.members.join(", ");
        insights.push(Insight {
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
        });
    }

    // Deep Chain: max_depth >= 8 OR (max_depth > 2*avg_depth && max_depth >= 5)
    if summary.max_depth >= 8
        || (summary.max_depth as f64 > 2.0 * summary.avg_depth && summary.max_depth >= 5)
    {
        insights.push(Insight {
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
        });
    }

    // High Entropy: entropy > 2.5 and fanout >= 5, skip god modules
    for (name, fo, _fi, entropy) in &node_metrics {
        if god_modules.contains(name) {
            continue;
        }
        if *entropy > 2.5 && *fo >= 5 {
            insights.push(Insight {
                category: InsightCategory::HighEntropy,
                severity: InsightSeverity::Info,
                module: name.clone(),
                message: format!(
                    "Module '{}' has high dependency entropy ({:.2}), \
                     meaning its {} dependencies are spread broadly. \
                     Consider consolidating behind a facade.",
                    name, entropy, fo
                ),
                metrics: InsightMetrics {
                    fanout: Some(*fo),
                    fanin: None,
                    entropy: Some((*entropy * 100.0).round() / 100.0),
                    scc_id: None,
                    scc_size: None,
                    depth: None,
                },
            });
        }
    }

    // Sort: Warnings first, then by category priority, then alphabetically by module
    insights.sort_by(|a, b| {
        let sev_a = matches!(a.severity, InsightSeverity::Warning);
        let sev_b = matches!(b.severity, InsightSeverity::Warning);
        sev_b
            .cmp(&sev_a)
            .then(a.category.cmp(&b.category))
            .then(a.module.cmp(&b.module))
    });

    insights
}

/// Generate insights using resolved configuration rules and per-path overrides.
pub fn generate_insights_with_config(
    graph: &DepGraph,
    summary: &Summary,
    sccs: &[SccInfo],
    rules: &ResolvedRules,
    overrides: &[(globset::GlobMatcher, OverrideEntry)],
) -> Vec<Insight> {
    let mut insights = Vec::new();
    let mut god_modules: HashSet<String> = HashSet::new();

    // Collect per-node metrics (including file path for override matching)
    let node_metrics: Vec<_> = graph
        .node_indices()
        .map(|idx| {
            let name = graph[idx].name.clone();
            let file_path = graph[idx].path.to_string_lossy().to_string();
            let fo = fan_out(graph, idx);
            let fi = fan_in(graph, idx);
            let edge_weights: Vec<usize> = graph
                .edges_directed(idx, Direction::Outgoing)
                .map(|e| e.weight().weight)
                .collect();
            let entropy = shannon_entropy(&edge_weights);
            (name, file_path, fo, fi, entropy)
        })
        .collect();

    // God Module
    if rules.god_module.enabled {
        for (name, file_path, fo, fi, _entropy) in &node_metrics {
            let (effective_rules, enabled) =
                crate::config::overrides::apply_overrides_with_file_path(
                    name,
                    Some(file_path),
                    rules,
                    overrides,
                );
            if !enabled || !effective_rules.god_module.enabled {
                continue;
            }
            let r = &effective_rules.god_module;
            let fo_check = if r.relative_to_p90 {
                *fo > summary.p90_fanout && *fo >= r.min_fanout
            } else {
                *fo >= r.min_fanout
            };
            let fi_check = if r.relative_to_p90 {
                *fi > summary.p90_fanin && *fi >= r.min_fanin
            } else {
                *fi >= r.min_fanin
            };
            if fo_check && fi_check {
                god_modules.insert(name.clone());
                insights.push(Insight {
                    category: InsightCategory::GodModule,
                    severity: InsightSeverity::Warning,
                    module: name.clone(),
                    message: format!(
                        "Module '{}' has both high fan-out ({}) and high fan-in ({}), \
                         suggesting it may be acting as a central hub. \
                         Consider decomposing it to reduce coupling.",
                        name, fo, fi
                    ),
                    metrics: InsightMetrics {
                        fanout: Some(*fo),
                        fanin: Some(*fi),
                        entropy: None,
                        scc_id: None,
                        scc_size: None,
                        depth: None,
                    },
                });
            }
        }
    }

    // High Fan-out
    if rules.high_fanout.enabled {
        for (name, file_path, fo, _fi, _entropy) in &node_metrics {
            if god_modules.contains(name) {
                continue;
            }
            let (effective_rules, enabled) =
                crate::config::overrides::apply_overrides_with_file_path(
                    name,
                    Some(file_path),
                    rules,
                    overrides,
                );
            if !enabled || !effective_rules.high_fanout.enabled {
                continue;
            }
            let r = &effective_rules.high_fanout;
            let triggers = if r.relative_to_p90 {
                *fo > summary.p90_fanout && *fo >= r.min_fanout
            } else {
                *fo >= r.min_fanout
            };
            if triggers {
                let severity = if *fo >= r.warning_multiplier * summary.p90_fanout {
                    InsightSeverity::Warning
                } else {
                    InsightSeverity::Info
                };
                insights.push(Insight {
                    category: InsightCategory::HighFanout,
                    severity,
                    module: name.clone(),
                    message: format!(
                        "Module '{}' has a fan-out of {} (p90={}). \
                         Consider whether it has too many responsibilities \
                         and might benefit from being split.",
                        name, fo, summary.p90_fanout
                    ),
                    metrics: InsightMetrics {
                        fanout: Some(*fo),
                        fanin: None,
                        entropy: None,
                        scc_id: None,
                        scc_size: None,
                        depth: None,
                    },
                });
            }
        }
    }

    // Circular Dependency
    if rules.circular_dependency.enabled {
        for scc in sccs {
            let severity = if scc.size >= rules.circular_dependency.warning_min_size {
                InsightSeverity::Warning
            } else {
                InsightSeverity::Info
            };
            let members_str = scc.members.join(", ");
            insights.push(Insight {
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
            });
        }
    }

    // Deep Chain
    if rules.deep_chain.enabled {
        let dc = &rules.deep_chain;
        if summary.max_depth >= dc.absolute_depth
            || (summary.max_depth as f64 > dc.relative_multiplier * summary.avg_depth
                && summary.max_depth >= dc.relative_min_depth)
        {
            insights.push(Insight {
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
            });
        }
    }

    // High Entropy
    if rules.high_entropy.enabled {
        for (name, file_path, fo, _fi, entropy) in &node_metrics {
            if god_modules.contains(name) {
                continue;
            }
            let (effective_rules, enabled) =
                crate::config::overrides::apply_overrides_with_file_path(
                    name,
                    Some(file_path),
                    rules,
                    overrides,
                );
            if !enabled || !effective_rules.high_entropy.enabled {
                continue;
            }
            let r = &effective_rules.high_entropy;
            if *entropy > r.min_entropy && *fo >= r.min_fanout {
                insights.push(Insight {
                    category: InsightCategory::HighEntropy,
                    severity: InsightSeverity::Info,
                    module: name.clone(),
                    message: format!(
                        "Module '{}' has high dependency entropy ({:.2}), \
                         meaning its {} dependencies are spread broadly. \
                         Consider consolidating behind a facade.",
                        name, entropy, fo
                    ),
                    metrics: InsightMetrics {
                        fanout: Some(*fo),
                        fanin: None,
                        entropy: Some((*entropy * 100.0).round() / 100.0),
                        scc_id: None,
                        scc_size: None,
                        depth: None,
                    },
                });
            }
        }
    }

    // Sort: Warnings first, then by category priority, then alphabetically by module
    insights.sort_by(|a, b| {
        let sev_a = matches!(a.severity, InsightSeverity::Warning);
        let sev_b = matches!(b.severity, InsightSeverity::Warning);
        sev_b
            .cmp(&sev_a)
            .then(a.category.cmp(&b.category))
            .then(a.module.cmp(&b.module))
    });

    insights
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::ir::{GraphEdge, GraphNode, NodeKind};
    use crate::metrics::scc::find_non_trivial_sccs;
    use crate::metrics::summary::Summary;
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
}
