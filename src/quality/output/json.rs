use crate::errors::Result;
use crate::quality::QualityReport;
use std::io::Write;

pub fn write_quality_json<W: Write>(writer: &mut W, report: &QualityReport) -> Result<()> {
    serde_json::to_writer_pretty(writer, report)?;
    Ok(())
}
