use crate::errors::Result;
use crate::quality::QualityReport;
use std::io::Write;

pub fn write_quality_text<W: Write>(writer: &mut W, report: &QualityReport) -> Result<()> {
    writeln!(writer, "Untangle Quality Report")?;
    writeln!(writer, "======================")?;
    writeln!(writer)?;
    writeln!(writer, "Metric:    {}", report.metadata.metric)?;
    if let Some(ref cov) = report.metadata.coverage_file {
        writeln!(writer, "Coverage:  {}", cov.display())?;
    }
    writeln!(writer, "Root:      {}", report.metadata.root.display())?;
    writeln!(writer, "Files:     {}", report.metadata.files_parsed)?;
    writeln!(writer, "Functions: {}", report.metadata.functions)?;
    writeln!(writer)?;

    if let Some(ref overall) = report.overall {
        writeln!(writer, "Untangle Metric")?;
        writeln!(writer, "---------------")?;
        writeln!(writer, "Nodes:      {}", overall.untangle.nodes)?;
        writeln!(writer, "Edges:      {}", overall.untangle.edges)?;
        writeln!(writer, "Density:    {:.4}", overall.untangle.edge_density)?;
        writeln!(
            writer,
            "Parsed:     {} files",
            overall.untangle.files_parsed
        )?;
        writeln!(
            writer,
            "Fan-out:  mean={:.2}  p90={}  max={}",
            overall.untangle.summary.mean_fanout,
            overall.untangle.summary.p90_fanout,
            overall.untangle.summary.max_fanout
        )?;
        writeln!(
            writer,
            "Fan-in:   mean={:.2}  p90={}  max={}",
            overall.untangle.summary.mean_fanin,
            overall.untangle.summary.p90_fanin,
            overall.untangle.summary.max_fanin
        )?;
        writeln!(
            writer,
            "SCCs:     {} (largest: {}, total nodes: {})",
            overall.untangle.summary.scc_count,
            overall.untangle.summary.largest_scc_size,
            overall.untangle.summary.total_nodes_in_sccs
        )?;
        writeln!(
            writer,
            "Depth:    max={}  avg={:.2}",
            overall.untangle.summary.max_depth, overall.untangle.summary.avg_depth
        )?;
        writeln!(
            writer,
            "Complexity: {} (nodes + edges + max_depth)",
            overall.untangle.summary.total_complexity
        )?;
        writeln!(writer)?;

        if !overall.untangle.hotspots.is_empty() {
            writeln!(writer, "Untangle Hotspots")?;
            writeln!(writer, "{:-<60}", "")?;
            writeln!(
                writer,
                "{:<40} {:>8} {:>8} {:>5}",
                "Module", "Fan-out", "Fan-in", "SCC"
            )?;
            for hotspot in &overall.untangle.hotspots {
                let scc_label = hotspot
                    .scc
                    .map(|id| format!("#{id}"))
                    .unwrap_or_else(|| "-".to_string());
                writeln!(
                    writer,
                    "{:<40} {:>8} {:>8} {:>5}",
                    hotspot.module, hotspot.fanout, hotspot.fanin, scc_label
                )?;
            }
            writeln!(writer)?;
        }

        writeln!(writer, "CRAP Summary")?;
        writeln!(writer, "------------")?;
        writeln!(
            writer,
            "Scores: mean={:.2}  p90={:.2}  max={:.2}",
            overall.crap.mean_score, overall.crap.p90_score, overall.crap.max_score
        )?;
        writeln!(
            writer,
            "Risk:   high={}  moderate={}  low={}",
            overall.crap.high_risk, overall.crap.moderate_risk, overall.crap.low_risk
        )?;
        writeln!(
            writer,
            "Reported functions: {}",
            overall.crap.functions_reported
        )?;
        writeln!(writer)?;
    }

    let header = format!(
        "{:<30} {:<40} {:>4} {:>6} {:>8} {:>8}",
        "Function", "File", "CC", "Cov%", "Score", "Risk"
    );
    writeln!(writer, "{header}")?;
    writeln!(writer, "{:-<102}", "")?;

    for r in &report.results {
        let risk = r.risk_band.as_deref().unwrap_or("-");
        writeln!(
            writer,
            "{:<30} {:<40} {:>4} {:>5.1}% {:>8.1} {:>8}",
            r.function,
            r.file.display(),
            r.cyclomatic_complexity,
            r.coverage_pct,
            r.score,
            risk
        )?;
    }

    Ok(())
}
