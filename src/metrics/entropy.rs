/// Compute Shannon entropy of a weight distribution.
///
/// H = -Σ p_i log₂(p_i) where p_i = w_i / Σw
pub fn shannon_entropy(edge_weights: &[usize]) -> f64 {
    let total: f64 = edge_weights.iter().sum::<usize>() as f64;
    if total == 0.0 {
        return 0.0;
    }
    edge_weights
        .iter()
        .filter(|&&w| w > 0)
        .map(|&w| {
            let p = w as f64 / total;
            -p * p.log2()
        })
        .sum()
}

/// Compute SCC-adjusted entropy.
///
/// For nodes in a non-trivial SCC (size > 1), the entropy is amplified
/// by the SCC size: H_adj = H × (1 + ln(|SCC|))
pub fn scc_adjusted_entropy(base_entropy: f64, scc_size: usize) -> f64 {
    if scc_size <= 1 {
        base_entropy
    } else {
        base_entropy * (1.0 + (scc_size as f64).ln())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entropy_empty() {
        assert_eq!(shannon_entropy(&[]), 0.0);
    }

    #[test]
    fn entropy_single_weight() {
        // All weight on one edge → 0 entropy
        let h = shannon_entropy(&[10]);
        assert!((h - 0.0).abs() < 1e-10);
    }

    #[test]
    fn entropy_uniform_distribution() {
        // 4 equal weights → log2(4) = 2.0
        let h = shannon_entropy(&[1, 1, 1, 1]);
        assert!((h - 2.0).abs() < 1e-10);
    }

    #[test]
    fn entropy_two_equal() {
        // 2 equal weights → log2(2) = 1.0
        let h = shannon_entropy(&[5, 5]);
        assert!((h - 1.0).abs() < 1e-10);
    }

    #[test]
    fn entropy_skewed() {
        // Skewed distribution → lower than uniform
        let h = shannon_entropy(&[9, 1]);
        assert!(h > 0.0);
        assert!(h < 1.0); // Less than log2(2) = 1.0
    }

    #[test]
    fn scc_adjusted_trivial() {
        assert_eq!(scc_adjusted_entropy(2.0, 1), 2.0);
    }

    #[test]
    fn scc_adjusted_nontrivial() {
        let result = scc_adjusted_entropy(2.0, 5);
        let expected = 2.0 * (1.0 + 5.0_f64.ln());
        assert!((result - expected).abs() < 1e-10);
    }

    #[test]
    fn scc_adjusted_zero_entropy() {
        assert_eq!(scc_adjusted_entropy(0.0, 10), 0.0);
    }
}
