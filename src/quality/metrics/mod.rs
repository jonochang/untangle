use crate::quality::coverage::CoverageMap;
use crate::quality::{FunctionInfo, QualityMetricKind, QualityResult};

pub mod crap;

pub trait QualityMetric {
    fn kind(&self) -> QualityMetricKind;
    fn requires_coverage(&self) -> bool;
    fn compute(&self, functions: &[FunctionInfo], coverage: Option<&CoverageMap>) -> Vec<QualityResult>;
}

pub fn metric_for(kind: QualityMetricKind) -> Box<dyn QualityMetric> {
    match kind {
        QualityMetricKind::Crap => Box::new(crap::CrapMetric),
    }
}
