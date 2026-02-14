use serde::Deserialize;
use std::collections::HashMap;

/// TOML-deserializable config file. All fields are Option for layered merging.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct FileConfig {
    #[serde(default)]
    pub defaults: DefaultsFileConfig,
    #[serde(default)]
    pub targeting: TargetingFileConfig,
    #[serde(default)]
    pub rules: RulesFileConfig,
    #[serde(default)]
    pub fail_on: FailOnFileConfig,
    #[serde(default)]
    pub go: GoFileConfig,
    #[serde(default)]
    pub python: PythonFileConfig,
    #[serde(default)]
    pub ruby: RubyFileConfig,
    #[serde(default)]
    pub overrides: HashMap<String, OverrideFileConfig>,

    // Backward compatibility: old format had [thresholds] section
    #[serde(default)]
    pub thresholds: Option<LegacyThresholdsConfig>,
}

/// Backward compat: old [defaults] had `exclude` which maps to targeting.exclude
/// and `lang` which maps to defaults.lang
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DefaultsFileConfig {
    pub lang: Option<String>,
    pub format: Option<String>,
    pub quiet: Option<bool>,
    pub top: Option<usize>,
    pub include_tests: Option<bool>,
    pub no_insights: Option<bool>,
    // Backward compat: old format had exclude here
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TargetingFileConfig {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RulesFileConfig {
    pub high_fanout: Option<HighFanoutRuleConfig>,
    pub god_module: Option<GodModuleRuleConfig>,
    pub circular_dependency: Option<CircularDependencyRuleConfig>,
    pub deep_chain: Option<DeepChainRuleConfig>,
    pub high_entropy: Option<HighEntropyRuleConfig>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HighFanoutRuleConfig {
    pub enabled: Option<bool>,
    pub min_fanout: Option<usize>,
    pub relative_to_p90: Option<bool>,
    pub warning_multiplier: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GodModuleRuleConfig {
    pub enabled: Option<bool>,
    pub min_fanout: Option<usize>,
    pub min_fanin: Option<usize>,
    pub relative_to_p90: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CircularDependencyRuleConfig {
    pub enabled: Option<bool>,
    pub warning_min_size: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DeepChainRuleConfig {
    pub enabled: Option<bool>,
    pub absolute_depth: Option<usize>,
    pub relative_multiplier: Option<f64>,
    pub relative_min_depth: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HighEntropyRuleConfig {
    pub enabled: Option<bool>,
    pub min_entropy: Option<f64>,
    pub min_fanout: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FailOnFileConfig {
    #[serde(default)]
    pub conditions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GoFileConfig {
    pub exclude_stdlib: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PythonFileConfig {
    pub resolve_relative: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RubyFileConfig {
    pub zeitwerk: Option<bool>,
    #[serde(default)]
    pub load_path: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct OverrideFileConfig {
    pub enabled: Option<bool>,
    pub rules: Option<RulesFileConfig>,
}

/// Legacy [thresholds] section for backward compatibility
#[derive(Debug, Clone, Deserialize, Default)]
pub struct LegacyThresholdsConfig {
    pub max_fanout: Option<usize>,
    pub max_scc_size: Option<usize>,
}

impl FileConfig {
    /// Load from a TOML string.
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    /// Migrate legacy fields into the new schema in-place.
    pub fn migrate_legacy(&mut self) {
        // [defaults].exclude -> [targeting].exclude (if targeting.exclude is empty)
        if self.targeting.exclude.is_empty() && !self.defaults.exclude.is_empty() {
            self.targeting.exclude = self.defaults.exclude.clone();
        }

        // [thresholds].max_fanout -> [rules.high_fanout].min_fanout
        if let Some(ref thresholds) = self.thresholds {
            if let Some(max_fanout) = thresholds.max_fanout {
                let rule = self.rules.high_fanout.get_or_insert_with(Default::default);
                if rule.min_fanout.is_none() {
                    rule.min_fanout = Some(max_fanout);
                }
            }
            if let Some(max_scc_size) = thresholds.max_scc_size {
                let rule = self
                    .rules
                    .circular_dependency
                    .get_or_insert_with(Default::default);
                if rule.warning_min_size.is_none() {
                    rule.warning_min_size = Some(max_scc_size);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_full_config() {
        let toml_str = r#"
[defaults]
lang = "go"
format = "json"
quiet = false
top = 20
include_tests = false
no_insights = false

[targeting]
include = ["src/**"]
exclude = ["vendor/**"]

[rules.high_fanout]
enabled = true
min_fanout = 5
relative_to_p90 = true
warning_multiplier = 2

[rules.god_module]
enabled = true
min_fanout = 3
min_fanin = 3
relative_to_p90 = true

[rules.circular_dependency]
enabled = true
warning_min_size = 4

[rules.deep_chain]
enabled = true
absolute_depth = 8
relative_multiplier = 2.0
relative_min_depth = 5

[rules.high_entropy]
enabled = true
min_entropy = 2.5
min_fanout = 5

[fail_on]
conditions = ["fanout-increase", "new-scc"]

[go]
exclude_stdlib = true

[python]
resolve_relative = true

[ruby]
zeitwerk = false
load_path = ["lib", "app"]

[overrides."**/vendor/**"]
enabled = false

[overrides."src/legacy/**"]
rules.high_fanout.min_fanout = 40
rules.high_fanout.relative_to_p90 = false
"#;
        let config = FileConfig::from_toml(toml_str).unwrap();
        assert_eq!(config.defaults.lang.as_deref(), Some("go"));
        assert_eq!(config.defaults.format.as_deref(), Some("json"));
        assert_eq!(config.defaults.top, Some(20));
        assert_eq!(config.targeting.include, vec!["src/**"]);
        assert_eq!(config.targeting.exclude, vec!["vendor/**"]);

        let hf = config.rules.high_fanout.as_ref().unwrap();
        assert_eq!(hf.enabled, Some(true));
        assert_eq!(hf.min_fanout, Some(5));

        let gm = config.rules.god_module.as_ref().unwrap();
        assert_eq!(gm.min_fanin, Some(3));

        assert_eq!(config.fail_on.conditions.len(), 2);
        assert_eq!(config.go.exclude_stdlib, Some(true));
        assert_eq!(config.python.resolve_relative, Some(true));
        assert_eq!(config.ruby.load_path, vec!["lib", "app"]);

        assert_eq!(config.overrides.len(), 2);
        let vendor = &config.overrides["**/vendor/**"];
        assert_eq!(vendor.enabled, Some(false));

        let legacy = &config.overrides["src/legacy/**"];
        let lr = legacy.rules.as_ref().unwrap();
        let lhf = lr.high_fanout.as_ref().unwrap();
        assert_eq!(lhf.min_fanout, Some(40));
        assert_eq!(lhf.relative_to_p90, Some(false));
    }

    #[test]
    fn deserialize_empty_config() {
        let config = FileConfig::from_toml("").unwrap();
        assert!(config.defaults.lang.is_none());
        assert!(config.targeting.include.is_empty());
        assert!(config.rules.high_fanout.is_none());
        assert!(config.overrides.is_empty());
    }

    #[test]
    fn deserialize_partial_config() {
        let toml_str = r#"
[defaults]
lang = "python"

[rules.high_fanout]
min_fanout = 10
"#;
        let config = FileConfig::from_toml(toml_str).unwrap();
        assert_eq!(config.defaults.lang.as_deref(), Some("python"));
        assert!(config.defaults.format.is_none());

        let hf = config.rules.high_fanout.as_ref().unwrap();
        assert_eq!(hf.min_fanout, Some(10));
        assert!(hf.enabled.is_none());
    }

    #[test]
    fn backward_compat_old_format() {
        let toml_str = r#"
[defaults]
lang = "python"
exclude = ["vendor/**"]

[thresholds]
max_fanout = 15
max_scc_size = 3

[fail_on]
conditions = ["fanout-increase", "new-scc"]

[go]
exclude_stdlib = true
"#;
        let mut config = FileConfig::from_toml(toml_str).unwrap();
        config.migrate_legacy();

        assert_eq!(config.defaults.lang.as_deref(), Some("python"));
        // Old exclude should migrate to targeting.exclude
        assert_eq!(config.targeting.exclude, vec!["vendor/**"]);
        // Old thresholds should migrate
        let hf = config.rules.high_fanout.as_ref().unwrap();
        assert_eq!(hf.min_fanout, Some(15));
        let cd = config.rules.circular_dependency.as_ref().unwrap();
        assert_eq!(cd.warning_min_size, Some(3));
    }
}
