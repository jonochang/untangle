use crate::errors::Result;
use crate::graph::diff::{
    ComparisonVerdict, DiffResult, EdgeChange, FanoutChange, SccChange, SummaryDelta,
};
use crate::graph::ir::DepGraph;
use crate::insights::{Insight, InsightSeverity};
use crate::metrics::scc::SccInfo;
use crate::metrics::summary::Summary;
use crate::output::json::{LanguageStats, Metadata};
use std::io::Write;

struct HotspotRow {
    module: String,
    fanout: usize,
    fanin: usize,
    scc_label: String,
}

/// Write analyze output as human-readable text.
pub fn write_analyze_text<W: Write>(
    writer: &mut W,
    graph: &DepGraph,
    summary: &Summary,
    sccs: &[SccInfo],
    metadata: &Metadata,
    top_n: Option<usize>,
    insights: Option<&[Insight]>,
) -> Result<()> {
    write_analyze_header(writer)?;
    write_language_metadata(writer, metadata)?;
    write_analyze_summary(writer, summary)?;
    write_hotspots(writer, collect_hotspots(graph), top_n)?;
    write_sccs(writer, sccs)?;
    write_insights(writer, insights)?;
    write_footer(writer, metadata.elapsed_ms, metadata.modules_per_second)?;
    Ok(())
}

fn write_analyze_header<W: Write>(writer: &mut W) -> Result<()> {
    writeln!(writer, "Untangle Analysis Report")?;
    writeln!(writer, "========================")?;
    writeln!(writer)?;
    Ok(())
}

fn write_language_metadata<W: Write>(writer: &mut W, metadata: &Metadata) -> Result<()> {
    if let Some(ref lang_stats) = metadata.languages {
        writeln!(writer, "Languages:")?;
        for ls in lang_stats {
            write_language_stat(writer, ls)?;
        }
    } else {
        writeln!(writer, "Language:   {}", metadata.language)?;
    }
    writeln!(writer, "Root:       {}", metadata.root.display())?;
    writeln!(writer, "Nodes:      {}", metadata.node_count)?;
    writeln!(writer, "Edges:      {}", metadata.edge_count)?;
    writeln!(writer, "Density:    {:.4}", metadata.edge_density)?;
    writeln!(writer, "Parsed:     {} files", metadata.files_parsed)?;
    writeln!(writer, "Skipped:    {} files", metadata.files_skipped)?;
    writeln!(
        writer,
        "Unresolved: {} imports",
        metadata.unresolved_imports
    )?;
    writeln!(writer)?;
    Ok(())
}

fn write_language_stat<W: Write>(writer: &mut W, stat: &LanguageStats) -> Result<()> {
    writeln!(
        writer,
        "  - {} ({} files, {} nodes, {}/{} imports resolved)",
        stat.language,
        stat.files_parsed,
        stat.nodes,
        stat.imports_resolved,
        stat.imports_resolved + stat.imports_unresolved
    )?;
    Ok(())
}

fn write_analyze_summary<W: Write>(writer: &mut W, summary: &Summary) -> Result<()> {
    writeln!(writer, "Summary")?;
    writeln!(writer, "-------")?;
    writeln!(
        writer,
        "Fan-out:  mean={:.2}  p90={}  max={}",
        summary.mean_fanout, summary.p90_fanout, summary.max_fanout
    )?;
    writeln!(
        writer,
        "Fan-in:   mean={:.2}  p90={}  max={}",
        summary.mean_fanin, summary.p90_fanin, summary.max_fanin
    )?;
    writeln!(
        writer,
        "SCCs:     {} (largest: {}, total nodes: {})",
        summary.scc_count, summary.largest_scc_size, summary.total_nodes_in_sccs
    )?;
    writeln!(
        writer,
        "Depth:    max={}  avg={:.2}",
        summary.max_depth, summary.avg_depth
    )?;
    writeln!(
        writer,
        "Complexity: {} (nodes + edges + max_depth)",
        summary.total_complexity
    )?;
    writeln!(writer)?;
    Ok(())
}

fn collect_hotspots(graph: &DepGraph) -> Vec<HotspotRow> {
    let scc_map = crate::metrics::scc::node_scc_map(graph);
    let mut rows: Vec<_> = graph
        .node_indices()
        .map(|idx| {
            let fanout = crate::metrics::fanout::fan_out(graph, idx);
            let fanin = crate::metrics::fanout::fan_in(graph, idx);
            let scc_label = scc_map
                .get(&idx)
                .map(|id| format!("#{id}"))
                .unwrap_or_else(|| "-".to_string());
            HotspotRow {
                module: graph[idx].name.clone(),
                fanout,
                fanin,
                scc_label,
            }
        })
        .collect();
    rows.sort_by(|a, b| b.fanout.cmp(&a.fanout));
    rows
}

fn write_hotspots<W: Write>(
    writer: &mut W,
    hotspots: Vec<HotspotRow>,
    top_n: Option<usize>,
) -> Result<()> {
    let limit = top_n.unwrap_or(20).min(hotspots.len());
    if limit > 0 {
        writeln!(writer, "Top {} Hotspots", limit)?;
        writeln!(writer, "{:-<60}", "")?;
        writeln!(
            writer,
            "{:<40} {:>8} {:>8} {:>5}",
            "Module", "Fan-out", "Fan-in", "SCC"
        )?;
        for hotspot in hotspots.iter().take(limit) {
            writeln!(
                writer,
                "{:<40} {:>8} {:>8} {:>5}",
                hotspot.module, hotspot.fanout, hotspot.fanin, hotspot.scc_label
            )?;
        }
        writeln!(writer)?;
    }
    Ok(())
}

fn write_sccs<W: Write>(writer: &mut W, sccs: &[SccInfo]) -> Result<()> {
    if !sccs.is_empty() {
        writeln!(writer, "Strongly Connected Components")?;
        writeln!(writer, "{:-<60}", "")?;
        for scc in sccs {
            writeln!(
                writer,
                "SCC #{} (size={}, internal_edges={})",
                scc.id, scc.size, scc.internal_edges
            )?;
            for member in &scc.members {
                writeln!(writer, "  - {member}")?;
            }
            writeln!(writer)?;
        }
    }
    Ok(())
}

fn write_insights<W: Write>(writer: &mut W, insights: Option<&[Insight]>) -> Result<()> {
    if let Some(insights) = insights {
        if !insights.is_empty() {
            writeln!(writer, "Insights")?;
            writeln!(writer, "{:-<60}", "")?;
            for insight in insights {
                writeln!(writer, "  {} {}", insight_marker(insight), insight.message)?;
            }
            writeln!(writer)?;
        }
    }
    Ok(())
}

fn insight_marker(insight: &Insight) -> &'static str {
    match insight.severity {
        InsightSeverity::Warning => "[!]",
        InsightSeverity::Info => "[i]",
    }
}

fn write_footer<W: Write>(writer: &mut W, elapsed_ms: u64, modules_per_second: f64) -> Result<()> {
    writeln!(
        writer,
        "Completed in {:.2}s ({:.0} modules/sec)",
        elapsed_ms as f64 / 1000.0,
        modules_per_second
    )?;
    Ok(())
}

/// Write diff output as human-readable text.
pub fn write_diff_text<W: Write>(writer: &mut W, result: &DiffResult) -> Result<()> {
    write_diff_header(writer, result)?;
    write_diff_summary(writer, &result.summary_delta)?;
    write_edge_changes(writer, "New Edges", &result.new_edges)?;
    write_edge_changes(writer, "Removed Edges", &result.removed_edges)?;
    write_fanout_changes(writer, &result.fanout_changes)?;
    write_scc_changes(writer, "New SCCs", &result.scc_changes.new_sccs)?;
    write_scc_changes(writer, "Enlarged SCCs", &result.scc_changes.enlarged_sccs)?;
    write_footer(writer, result.elapsed_ms, result.modules_per_second)?;
    Ok(())
}

fn write_diff_header<W: Write>(writer: &mut W, result: &DiffResult) -> Result<()> {
    writeln!(writer, "Untangle Diff Report")?;
    writeln!(writer, "====================")?;
    writeln!(writer)?;
    writeln!(writer, "Base: {}", result.base_ref)?;
    writeln!(writer, "Head: {}", result.head_ref)?;
    writeln!(writer, "Verdict: {:?}", result.verdict)?;
    writeln!(
        writer,
        "Comparison: {}",
        comparison_verdict_label(&result.comparison.verdict)
    )?;
    writeln!(writer, "Summary: {}", result.comparison.summary)?;
    writeln!(writer, "Recommendation: {}", result.comparison.recommendation)?;
    if !result.comparison.drivers.is_empty() {
        writeln!(writer, "Drivers: {}", result.comparison.drivers.join(", "))?;
    }
    if !result.reasons.is_empty() {
        writeln!(writer, "Violations: {}", result.reasons.join(", "))?;
    }
    writeln!(writer)?;
    Ok(())
}

fn comparison_verdict_label(verdict: &ComparisonVerdict) -> &'static str {
    match verdict {
        ComparisonVerdict::Improved => "improved",
        ComparisonVerdict::Worse => "worse",
        ComparisonVerdict::Mixed => "mixed",
        ComparisonVerdict::Unchanged => "unchanged",
    }
}

fn write_diff_summary<W: Write>(writer: &mut W, delta: &SummaryDelta) -> Result<()> {
    writeln!(writer, "Summary")?;
    writeln!(writer, "-------")?;
    writeln!(
        writer,
        "Nodes:       +{} / -{}",
        delta.nodes_added, delta.nodes_removed
    )?;
    writeln!(
        writer,
        "Edges:       +{} / -{} (net {:+})",
        delta.edges_added, delta.edges_removed, delta.net_edge_change
    )?;
    writeln!(writer, "Mean fanout: {:+.2}", delta.mean_fanout_delta)?;
    writeln!(writer, "Mean entropy: {:+.2}", delta.mean_entropy_delta)?;
    writeln!(
        writer,
        "SCCs:        {:+} (largest: {:+})",
        delta.scc_count_delta, delta.largest_scc_size_delta
    )?;
    writeln!(writer)?;
    Ok(())
}

fn write_edge_changes<W: Write>(writer: &mut W, label: &str, edges: &[EdgeChange]) -> Result<()> {
    if !edges.is_empty() {
        writeln!(writer, "{} ({})", label, edges.len())?;
        writeln!(writer, "{:-<60}", "")?;
        for edge in edges {
            writeln!(writer, "  {} -> {}", edge.from, edge.to)?;
        }
        writeln!(writer)?;
    }
    Ok(())
}

fn write_fanout_changes<W: Write>(writer: &mut W, changes: &[FanoutChange]) -> Result<()> {
    if !changes.is_empty() {
        writeln!(writer, "Fan-out Changes ({})", changes.len())?;
        writeln!(writer, "{:-<60}", "")?;
        for change in changes {
            writeln!(
                writer,
                "  {} : {} -> {} ({:+})",
                change.node, change.fanout_before, change.fanout_after, change.delta
            )?;
        }
        writeln!(writer)?;
    }
    Ok(())
}

fn write_scc_changes<W: Write>(writer: &mut W, label: &str, changes: &[SccChange]) -> Result<()> {
    if !changes.is_empty() {
        writeln!(writer, "{} ({})", label, changes.len())?;
        writeln!(writer, "{:-<60}", "")?;
        for scc in changes {
            writeln!(writer, "  size={}: {}", scc.size, scc.members.join(", "))?;
        }
        writeln!(writer)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::diff::{
        Comparison, ComparisonVerdict, EdgeChange, FanoutChange, SccChange, SummaryDelta, Verdict,
    };
    use crate::graph::ir::{EdgeKind, GraphEdge, GraphNode, NodeKind};
    use crate::parse::common::SourceLocation;
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

    fn make_metadata() -> Metadata {
        Metadata {
            language: "rust".to_string(),
            granularity: "module".to_string(),
            root: PathBuf::from("/tmp/project"),
            node_count: 3,
            edge_count: 3,
            edge_density: 0.3333,
            files_parsed: 3,
            files_skipped: 0,
            unresolved_imports: 1,
            timestamp: "2026-03-10T00:00:00Z".to_string(),
            elapsed_ms: 1250,
            modules_per_second: 24.0,
            languages: None,
        }
    }

    fn make_graph_with_cycle() -> DepGraph {
        let mut graph = DepGraph::new();
        let a = graph.add_node(make_node("alpha"));
        let b = graph.add_node(make_node("beta"));
        let c = graph.add_node(make_node("gamma"));
        graph.add_edge(a, b, make_edge());
        graph.add_edge(a, c, make_edge());
        graph.add_edge(b, a, make_edge());
        graph
    }

    fn make_insight(message: &str, severity: InsightSeverity) -> Insight {
        Insight {
            category: crate::insights::InsightCategory::HighFanout,
            severity,
            module: "alpha".to_string(),
            message: message.to_string(),
            metrics: crate::insights::InsightMetrics {
                fanout: Some(2),
                fanin: None,
                entropy: None,
                scc_id: None,
                scc_size: None,
                depth: None,
            },
        }
    }

    fn make_diff_result() -> DiffResult {
        DiffResult {
            base_ref: "main".to_string(),
            head_ref: "HEAD".to_string(),
            verdict: Verdict::Fail,
            comparison: Comparison {
                verdict: ComparisonVerdict::Worse,
                summary: "Change appears worse: net edge count increased by 1.".to_string(),
                recommendation:
                    "Review the added coupling before treating this change as complete."
                        .to_string(),
                drivers: vec!["net edge count increased by 1".to_string()],
            },
            reasons: vec!["new-edge".to_string()],
            elapsed_ms: 850,
            modules_per_second: 12.0,
            summary_delta: SummaryDelta {
                nodes_added: 1,
                nodes_removed: 0,
                edges_added: 2,
                edges_removed: 1,
                net_edge_change: 1,
                scc_count_delta: 1,
                largest_scc_size_delta: 2,
                mean_fanout_delta: 0.5,
                mean_entropy_delta: 0.25,
                max_depth_delta: 0,
                total_complexity_delta: 0,
            },
            new_edges: vec![EdgeChange {
                from: "a".to_string(),
                to: "b".to_string(),
                source_locations: vec![SourceLocation {
                    file: PathBuf::from("src/lib.rs"),
                    line: 1,
                    column: Some(1),
                }],
            }],
            removed_edges: vec![EdgeChange {
                from: "b".to_string(),
                to: "c".to_string(),
                source_locations: vec![],
            }],
            fanout_changes: vec![FanoutChange {
                node: "a".to_string(),
                fanout_before: 1,
                fanout_after: 2,
                delta: 1,
                entropy_before: 0.0,
                entropy_after: 1.0,
                new_targets: vec![],
            }],
            scc_changes: crate::graph::diff::SccChanges {
                new_sccs: vec![SccChange {
                    members: vec!["a".to_string(), "b".to_string()],
                    size: 2,
                }],
                enlarged_sccs: vec![SccChange {
                    members: vec!["x".to_string(), "y".to_string(), "z".to_string()],
                    size: 3,
                }],
                resolved_sccs: vec![],
            },
            architecture_policy_delta: None,
        }
    }

    #[test]
    fn analyze_text_omits_optional_sections_when_empty() {
        let graph = DepGraph::new();
        let summary = Summary::from_graph(&graph);
        let mut buf = Vec::new();
        write_analyze_text(
            &mut buf,
            &graph,
            &summary,
            &[],
            &make_metadata(),
            Some(0),
            None,
        )
        .unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Untangle Analysis Report"));
        assert!(output.contains("Language:   rust"));
        assert!(!output.contains("Top 0 Hotspots"));
        assert!(!output.contains("Strongly Connected Components"));
        assert!(!output.contains("Insights"));
        assert!(output.contains("Completed in 1.25s (24 modules/sec)"));
    }

    #[test]
    fn analyze_text_renders_languages_hotspots_sccs_and_insights() {
        let graph = make_graph_with_cycle();
        let summary = Summary::from_graph(&graph);
        let sccs = crate::metrics::scc::find_non_trivial_sccs(&graph);
        let mut metadata = make_metadata();
        metadata.languages = Some(vec![
            LanguageStats {
                language: "rust".to_string(),
                files_parsed: 2,
                nodes: 2,
                imports_resolved: 3,
                imports_unresolved: 1,
            },
            LanguageStats {
                language: "python".to_string(),
                files_parsed: 1,
                nodes: 1,
                imports_resolved: 1,
                imports_unresolved: 0,
            },
        ]);
        let insights = vec![
            make_insight("first warning", InsightSeverity::Warning),
            make_insight("second info", InsightSeverity::Info),
        ];
        let mut buf = Vec::new();
        write_analyze_text(
            &mut buf,
            &graph,
            &summary,
            &sccs,
            &metadata,
            Some(2),
            Some(&insights),
        )
        .unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Languages:"));
        assert!(output.contains("rust (2 files, 2 nodes, 3/4 imports resolved)"));
        assert!(output.contains("Top 2 Hotspots"));
        assert!(output.contains("alpha"));
        assert!(output.contains("#0"));
        assert!(output.contains("Strongly Connected Components"));
        assert!(output.contains("SCC #0"));
        assert!(output.contains("Insights"));
        assert!(output.contains("[!] first warning"));
        assert!(output.contains("[i] second info"));
    }

    #[test]
    fn diff_text_omits_change_sections_when_empty() {
        let mut result = make_diff_result();
        result.verdict = Verdict::Pass;
        result.reasons.clear();
        result.new_edges.clear();
        result.removed_edges.clear();
        result.fanout_changes.clear();
        result.scc_changes.new_sccs.clear();
        result.scc_changes.enlarged_sccs.clear();
        let mut buf = Vec::new();
        write_diff_text(&mut buf, &result).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Verdict: Pass"));
        assert!(output.contains("Summary"));
        assert!(!output.contains("Violations:"));
        assert!(!output.contains("New Edges"));
        assert!(!output.contains("Fan-out Changes"));
    }

    #[test]
    fn diff_text_renders_all_sections() {
        let result = make_diff_result();
        let mut buf = Vec::new();
        write_diff_text(&mut buf, &result).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Verdict: Fail"));
        assert!(output.contains("Violations: new-edge"));
        assert!(output.contains("New Edges (1)"));
        assert!(output.contains("Removed Edges (1)"));
        assert!(output.contains("Fan-out Changes (1)"));
        assert!(output.contains("New SCCs (1)"));
        assert!(output.contains("Enlarged SCCs (1)"));
        assert!(output.contains("Completed in 0.85s (12 modules/sec)"));
    }
}
