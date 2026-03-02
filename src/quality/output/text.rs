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
