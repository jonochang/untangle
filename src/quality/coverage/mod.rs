use std::collections::HashMap;
use std::path::PathBuf;

pub mod lcov;

/// file-path → (line → hit-count)
pub type CoverageMap = HashMap<PathBuf, HashMap<usize, u64>>;

/// Fraction of instrumented lines in [start, end] that were hit at least once.
/// Returns 0.0 when no instrumented lines exist.
pub fn coverage_for_range(
    file_coverage: &HashMap<usize, u64>,
    start_line: usize,
    end_line: usize,
) -> f64 {
    let mut instrumented = 0usize;
    let mut covered = 0usize;
    for line in start_line..=end_line {
        if let Some(&hits) = file_coverage.get(&line) {
            instrumented += 1;
            if hits > 0 {
                covered += 1;
            }
        }
    }
    if instrumented == 0 {
        0.0
    } else {
        covered as f64 / instrumented as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_for_range_empty() {
        let map: HashMap<usize, u64> = HashMap::new();
        assert_eq!(coverage_for_range(&map, 1, 10), 0.0);
    }

    #[test]
    fn coverage_for_range_partial() {
        let mut map = HashMap::new();
        map.insert(1, 1);
        map.insert(2, 0);
        map.insert(3, 2);
        let cov = coverage_for_range(&map, 1, 3);
        assert!((cov - (2.0 / 3.0)).abs() < 1e-9);
    }
}
