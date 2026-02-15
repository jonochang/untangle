use crate::config::ResolvedConfig;
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
    let prefix = format!("rules.{}.", category);
    let entries: Vec<_> = config.provenance.entries_with_prefix(&prefix);

    if entries.is_empty() {
        writeln!(w, "Unknown rule category: {}", category)?;
        writeln!(
            w,
            "Available categories: high_fanout, god_module, circular_dependency, deep_chain, high_entropy"
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
    match key {
        "defaults.lang" => config
            .lang
            .map_or("(auto-detect)".to_string(), |l| l.to_string()),
        "defaults.format" => config.format.clone(),
        "defaults.quiet" => config.quiet.to_string(),
        "defaults.top" => config.top.map_or("(all)".to_string(), |n| n.to_string()),
        "defaults.include_tests" => config.include_tests.to_string(),
        "defaults.no_insights" => config.no_insights.to_string(),
        "rules.high_fanout.enabled" => config.rules.high_fanout.enabled.to_string(),
        "rules.high_fanout.min_fanout" => config.rules.high_fanout.min_fanout.to_string(),
        "rules.high_fanout.relative_to_p90" => config.rules.high_fanout.relative_to_p90.to_string(),
        "rules.high_fanout.warning_multiplier" => {
            config.rules.high_fanout.warning_multiplier.to_string()
        }
        "rules.god_module.enabled" => config.rules.god_module.enabled.to_string(),
        "rules.god_module.min_fanout" => config.rules.god_module.min_fanout.to_string(),
        "rules.god_module.min_fanin" => config.rules.god_module.min_fanin.to_string(),
        "rules.god_module.relative_to_p90" => config.rules.god_module.relative_to_p90.to_string(),
        "rules.circular_dependency.enabled" => config.rules.circular_dependency.enabled.to_string(),
        "rules.circular_dependency.warning_min_size" => config
            .rules
            .circular_dependency
            .warning_min_size
            .to_string(),
        "rules.deep_chain.enabled" => config.rules.deep_chain.enabled.to_string(),
        "rules.deep_chain.absolute_depth" => config.rules.deep_chain.absolute_depth.to_string(),
        "rules.deep_chain.relative_multiplier" => {
            config.rules.deep_chain.relative_multiplier.to_string()
        }
        "rules.deep_chain.relative_min_depth" => {
            config.rules.deep_chain.relative_min_depth.to_string()
        }
        "rules.high_entropy.enabled" => config.rules.high_entropy.enabled.to_string(),
        "rules.high_entropy.min_entropy" => config.rules.high_entropy.min_entropy.to_string(),
        "rules.high_entropy.min_fanout" => config.rules.high_entropy.min_fanout.to_string(),
        "go.exclude_stdlib" => config.go.exclude_stdlib.to_string(),
        "python.resolve_relative" => config.python.resolve_relative.to_string(),
        "ruby.zeitwerk" => config.ruby.zeitwerk.to_string(),
        "ruby.load_path" => format!("{:?}", config.ruby.load_path),
        _ => "(unknown)".to_string(),
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
        prov.set("defaults.format", Source::Default);
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
            format: "json".to_string(),
            quiet: false,
            top: None,
            include_tests: false,
            no_insights: false,
            include: Vec::new(),
            exclude: Vec::new(),
            ignore_patterns: Vec::new(),
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
        assert!(output.contains("defaults.format: json <- default"));
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
}
