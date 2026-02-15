use crate::errors::Result;
use crate::graph::diff::DiffResult;
use crate::graph::ir::DepGraph;
use crate::insights::{Insight, InsightSeverity};
use crate::metrics::scc::SccInfo;
use crate::metrics::summary::Summary;
use crate::output::json::Metadata;
use std::io::Write;

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
    writeln!(writer, "Untangle Analysis Report")?;
    writeln!(writer, "========================")?;
    writeln!(writer)?;
    if let Some(ref lang_stats) = metadata.languages {
        writeln!(writer, "Languages:")?;
        for ls in lang_stats {
            writeln!(
                writer,
                "  - {} ({} files, {} nodes)",
                ls.language, ls.files_parsed, ls.nodes
            )?;
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

    // Hotspots
    let scc_map = crate::metrics::scc::node_scc_map(graph);
    let mut nodes: Vec<_> = graph
        .node_indices()
        .map(|idx| {
            let fanout = crate::metrics::fanout::fan_out(graph, idx);
            let fanin = crate::metrics::fanout::fan_in(graph, idx);
            (idx, fanout, fanin)
        })
        .collect();
    nodes.sort_by(|a, b| b.1.cmp(&a.1));

    let limit = top_n.unwrap_or(20).min(nodes.len());
    if limit > 0 {
        writeln!(writer, "Top {} Hotspots", limit)?;
        writeln!(writer, "{:-<60}", "")?;
        writeln!(
            writer,
            "{:<40} {:>8} {:>8} {:>5}",
            "Module", "Fan-out", "Fan-in", "SCC"
        )?;
        for &(idx, fanout, fanin) in nodes.iter().take(limit) {
            let scc_label = scc_map
                .get(&idx)
                .map(|id| format!("#{id}"))
                .unwrap_or_else(|| "-".to_string());
            writeln!(
                writer,
                "{:<40} {:>8} {:>8} {:>5}",
                graph[idx].name, fanout, fanin, scc_label
            )?;
        }
        writeln!(writer)?;
    }

    // SCCs
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

    if let Some(insights) = insights {
        if !insights.is_empty() {
            writeln!(writer, "Insights")?;
            writeln!(writer, "{:-<60}", "")?;
            for insight in insights {
                let marker = match insight.severity {
                    InsightSeverity::Warning => "[!]",
                    InsightSeverity::Info => "[i]",
                };
                writeln!(writer, "  {} {}", marker, insight.message)?;
            }
            writeln!(writer)?;
        }
    }

    writeln!(
        writer,
        "Completed in {:.2}s ({:.0} modules/sec)",
        metadata.elapsed_ms as f64 / 1000.0,
        metadata.modules_per_second
    )?;

    Ok(())
}

/// Write diff output as human-readable text.
pub fn write_diff_text<W: Write>(writer: &mut W, result: &DiffResult) -> Result<()> {
    writeln!(writer, "Untangle Diff Report")?;
    writeln!(writer, "====================")?;
    writeln!(writer)?;
    writeln!(writer, "Base: {}", result.base_ref)?;
    writeln!(writer, "Head: {}", result.head_ref)?;
    writeln!(writer, "Verdict: {:?}", result.verdict)?;
    if !result.reasons.is_empty() {
        writeln!(writer, "Violations: {}", result.reasons.join(", "))?;
    }
    writeln!(writer)?;

    // Summary delta
    let d = &result.summary_delta;
    writeln!(writer, "Summary")?;
    writeln!(writer, "-------")?;
    writeln!(
        writer,
        "Nodes:       +{} / -{}",
        d.nodes_added, d.nodes_removed
    )?;
    writeln!(
        writer,
        "Edges:       +{} / -{} (net {:+})",
        d.edges_added, d.edges_removed, d.net_edge_change
    )?;
    writeln!(writer, "Mean fanout: {:+.2}", d.mean_fanout_delta)?;
    writeln!(writer, "Mean entropy: {:+.2}", d.mean_entropy_delta)?;
    writeln!(
        writer,
        "SCCs:        {:+} (largest: {:+})",
        d.scc_count_delta, d.largest_scc_size_delta
    )?;
    writeln!(writer)?;

    // New edges
    if !result.new_edges.is_empty() {
        writeln!(writer, "New Edges ({})", result.new_edges.len())?;
        writeln!(writer, "{:-<60}", "")?;
        for edge in &result.new_edges {
            writeln!(writer, "  {} -> {}", edge.from, edge.to)?;
        }
        writeln!(writer)?;
    }

    // Removed edges
    if !result.removed_edges.is_empty() {
        writeln!(writer, "Removed Edges ({})", result.removed_edges.len())?;
        writeln!(writer, "{:-<60}", "")?;
        for edge in &result.removed_edges {
            writeln!(writer, "  {} -> {}", edge.from, edge.to)?;
        }
        writeln!(writer)?;
    }

    // Fan-out changes
    if !result.fanout_changes.is_empty() {
        writeln!(writer, "Fan-out Changes ({})", result.fanout_changes.len())?;
        writeln!(writer, "{:-<60}", "")?;
        for change in &result.fanout_changes {
            writeln!(
                writer,
                "  {} : {} -> {} ({:+})",
                change.node, change.fanout_before, change.fanout_after, change.delta
            )?;
        }
        writeln!(writer)?;
    }

    // SCC changes
    if !result.scc_changes.new_sccs.is_empty() {
        writeln!(writer, "New SCCs ({})", result.scc_changes.new_sccs.len())?;
        writeln!(writer, "{:-<60}", "")?;
        for scc in &result.scc_changes.new_sccs {
            writeln!(writer, "  size={}: {}", scc.size, scc.members.join(", "))?;
        }
        writeln!(writer)?;
    }
    if !result.scc_changes.enlarged_sccs.is_empty() {
        writeln!(
            writer,
            "Enlarged SCCs ({})",
            result.scc_changes.enlarged_sccs.len()
        )?;
        writeln!(writer, "{:-<60}", "")?;
        for scc in &result.scc_changes.enlarged_sccs {
            writeln!(writer, "  size={}: {}", scc.size, scc.members.join(", "))?;
        }
        writeln!(writer)?;
    }

    writeln!(
        writer,
        "Completed in {:.2}s ({:.0} modules/sec)",
        result.elapsed_ms as f64 / 1000.0,
        result.modules_per_second
    )?;

    Ok(())
}
