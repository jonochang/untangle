use crate::config::{keys, ResolvedConfig};
use std::io::Write;

/// Render `config show` output.
pub fn render_show<W: Write>(w: &mut W, config: &ResolvedConfig) -> std::io::Result<()> {
    // Loaded files
    if config.loaded_files.is_empty() {
        writeln!(w, "Loaded config files: (none)")?;
    } else {
        writeln!(w, "Loaded config files:")?;
        for (i, path) in config.loaded_files.iter().enumerate() {
            writeln!(w, "  {}. {}", i + 1, path.display())?;
        }
    }
    writeln!(w)?;

    // Resolved settings
    writeln!(w, "Resolved settings:")?;
    for (key, source) in config.provenance.sorted_entries() {
        let value = get_value_for_key(config, key);
        writeln!(w, "  {}: {} <- {}", key, value, source)?;
    }

    Ok(())
}

/// Render `config explain <category>` output.
pub fn render_explain<W: Write>(
    w: &mut W,
    config: &ResolvedConfig,
    category: &str,
) -> std::io::Result<()> {
    let entries: Vec<_> = if category == "architecture_policy" {
        config
            .provenance
            .entries_with_prefix("analyze.architecture.")
    } else {
        let prefix = format!("rules.{}.", category);
        config.provenance.entries_with_prefix(&prefix)
    };

    if entries.is_empty() {
        writeln!(w, "Unknown rule category: {}", category)?;
        writeln!(
            w,
            "Available categories: high_fanout, god_module, circular_dependency, deep_chain, high_entropy, architecture_policy"
        )?;
        return Ok(());
    }

    writeln!(w, "Rule: {}", category)?;
    writeln!(w)?;

    for (key, source) in &entries {
        let value = get_value_for_key(config, key);
        writeln!(w, "  {}: {} <- {}", key, value, source)?;
    }

    Ok(())
}

fn get_value_for_key(config: &ResolvedConfig, key: &str) -> String {
    defaults_value(config, key)
        .or_else(|| analyze_value(config, key))
        .or_else(|| quality_value(config, key))
        .or_else(|| rules_value(config, key))
        .or_else(|| language_value(config, key))
        .unwrap_or_else(|| "(unknown)".to_string())
}

fn defaults_value(config: &ResolvedConfig, key: &str) -> Option<String> {
    match key {
        keys::DEFAULTS_LANG => Some(
            config
                .lang
                .map_or("(auto-detect)".to_string(), |l| l.to_string()),
        ),
        keys::DEFAULTS_QUIET => Some(config.quiet.to_string()),
        keys::DEFAULTS_INCLUDE_TESTS => Some(config.include_tests.to_string()),
        _ => None,
    }
}

fn analyze_value(config: &ResolvedConfig, key: &str) -> Option<String> {
    match key {
        keys::ANALYZE_REPORT_FORMAT => Some(config.analyze_report.format.to_string()),
        keys::ANALYZE_REPORT_TOP => Some(
            config
                .analyze_report
                .top
                .map_or("(all)".to_string(), |n| n.to_string()),
        ),
        keys::ANALYZE_REPORT_INSIGHTS => Some(match config.analyze_report.insights {
            crate::config::InsightsConfig::Auto => "auto".to_string(),
            crate::config::InsightsConfig::On => "on".to_string(),
            crate::config::InsightsConfig::Off => "off".to_string(),
        }),
        keys::ANALYZE_REPORT_THRESHOLD_FANOUT => Some(
            config
                .analyze_report
                .threshold_fanout
                .map_or("(unset)".to_string(), |n| n.to_string()),
        ),
        keys::ANALYZE_REPORT_THRESHOLD_SCC => Some(
            config
                .analyze_report
                .threshold_scc
                .map_or("(unset)".to_string(), |n| n.to_string()),
        ),
        keys::ANALYZE_GRAPH_FORMAT => Some(config.analyze_graph.format.to_string()),
        keys::ANALYZE_ARCHITECTURE_FORMAT => Some(config.analyze_architecture.format.to_string()),
        keys::ANALYZE_ARCHITECTURE_LEVEL => Some(config.analyze_architecture.level.to_string()),
        keys::ANALYZE_ARCHITECTURE_CHECK_FORMAT => {
            Some(config.analyze_architecture.check_format.to_string())
        }
        keys::ANALYZE_ARCHITECTURE_FAIL_ON_VIOLATIONS => Some(
            config
                .analyze_architecture
                .fail_on_violations
                .to_string(),
        ),
        keys::ANALYZE_ARCHITECTURE_FAIL_ON_CYCLES => {
            Some(config.analyze_architecture.fail_on_cycles.to_string())
        }
        keys::ANALYZE_ARCHITECTURE_IGNORED_COMPONENTS => Some(format!(
            "{:?}",
            config.analyze_architecture.ignored_components
        )),
        keys::ANALYZE_ARCHITECTURE_ALLOWED_DEPENDENCIES => Some(format!(
            "{:?}",
            config.analyze_architecture.allowed_dependencies
        )),
        keys::ANALYZE_ARCHITECTURE_FORBIDDEN_DEPENDENCIES => Some(format!(
            "{:?}",
            config.analyze_architecture.forbidden_dependencies
        )),
        keys::ANALYZE_ARCHITECTURE_EXCEPTIONS => {
            Some(format!("{:?}", config.analyze_architecture.exceptions))
        }
        keys::DIFF_FORMAT => Some(config.diff.format.to_string()),
        keys::SERVICE_GRAPH_FORMAT => Some(config.service_graph.format.to_string()),
        _ => None,
    }
}

fn quality_value(config: &ResolvedConfig, key: &str) -> Option<String> {
    match key {
        keys::QUALITY_FUNCTIONS_FORMAT => Some(config.quality_functions.format.to_string()),
        keys::QUALITY_FUNCTIONS_TOP => Some(
            config
                .quality_functions
                .top
                .map_or("(all)".to_string(), |n| n.to_string()),
        ),
        keys::QUALITY_PROJECT_FORMAT => Some(config.quality_project.format.to_string()),
        keys::QUALITY_PROJECT_TOP => Some(
            config
                .quality_project
                .top
                .map_or("(all)".to_string(), |n| n.to_string()),
        ),
        keys::QUALITY_SPECS_FORMAT => Some(config.quality_specs.format.to_string()),
        keys::QUALITY_SPECS_TOP => Some(
            config
                .quality_specs
                .top
                .map_or("(all)".to_string(), |n| n.to_string()),
        ),
        keys::QUALITY_SPECS_STABLE_MAX_SCORE => {
            Some(config.quality_specs.stable_max_score.to_string())
        }
        keys::QUALITY_SPECS_SPLIT_MIN_SCORE => {
            Some(config.quality_specs.split_min_score.to_string())
        }
        _ => None,
    }
}

fn rules_value(config: &ResolvedConfig, key: &str) -> Option<String> {
    match key {
        keys::RULES_HIGH_FANOUT_ENABLED => Some(config.rules.high_fanout.enabled.to_string()),
        keys::RULES_HIGH_FANOUT_MIN_FANOUT => Some(config.rules.high_fanout.min_fanout.to_string()),
        keys::RULES_HIGH_FANOUT_RELATIVE_TO_P90 => {
            Some(config.rules.high_fanout.relative_to_p90.to_string())
        }
        keys::RULES_HIGH_FANOUT_WARNING_MULTIPLIER => {
            Some(config.rules.high_fanout.warning_multiplier.to_string())
        }
        keys::RULES_GOD_MODULE_ENABLED => Some(config.rules.god_module.enabled.to_string()),
        keys::RULES_GOD_MODULE_MIN_FANOUT => Some(config.rules.god_module.min_fanout.to_string()),
        keys::RULES_GOD_MODULE_MIN_FANIN => Some(config.rules.god_module.min_fanin.to_string()),
        keys::RULES_GOD_MODULE_RELATIVE_TO_P90 => {
            Some(config.rules.god_module.relative_to_p90.to_string())
        }
        keys::RULES_CIRCULAR_DEPENDENCY_ENABLED => {
            Some(config.rules.circular_dependency.enabled.to_string())
        }
        keys::RULES_CIRCULAR_DEPENDENCY_WARNING_MIN_SIZE => Some(
            config
                .rules
                .circular_dependency
                .warning_min_size
                .to_string(),
        ),
        keys::RULES_DEEP_CHAIN_ENABLED => Some(config.rules.deep_chain.enabled.to_string()),
        keys::RULES_DEEP_CHAIN_ABSOLUTE_DEPTH => {
            Some(config.rules.deep_chain.absolute_depth.to_string())
        }
        keys::RULES_DEEP_CHAIN_RELATIVE_MULTIPLIER => {
            Some(config.rules.deep_chain.relative_multiplier.to_string())
        }
        keys::RULES_DEEP_CHAIN_RELATIVE_MIN_DEPTH => {
            Some(config.rules.deep_chain.relative_min_depth.to_string())
        }
        keys::RULES_HIGH_ENTROPY_ENABLED => Some(config.rules.high_entropy.enabled.to_string()),
        keys::RULES_HIGH_ENTROPY_MIN_ENTROPY => {
            Some(config.rules.high_entropy.min_entropy.to_string())
        }
        keys::RULES_HIGH_ENTROPY_MIN_FANOUT => {
            Some(config.rules.high_entropy.min_fanout.to_string())
        }
        _ => None,
    }
}

fn language_value(config: &ResolvedConfig, key: &str) -> Option<String> {
    match key {
        keys::GO_EXCLUDE_STDLIB => Some(config.go.exclude_stdlib.to_string()),
        keys::PYTHON_RESOLVE_RELATIVE => Some(config.python.resolve_relative.to_string()),
        keys::RUBY_ZEITWERK => Some(config.ruby.zeitwerk.to_string()),
        keys::RUBY_LOAD_PATH => Some(format!("{:?}", config.ruby.load_path)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::provenance::{ProvenanceMap, Source};
    use crate::config::{
        ResolvedConfig, ResolvedGoConfig, ResolvedPythonConfig, ResolvedRubyConfig, ResolvedRules,
    };
    use std::path::PathBuf;

    fn make_test_config() -> ResolvedConfig {
        let mut prov = ProvenanceMap::new();
        prov.set("analyze.report.format", Source::Default);
        prov.set("defaults.quiet", Source::Default);
        prov.set(
            "rules.high_fanout.enabled",
            Source::ProjectConfig(PathBuf::from("/project/.untangle.toml")),
        );
        prov.set("rules.high_fanout.min_fanout", Source::Default);
        prov.set("rules.high_fanout.relative_to_p90", Source::Default);
        prov.set("rules.high_fanout.warning_multiplier", Source::Default);

        ResolvedConfig {
            lang: None,
            quiet: false,
            include_tests: false,
            include: Vec::new(),
            exclude: Vec::new(),
            ignore_patterns: Vec::new(),
            analyze_report: Default::default(),
            analyze_graph: Default::default(),
            analyze_architecture: Default::default(),
            diff: Default::default(),
            quality_functions: Default::default(),
            quality_project: Default::default(),
            quality_specs: Default::default(),
            service_graph: Default::default(),
            rules: ResolvedRules::default(),
            fail_on: Vec::new(),
            go: ResolvedGoConfig::default(),
            python: ResolvedPythonConfig::default(),
            ruby: ResolvedRubyConfig::default(),
            overrides: Vec::new(),
            services: Vec::new(),
            provenance: prov,
            loaded_files: vec![PathBuf::from("/project/.untangle.toml")],
        }
    }

    #[test]
    fn render_show_format() {
        let config = make_test_config();
        let mut buf = Vec::new();
        render_show(&mut buf, &config).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Loaded config files:"));
        assert!(output.contains("/project/.untangle.toml"));
        assert!(output.contains("Resolved settings:"));
        assert!(output.contains("analyze.report.format: json <- default"));
    }

    #[test]
    fn render_show_no_files() {
        let mut config = make_test_config();
        config.loaded_files.clear();
        let mut buf = Vec::new();
        render_show(&mut buf, &config).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Loaded config files: (none)"));
    }

    #[test]
    fn render_explain_known_category() {
        let config = make_test_config();
        let mut buf = Vec::new();
        render_explain(&mut buf, &config, "high_fanout").unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Rule: high_fanout"));
        assert!(output.contains("rules.high_fanout.enabled"));
        assert!(output.contains("rules.high_fanout.min_fanout"));
    }

    #[test]
    fn render_explain_unknown_category() {
        let config = make_test_config();
        let mut buf = Vec::new();
        render_explain(&mut buf, &config, "nonexistent").unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Unknown rule category: nonexistent"));
    }

    #[test]
    fn every_known_key_has_a_renderable_value() {
        let config = make_test_config();
        for key in keys::ALL {
            assert_ne!(
                get_value_for_key(&config, key),
                "(unknown)",
                "missing key: {key}"
            );
        }
    }
}
