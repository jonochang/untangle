use crate::quality::coverage::{coverage_for_range, CoverageMap};
use crate::quality::metrics::QualityMetric;
use crate::quality::{FunctionInfo, QualityMetricKind, QualityResult};

pub struct CrapMetric;

impl CrapMetric {
    pub fn crap_score(cc: f64, coverage: f64) -> f64 {
        cc * cc * (1.0 - coverage).powi(3) + cc
    }

    pub fn risk_band(score: f64) -> &'static str {
        if score < 5.0 {
            "low"
        } else if score < 30.0 {
            "moderate"
        } else {
            "high"
        }
    }
}

impl QualityMetric for CrapMetric {
    fn kind(&self) -> QualityMetricKind {
        QualityMetricKind::Crap
    }

    fn requires_coverage(&self) -> bool {
        true
    }

    fn compute(
        &self,
        functions: &[FunctionInfo],
        coverage: Option<&CoverageMap>,
    ) -> Vec<QualityResult> {
        let mut results = Vec::new();
        for f in functions {
            let cov = coverage
                .and_then(|map| map.get(&f.file))
                .map(|fc| coverage_for_range(fc, f.start_line, f.end_line))
                .unwrap_or(0.0);
            let cc = f.cyclomatic_complexity as f64;
            let score = Self::crap_score(cc, cov);
            results.push(QualityResult {
                metric: self.kind(),
                file: f.file.clone(),
                function: f.name.clone(),
                start_line: f.start_line,
                end_line: f.end_line,
                cyclomatic_complexity: f.cyclomatic_complexity,
                coverage_pct: (cov * 1000.0).round() / 10.0,
                score: (score * 10.0).round() / 10.0,
                risk_band: Some(Self::risk_band(score).to_string()),
            });
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crap_score_full_coverage() {
        let score = CrapMetric::crap_score(10.0, 1.0);
        assert!((score - 10.0).abs() < 1e-10);
    }

    #[test]
    fn crap_score_zero_coverage() {
        let score = CrapMetric::crap_score(10.0, 0.0);
        assert!((score - 110.0).abs() < 1e-10);
    }
}
