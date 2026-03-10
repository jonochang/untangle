pub mod complexity;
pub mod coverage;
pub mod engine;
pub mod functions;
pub mod metrics;
pub mod output;
pub mod untangle;

use crate::metrics::summary::Summary;
use crate::walk::Language;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityMetricKind {
    Crap,
    Complexity,
    Overall,
}

impl std::fmt::Display for QualityMetricKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QualityMetricKind::Crap => write!(f, "crap"),
            QualityMetricKind::Complexity => write!(f, "complexity"),
            QualityMetricKind::Overall => write!(f, "overall"),
        }
    }
}

impl std::str::FromStr for QualityMetricKind {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "crap" => Ok(QualityMetricKind::Crap),
            "complexity" | "cyclomatic" | "cc" => Ok(QualityMetricKind::Complexity),
            "overall" | "all" => Ok(QualityMetricKind::Overall),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overall: Option<QualityOverallSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QualityOverallSummary {
    pub untangle: UntangleMetricSummary,
    pub crap: CrapMetricSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct UntangleMetricSummary {
    pub nodes: usize,
    pub edges: usize,
    pub edge_density: f64,
    pub files_parsed: usize,
    pub summary: Summary,
    pub hotspots: Vec<UntangleHotspot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrapMetricSummary {
    pub functions_reported: usize,
    pub mean_score: f64,
    pub p90_score: f64,
    pub max_score: f64,
    pub high_risk: usize,
    pub moderate_risk: usize,
    pub low_risk: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct UntangleHotspot {
    pub module: String,
    pub path: PathBuf,
    pub fanout: usize,
    pub fanin: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scc: Option<usize>,
}
