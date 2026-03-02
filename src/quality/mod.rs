pub mod coverage;
pub mod complexity;
pub mod engine;
pub mod metrics;
pub mod output;

use crate::walk::Language;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityMetricKind {
    Crap,
}

impl std::fmt::Display for QualityMetricKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QualityMetricKind::Crap => write!(f, "crap"),
        }
    }
}

impl std::str::FromStr for QualityMetricKind {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "crap" => Ok(QualityMetricKind::Crap),
            _ => Err(format!("unknown metric: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionInfo {
    pub name: String,
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub cyclomatic_complexity: usize,
    pub language: Language,
}

#[derive(Debug, Clone, Serialize)]
pub struct QualityResult {
    pub metric: QualityMetricKind,
    pub file: PathBuf,
    pub function: String,
    pub start_line: usize,
    pub end_line: usize,
    pub cyclomatic_complexity: usize,
    pub coverage_pct: f64,
    pub score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_band: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QualityMetadata {
    pub root: PathBuf,
    pub metric: QualityMetricKind,
    pub coverage_file: Option<PathBuf>,
    pub languages: Vec<String>,
    pub files_parsed: usize,
    pub functions: usize,
    pub timestamp: String,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct QualityReport {
    pub metadata: QualityMetadata,
    pub results: Vec<QualityResult>,
}
