use crate::quality::coverage::CoverageMap;
use crate::quality::metrics::QualityMetric;
use crate::quality::{FunctionInfo, QualityMetricKind, QualityResult};

pub struct ComplexityMetric;

impl ComplexityMetric {
    fn risk_band(cc: usize) -> &'static str {
        match cc {
            0..=4 => "low",
            5..=9 => "moderate",
            _ => "high",
        }
    }
}

impl QualityMetric for ComplexityMetric {
    fn kind(&self) -> QualityMetricKind {
        QualityMetricKind::Complexity
    }

    fn requires_coverage(&self) -> bool {
        false
    }

    fn compute(
        &self,
        functions: &[FunctionInfo],
        _coverage: Option<&CoverageMap>,
    ) -> Vec<QualityResult> {
        functions
            .iter()
            .map(|function| QualityResult {
                metric: self.kind(),
                file: function.file.clone(),
                function: function.name.clone(),
                start_line: function.start_line,
                end_line: function.end_line,
                cyclomatic_complexity: function.cyclomatic_complexity,
                coverage_pct: None,
                score: function.cyclomatic_complexity as f64,
                risk_band: Some(Self::risk_band(function.cyclomatic_complexity).to_string()),
            })
            .collect()
    }
}
