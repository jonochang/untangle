use crate::errors::Result;
use crate::graph::ir::DepGraph;
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
) -> Result<()> {
    writeln!(writer, "Untangle Analysis Report")?;
    writeln!(writer, "========================")?;
    writeln!(writer)?;
    writeln!(writer, "Language:   {}", metadata.language)?;
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

    writeln!(
        writer,
        "Completed in {:.2}s ({:.0} modules/sec)",
        metadata.elapsed_ms as f64 / 1000.0,
        metadata.modules_per_second
    )?;

    Ok(())
}
