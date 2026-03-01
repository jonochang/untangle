use crate::config::provenance::{ProvenanceMap, Source};
use crate::config::schema::FileConfig;
use crate::config::{
    CircularDependencyRule, DeepChainRule, GodModuleRule, HighEntropyRule, HighFanoutRule,
    OverrideEntry, ResolvedConfig, ResolvedGoConfig, ResolvedPythonConfig, ResolvedRubyConfig,
    ResolvedRules, ResolvedService,
};
use crate::errors::{Result, UntangleError};
use crate::output::OutputFormat;
use crate::walk::Language;
use globset::Glob;
use std::path::{Path, PathBuf};

/// CLI overrides extracted from command arguments.
#[derive(Debug, Default)]
pub struct CliOverrides {
    pub lang: Option<Language>,
    pub format: Option<OutputFormat>,
    pub quiet: bool,
    pub top: Option<usize>,
    pub include_tests: bool,
    pub no_insights: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub fail_on: Vec<String>,
    pub threshold_fanout: Option<usize>,
    pub threshold_scc: Option<usize>,
}

/// Resolve configuration by applying layers bottom-up:
/// 1. Built-in defaults
/// 2. User config (~/.config/untangle/config.toml)
/// 3. Project config (nearest .untangle.toml walking up from working_dir)
/// 4. Environment variables
/// 5. CLI overrides
pub fn resolve_config(working_dir: &Path, cli: &CliOverrides) -> Result<ResolvedConfig> {
    let mut prov = ProvenanceMap::new();
    let mut loaded_files = Vec::new();

    // 1. Start with built-in defaults
    let mut config = ResolvedConfig {
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
        provenance: ProvenanceMap::new(),
        loaded_files: Vec::new(),
    };

    // Set default provenance
    set_all_default_provenance(&mut prov);

    // 2. User config
    if let Some(user_config_path) = find_user_config() {
        if user_config_path.exists() {
            let content = std::fs::read_to_string(&user_config_path).map_err(|_| {
                UntangleError::Config(format!(
                    "Could not read user config: {}",
                    user_config_path.display()
                ))
            })?;
            let mut file_config = FileConfig::from_toml(&content)
                .map_err(|e| UntangleError::Config(format!("Invalid user config: {e}")))?;
            file_config.migrate_legacy();
            apply_file_config(
                &mut config,
                &file_config,
                Source::UserConfig(user_config_path.clone()),
                &mut prov,
            );
            loaded_files.push(user_config_path);
        }
    }

    // 3. Project config (walk up from working_dir)
    if let Some(project_config_path) = find_project_config(working_dir) {
        let content = std::fs::read_to_string(&project_config_path).map_err(|_| {
            UntangleError::Config(format!(
                "Could not read project config: {}",
                project_config_path.display()
            ))
        })?;
        let mut file_config = FileConfig::from_toml(&content)
            .map_err(|e| UntangleError::Config(format!("Invalid project config: {e}")))?;
        file_config.migrate_legacy();
        apply_file_config(
            &mut config,
            &file_config,
            Source::ProjectConfig(project_config_path.clone()),
            &mut prov,
        );
        loaded_files.push(project_config_path);
    }

    // 4. Environment variables
    apply_env_vars(&mut config, &mut prov);

    // 5. CLI overrides
    apply_cli_overrides(&mut config, cli, &mut prov);

    // Load .untangleignore
    let ignore_patterns = crate::config::ignore::load_untangleignore(working_dir);
    config.ignore_patterns = ignore_patterns;

    config.provenance = prov;
    config.loaded_files = loaded_files;

    Ok(config)
}

fn find_user_config() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("untangle").join("config.toml"))
}

fn find_project_config(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let config_path = dir.join(".untangle.toml");
        if config_path.exists() {
            return Some(config_path);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn set_all_default_provenance(prov: &mut ProvenanceMap) {
    let defaults = [
        "defaults.lang",
        "defaults.format",
        "defaults.quiet",
        "defaults.top",
        "defaults.include_tests",
        "defaults.no_insights",
        "rules.high_fanout.enabled",
        "rules.high_fanout.min_fanout",
        "rules.high_fanout.relative_to_p90",
        "rules.high_fanout.warning_multiplier",
        "rules.god_module.enabled",
        "rules.god_module.min_fanout",
        "rules.god_module.min_fanin",
        "rules.god_module.relative_to_p90",
        "rules.circular_dependency.enabled",
        "rules.circular_dependency.warning_min_size",
        "rules.deep_chain.enabled",
        "rules.deep_chain.absolute_depth",
        "rules.deep_chain.relative_multiplier",
        "rules.deep_chain.relative_min_depth",
        "rules.high_entropy.enabled",
        "rules.high_entropy.min_entropy",
        "rules.high_entropy.min_fanout",
        "go.exclude_stdlib",
        "python.resolve_relative",
        "ruby.zeitwerk",
        "ruby.load_path",
    ];
    for key in defaults {
        prov.set(key, Source::Default);
    }
}

fn apply_file_config(
    config: &mut ResolvedConfig,
    file: &FileConfig,
    source: Source,
    prov: &mut ProvenanceMap,
) {
    // Defaults
    if let Some(ref lang) = file.defaults.lang {
        if let Ok(l) = lang.parse::<Language>() {
            config.lang = Some(l);
            prov.set("defaults.lang", source.clone());
        }
    }
    if let Some(ref format) = file.defaults.format {
        config.format = format.clone();
        prov.set("defaults.format", source.clone());
    }
    if let Some(quiet) = file.defaults.quiet {
        config.quiet = quiet;
        prov.set("defaults.quiet", source.clone());
    }
    if let Some(top) = file.defaults.top {
        config.top = Some(top);
        prov.set("defaults.top", source.clone());
    }
    if let Some(include_tests) = file.defaults.include_tests {
        config.include_tests = include_tests;
        prov.set("defaults.include_tests", source.clone());
    }
    if let Some(no_insights) = file.defaults.no_insights {
        config.no_insights = no_insights;
        prov.set("defaults.no_insights", source.clone());
    }

    // Targeting
    if !file.targeting.include.is_empty() {
        config.include = file.targeting.include.clone();
    }
    if !file.targeting.exclude.is_empty() {
        config.exclude = file.targeting.exclude.clone();
    }

    // Rules
    if let Some(ref hf) = file.rules.high_fanout {
        apply_high_fanout_config(&mut config.rules.high_fanout, hf, &source, prov);
    }
    if let Some(ref gm) = file.rules.god_module {
        apply_god_module_config(&mut config.rules.god_module, gm, &source, prov);
    }
    if let Some(ref cd) = file.rules.circular_dependency {
        apply_circular_dep_config(&mut config.rules.circular_dependency, cd, &source, prov);
    }
    if let Some(ref dc) = file.rules.deep_chain {
        apply_deep_chain_config(&mut config.rules.deep_chain, dc, &source, prov);
    }
    if let Some(ref he) = file.rules.high_entropy {
        apply_high_entropy_config(&mut config.rules.high_entropy, he, &source, prov);
    }

    // Fail on
    if !file.fail_on.conditions.is_empty() {
        config.fail_on = file.fail_on.conditions.clone();
    }

    // Language configs
    if let Some(exclude_stdlib) = file.go.exclude_stdlib {
        config.go.exclude_stdlib = exclude_stdlib;
        prov.set("go.exclude_stdlib", source.clone());
    }
    if let Some(resolve_relative) = file.python.resolve_relative {
        config.python.resolve_relative = resolve_relative;
        prov.set("python.resolve_relative", source.clone());
    }
    if let Some(zeitwerk) = file.ruby.zeitwerk {
        config.ruby.zeitwerk = zeitwerk;
        prov.set("ruby.zeitwerk", source.clone());
    }
    if !file.ruby.load_path.is_empty() {
        config.ruby.load_path = file.ruby.load_path.clone();
        prov.set("ruby.load_path", source.clone());
    }

    // Overrides
    for (pattern, ov) in &file.overrides {
        if let Ok(glob) = Glob::new(pattern) {
            let matcher = glob.compile_matcher();
            let rules = ov.rules.as_ref().map(|r| {
                let mut resolved = ResolvedRules::default();
                if let Some(ref hf) = r.high_fanout {
                    apply_high_fanout_override(&mut resolved.high_fanout, hf);
                }
                if let Some(ref gm) = r.god_module {
                    apply_god_module_override(&mut resolved.god_module, gm);
                }
                if let Some(ref cd) = r.circular_dependency {
                    apply_circular_dep_override(&mut resolved.circular_dependency, cd);
                }
                if let Some(ref dc) = r.deep_chain {
                    apply_deep_chain_override(&mut resolved.deep_chain, dc);
                }
                if let Some(ref he) = r.high_entropy {
                    apply_high_entropy_override(&mut resolved.high_entropy, he);
                }
                resolved
            });
            config.overrides.push((
                matcher,
                OverrideEntry {
                    enabled: ov.enabled.unwrap_or(true),
                    rules,
                },
            ));
        }
    }

    // Services
    for (name, svc) in &file.services {
        let lang = svc.lang.as_ref().and_then(|l| l.parse::<Language>().ok());
        config.services.push(ResolvedService {
            name: name.clone(),
            root: PathBuf::from(&svc.root),
            lang,
            graphql_schemas: svc.graphql_schemas.iter().map(PathBuf::from).collect(),
            openapi_specs: svc.openapi_specs.iter().map(PathBuf::from).collect(),
            base_urls: svc.base_urls.clone(),
        });
    }
}

fn apply_high_fanout_config(
    rule: &mut HighFanoutRule,
    file: &crate::config::schema::HighFanoutRuleConfig,
    source: &Source,
    prov: &mut ProvenanceMap,
) {
    if let Some(enabled) = file.enabled {
        rule.enabled = enabled;
        prov.set("rules.high_fanout.enabled", source.clone());
    }
    if let Some(min_fanout) = file.min_fanout {
        rule.min_fanout = min_fanout;
        prov.set("rules.high_fanout.min_fanout", source.clone());
    }
    if let Some(relative_to_p90) = file.relative_to_p90 {
        rule.relative_to_p90 = relative_to_p90;
        prov.set("rules.high_fanout.relative_to_p90", source.clone());
    }
    if let Some(warning_multiplier) = file.warning_multiplier {
        rule.warning_multiplier = warning_multiplier;
        prov.set("rules.high_fanout.warning_multiplier", source.clone());
    }
}

fn apply_god_module_config(
    rule: &mut GodModuleRule,
    file: &crate::config::schema::GodModuleRuleConfig,
    source: &Source,
    prov: &mut ProvenanceMap,
) {
    if let Some(enabled) = file.enabled {
        rule.enabled = enabled;
        prov.set("rules.god_module.enabled", source.clone());
    }
    if let Some(min_fanout) = file.min_fanout {
        rule.min_fanout = min_fanout;
        prov.set("rules.god_module.min_fanout", source.clone());
    }
    if let Some(min_fanin) = file.min_fanin {
        rule.min_fanin = min_fanin;
        prov.set("rules.god_module.min_fanin", source.clone());
    }
    if let Some(relative_to_p90) = file.relative_to_p90 {
        rule.relative_to_p90 = relative_to_p90;
        prov.set("rules.god_module.relative_to_p90", source.clone());
    }
}

fn apply_circular_dep_config(
    rule: &mut CircularDependencyRule,
    file: &crate::config::schema::CircularDependencyRuleConfig,
    source: &Source,
    prov: &mut ProvenanceMap,
) {
    if let Some(enabled) = file.enabled {
        rule.enabled = enabled;
        prov.set("rules.circular_dependency.enabled", source.clone());
    }
    if let Some(warning_min_size) = file.warning_min_size {
        rule.warning_min_size = warning_min_size;
        prov.set("rules.circular_dependency.warning_min_size", source.clone());
    }
}

fn apply_deep_chain_config(
    rule: &mut DeepChainRule,
    file: &crate::config::schema::DeepChainRuleConfig,
    source: &Source,
    prov: &mut ProvenanceMap,
) {
    if let Some(enabled) = file.enabled {
        rule.enabled = enabled;
        prov.set("rules.deep_chain.enabled", source.clone());
    }
    if let Some(absolute_depth) = file.absolute_depth {
        rule.absolute_depth = absolute_depth;
        prov.set("rules.deep_chain.absolute_depth", source.clone());
    }
    if let Some(relative_multiplier) = file.relative_multiplier {
        rule.relative_multiplier = relative_multiplier;
        prov.set("rules.deep_chain.relative_multiplier", source.clone());
    }
    if let Some(relative_min_depth) = file.relative_min_depth {
        rule.relative_min_depth = relative_min_depth;
        prov.set("rules.deep_chain.relative_min_depth", source.clone());
    }
}

fn apply_high_entropy_config(
    rule: &mut HighEntropyRule,
    file: &crate::config::schema::HighEntropyRuleConfig,
    source: &Source,
    prov: &mut ProvenanceMap,
) {
    if let Some(enabled) = file.enabled {
        rule.enabled = enabled;
        prov.set("rules.high_entropy.enabled", source.clone());
    }
    if let Some(min_entropy) = file.min_entropy {
        rule.min_entropy = min_entropy;
        prov.set("rules.high_entropy.min_entropy", source.clone());
    }
    if let Some(min_fanout) = file.min_fanout {
        rule.min_fanout = min_fanout;
        prov.set("rules.high_entropy.min_fanout", source.clone());
    }
}

// Override-specific apply functions: override block replaces entire rule object,
// so we start from defaults and only set explicitly specified fields.
fn apply_high_fanout_override(
    rule: &mut HighFanoutRule,
    file: &crate::config::schema::HighFanoutRuleConfig,
) {
    if let Some(enabled) = file.enabled {
        rule.enabled = enabled;
    }
    if let Some(min_fanout) = file.min_fanout {
        rule.min_fanout = min_fanout;
    }
    if let Some(relative_to_p90) = file.relative_to_p90 {
        rule.relative_to_p90 = relative_to_p90;
    }
    if let Some(warning_multiplier) = file.warning_multiplier {
        rule.warning_multiplier = warning_multiplier;
    }
}

fn apply_god_module_override(
    rule: &mut GodModuleRule,
    file: &crate::config::schema::GodModuleRuleConfig,
) {
    if let Some(enabled) = file.enabled {
        rule.enabled = enabled;
    }
    if let Some(min_fanout) = file.min_fanout {
        rule.min_fanout = min_fanout;
    }
    if let Some(min_fanin) = file.min_fanin {
        rule.min_fanin = min_fanin;
    }
    if let Some(relative_to_p90) = file.relative_to_p90 {
        rule.relative_to_p90 = relative_to_p90;
    }
}

fn apply_circular_dep_override(
    rule: &mut CircularDependencyRule,
    file: &crate::config::schema::CircularDependencyRuleConfig,
) {
    if let Some(enabled) = file.enabled {
        rule.enabled = enabled;
    }
    if let Some(warning_min_size) = file.warning_min_size {
        rule.warning_min_size = warning_min_size;
    }
}

fn apply_deep_chain_override(
    rule: &mut DeepChainRule,
    file: &crate::config::schema::DeepChainRuleConfig,
) {
    if let Some(enabled) = file.enabled {
        rule.enabled = enabled;
    }
    if let Some(absolute_depth) = file.absolute_depth {
        rule.absolute_depth = absolute_depth;
    }
    if let Some(relative_multiplier) = file.relative_multiplier {
        rule.relative_multiplier = relative_multiplier;
    }
    if let Some(relative_min_depth) = file.relative_min_depth {
        rule.relative_min_depth = relative_min_depth;
    }
}

fn apply_high_entropy_override(
    rule: &mut HighEntropyRule,
    file: &crate::config::schema::HighEntropyRuleConfig,
) {
    if let Some(enabled) = file.enabled {
        rule.enabled = enabled;
    }
    if let Some(min_entropy) = file.min_entropy {
        rule.min_entropy = min_entropy;
    }
    if let Some(min_fanout) = file.min_fanout {
        rule.min_fanout = min_fanout;
    }
}

fn apply_env_vars(config: &mut ResolvedConfig, prov: &mut ProvenanceMap) {
    if let Ok(val) = std::env::var("UNTANGLE_FORMAT") {
        config.format = val;
        prov.set("defaults.format", Source::EnvVar("UNTANGLE_FORMAT".into()));
    }
    if let Ok(val) = std::env::var("UNTANGLE_LANG") {
        if let Ok(l) = val.parse::<Language>() {
            config.lang = Some(l);
            prov.set("defaults.lang", Source::EnvVar("UNTANGLE_LANG".into()));
        }
    }
    if let Ok(val) = std::env::var("UNTANGLE_QUIET") {
        config.quiet = val == "1" || val.eq_ignore_ascii_case("true");
        prov.set("defaults.quiet", Source::EnvVar("UNTANGLE_QUIET".into()));
    }
    if let Ok(val) = std::env::var("UNTANGLE_NO_INSIGHTS") {
        config.no_insights = val == "1" || val.eq_ignore_ascii_case("true");
        prov.set("defaults.no_insights", Source::EnvVar("UNTANGLE_NO_INSIGHTS".into()));
    }
    if let Ok(val) = std::env::var("UNTANGLE_TOP") {
        if let Ok(n) = val.parse::<usize>() {
            config.top = Some(n);
            prov.set("defaults.top", Source::EnvVar("UNTANGLE_TOP".into()));
        }
    }
    if let Ok(val) = std::env::var("UNTANGLE_INCLUDE_TESTS") {
        config.include_tests = val == "1" || val.eq_ignore_ascii_case("true");
        prov.set(
            "defaults.include_tests",
            Source::EnvVar("UNTANGLE_INCLUDE_TESTS".into()),
        );
    }
    if let Ok(val) = std::env::var("UNTANGLE_FAIL_ON") {
        config.fail_on = val.split(',').map(|s| s.trim().to_string()).collect();
        prov.set("fail_on.conditions", Source::EnvVar("UNTANGLE_FAIL_ON".into()));
    }
    if let Ok(val) = std::env::var("UNTANGLE_INCLUDE") {
        config.include = val.split(',').map(|s| s.trim().to_string()).collect();
        prov.set("targeting.include", Source::EnvVar("UNTANGLE_INCLUDE".into()));
    }
    if let Ok(val) = std::env::var("UNTANGLE_EXCLUDE") {
        config.exclude = val.split(',').map(|s| s.trim().to_string()).collect();
        prov.set("targeting.exclude", Source::EnvVar("UNTANGLE_EXCLUDE".into()));
    }
}

fn apply_cli_overrides(config: &mut ResolvedConfig, cli: &CliOverrides, prov: &mut ProvenanceMap) {
    if let Some(lang) = cli.lang {
        config.lang = Some(lang);
        prov.set("defaults.lang", Source::CliFlag("--lang".into()));
    }
    if let Some(format) = cli.format {
        config.format = format.to_string();
        prov.set("defaults.format", Source::CliFlag("--format".into()));
    }
    if cli.quiet {
        config.quiet = true;
        prov.set("defaults.quiet", Source::CliFlag("--quiet".into()));
    }
    if let Some(top) = cli.top {
        config.top = Some(top);
        prov.set("defaults.top", Source::CliFlag("--top".into()));
    }
    if cli.include_tests {
        config.include_tests = true;
        prov.set(
            "defaults.include_tests",
            Source::CliFlag("--include-tests".into()),
        );
    }
    if cli.no_insights {
        config.no_insights = true;
        prov.set(
            "defaults.no_insights",
            Source::CliFlag("--no-insights".into()),
        );
    }
    if !cli.include.is_empty() {
        config.include = cli.include.clone();
        prov.set("targeting.include", Source::CliFlag("--include".into()));
    }
    if !cli.exclude.is_empty() {
        config.exclude = cli.exclude.clone();
        prov.set("targeting.exclude", Source::CliFlag("--exclude".into()));
    }
    if !cli.fail_on.is_empty() {
        config.fail_on = cli.fail_on.clone();
        prov.set("fail_on.conditions", Source::CliFlag("--fail-on".into()));
    }
    if let Some(threshold_fanout) = cli.threshold_fanout {
        config.rules.high_fanout.min_fanout = threshold_fanout;
        config.rules.high_fanout.enabled = true;
        prov.set(
            "rules.high_fanout.min_fanout",
            Source::CliFlag("--threshold-fanout".into()),
        );
        prov.set(
            "rules.high_fanout.enabled",
            Source::CliFlag("--threshold-fanout".into()),
        );
    }
    if let Some(threshold_scc) = cli.threshold_scc {
        config.rules.circular_dependency.warning_min_size = threshold_scc;
        config.rules.circular_dependency.enabled = true;
        prov.set(
            "rules.circular_dependency.warning_min_size",
            Source::CliFlag("--threshold-scc".into()),
        );
        prov.set(
            "rules.circular_dependency.enabled",
            Source::CliFlag("--threshold-scc".into()),
        );
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Text => write!(f, "text"),
            OutputFormat::Dot => write!(f, "dot"),
            OutputFormat::Sarif => write!(f, "sarif"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn defaults_only() {
        let dir = PathBuf::from("/nonexistent");
        let cli = CliOverrides::default();
        let config = resolve_config(&dir, &cli).unwrap();

        assert_eq!(config.format, "json");
        assert!(!config.quiet);
        assert!(config.top.is_none());
        assert!(!config.include_tests);
        assert!(!config.no_insights);
        assert!(config.rules.high_fanout.enabled);
        assert_eq!(config.rules.high_fanout.min_fanout, 5);
        assert!(config.go.exclude_stdlib);
        assert!(config.python.resolve_relative);
    }

    #[test]
    fn cli_override_takes_precedence() {
        let dir = PathBuf::from("/nonexistent");
        let cli = CliOverrides {
            format: Some(OutputFormat::Text),
            quiet: true,
            top: Some(10),
            include_tests: true,
            no_insights: true,
            threshold_fanout: Some(20),
            ..Default::default()
        };
        let config = resolve_config(&dir, &cli).unwrap();

        assert_eq!(config.format, "text");
        assert!(config.quiet);
        assert_eq!(config.top, Some(10));
        assert!(config.include_tests);
        assert!(config.no_insights);
        assert_eq!(config.rules.high_fanout.min_fanout, 20);

        // Verify provenance
        assert!(matches!(
            config.provenance.get("defaults.format"),
            Some(Source::CliFlag(_))
        ));
    }

    #[test]
    fn project_config_applied() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(".untangle.toml");
        std::fs::write(
            &config_path,
            r#"
[defaults]
format = "text"
quiet = true
top = 15

[rules.high_fanout]
min_fanout = 10
"#,
        )
        .unwrap();

        let cli = CliOverrides::default();
        let config = resolve_config(tmp.path(), &cli).unwrap();

        assert_eq!(config.format, "text");
        assert!(config.quiet);
        assert_eq!(config.top, Some(15));
        assert_eq!(config.rules.high_fanout.min_fanout, 10);
        assert_eq!(config.loaded_files.len(), 1);
    }

    #[test]
    fn cli_overrides_project_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(".untangle.toml");
        std::fs::write(
            &config_path,
            r#"
[defaults]
format = "text"
"#,
        )
        .unwrap();

        let cli = CliOverrides {
            format: Some(OutputFormat::Json),
            ..Default::default()
        };
        let config = resolve_config(tmp.path(), &cli).unwrap();

        assert_eq!(config.format, "json");
        assert!(matches!(
            config.provenance.get("defaults.format"),
            Some(Source::CliFlag(_))
        ));
    }

    #[test]
    fn backward_compat_thresholds() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(".untangle.toml");
        std::fs::write(
            &config_path,
            r#"
[defaults]
lang = "python"
exclude = ["vendor/**"]

[thresholds]
max_fanout = 15
max_scc_size = 3

[fail_on]
conditions = ["fanout-increase"]
"#,
        )
        .unwrap();

        let cli = CliOverrides::default();
        let config = resolve_config(tmp.path(), &cli).unwrap();

        assert_eq!(config.rules.high_fanout.min_fanout, 15);
        assert_eq!(config.rules.circular_dependency.warning_min_size, 3);
        assert_eq!(config.fail_on, vec!["fanout-increase"]);
        assert_eq!(config.exclude, vec!["vendor/**"]);
    }

    #[test]
    fn services_resolved_from_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(".untangle.toml");
        std::fs::write(
            &config_path,
            r#"
[services.user-api]
root = "services/user-api"
lang = "go"
graphql_schemas = ["services/user-api/schema.graphql"]

[services.web-frontend]
root = "services/web-frontend"
lang = "python"
"#,
        )
        .unwrap();

        let cli = CliOverrides::default();
        let config = resolve_config(tmp.path(), &cli).unwrap();

        assert_eq!(config.services.len(), 2);

        let user_api = config
            .services
            .iter()
            .find(|s| s.name == "user-api")
            .unwrap();
        assert_eq!(user_api.root, PathBuf::from("services/user-api"));
        assert_eq!(user_api.lang, Some(Language::Go));
        assert_eq!(
            user_api.graphql_schemas,
            vec![PathBuf::from("services/user-api/schema.graphql")]
        );

        let web = config
            .services
            .iter()
            .find(|s| s.name == "web-frontend")
            .unwrap();
        assert_eq!(web.root, PathBuf::from("services/web-frontend"));
        assert_eq!(web.lang, Some(Language::Python));
    }
}
