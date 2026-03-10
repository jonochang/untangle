use crate::errors::Result;
use crate::quality::QualityReport;
use std::io::Write;

pub fn write_quality_json<W: Write>(writer: &mut W, report: &QualityReport) -> Result<()> {
    let kind = if report.metadata.metric == crate::quality::QualityMetricKind::Overall {
        "quality.project"
    } else {
        "quality.functions"
    };
    serde_json::to_writer_pretty(
        writer,
        &serde_json::json!({
            "kind": kind,
            "schema_version": 2,
            "report": report,
        }),
    )?;
    Ok(())
}
