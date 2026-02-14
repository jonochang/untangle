use crate::config::{OverrideEntry, ResolvedRules};

/// Apply per-path overrides to a module.
///
/// Returns `(rules, enabled)`:
/// - If `enabled=false`, the caller should skip the module entirely.
/// - First matching glob wins.
/// - Override block replaces the entire rule object (un-specified fields revert to built-in defaults).
pub fn apply_overrides(
    module_path: &str,
    base_rules: &ResolvedRules,
    overrides: &[(globset::GlobMatcher, OverrideEntry)],
) -> (ResolvedRules, bool) {
    for (matcher, entry) in overrides {
        if matcher.is_match(module_path) {
            if !entry.enabled {
                return (base_rules.clone(), false);
            }
            if let Some(ref override_rules) = entry.rules {
                return (override_rules.clone(), true);
            }
            return (base_rules.clone(), true);
        }
    }
    (base_rules.clone(), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HighFanoutRule, OverrideEntry, ResolvedRules};
    use globset::Glob;

    fn make_overrides() -> Vec<(globset::GlobMatcher, OverrideEntry)> {
        vec![
            (
                Glob::new("**/vendor/**").unwrap().compile_matcher(),
                OverrideEntry {
                    enabled: false,
                    rules: None,
                },
            ),
            (
                Glob::new("src/legacy/**").unwrap().compile_matcher(),
                OverrideEntry {
                    enabled: true,
                    rules: Some(ResolvedRules {
                        high_fanout: HighFanoutRule {
                            enabled: true,
                            min_fanout: 40,
                            relative_to_p90: false,
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                },
            ),
        ]
    }

    #[test]
    fn no_match_passthrough() {
        let base = ResolvedRules::default();
        let overrides = make_overrides();
        let (rules, enabled) = apply_overrides("src/main.rs", &base, &overrides);
        assert!(enabled);
        assert_eq!(rules.high_fanout.min_fanout, base.high_fanout.min_fanout);
    }

    #[test]
    fn vendor_disabled() {
        let base = ResolvedRules::default();
        let overrides = make_overrides();
        let (_, enabled) = apply_overrides("foo/vendor/lib.rs", &base, &overrides);
        assert!(!enabled);
    }

    #[test]
    fn legacy_override_replaces_rules() {
        let base = ResolvedRules::default();
        let overrides = make_overrides();
        let (rules, enabled) = apply_overrides("src/legacy/old.rs", &base, &overrides);
        assert!(enabled);
        assert_eq!(rules.high_fanout.min_fanout, 40);
        assert!(!rules.high_fanout.relative_to_p90);
        // Other rules remain at defaults (override replaces)
        assert_eq!(
            rules.god_module.min_fanout,
            ResolvedRules::default().god_module.min_fanout
        );
    }

    #[test]
    fn first_match_wins() {
        let overrides = vec![
            (
                Glob::new("src/**").unwrap().compile_matcher(),
                OverrideEntry {
                    enabled: false,
                    rules: None,
                },
            ),
            (
                Glob::new("src/legacy/**").unwrap().compile_matcher(),
                OverrideEntry {
                    enabled: true,
                    rules: None,
                },
            ),
        ];
        let base = ResolvedRules::default();
        let (_, enabled) = apply_overrides("src/legacy/old.rs", &base, &overrides);
        // First match (src/**) wins
        assert!(!enabled);
    }
}
