use crate::errors::{Result, UntangleError};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Configuration loaded from `.untangle.toml`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub defaults: DefaultsConfig,
    #[serde(default)]
    pub thresholds: ThresholdsConfig,
    #[serde(default)]
    pub fail_on: FailOnConfig,
    #[serde(default)]
    pub python: PythonConfig,
    #[serde(default)]
    pub ruby: RubyConfig,
    #[serde(default)]
    pub go: GoConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DefaultsConfig {
    pub lang: Option<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ThresholdsConfig {
    pub max_fanout: Option<usize>,
    pub max_scc_size: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FailOnConfig {
    #[serde(default)]
    pub conditions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PythonConfig {
    pub granularity: Option<String>,
    #[serde(default = "default_true")]
    pub resolve_relative: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RubyConfig {
    #[serde(default)]
    pub zeitwerk: bool,
    #[serde(default)]
    pub load_path: Vec<String>,
}

/// Go-specific configuration.
///
/// `exclude_stdlib` defaults to true via custom `Default` implementation.
#[derive(Debug, Clone, Deserialize)]
pub struct GoConfig {
    #[serde(default = "default_true")]
    pub exclude_stdlib: bool,
}

impl Default for GoConfig {
    fn default() -> Self {
        Self {
            exclude_stdlib: true,
        }
    }
}

fn default_true() -> bool {
    true
}

impl Config {
    /// Load configuration from a file path.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|_| {
            UntangleError::Config(format!("Could not read config file: {}", path.display()))
        })?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| UntangleError::Config(format!("Invalid config file: {e}")))?;
        Ok(config)
    }

    /// Try to find and load `.untangle.toml` by walking up from the given directory.
    pub fn find_and_load(start: &Path) -> Option<Self> {
        let mut dir = start.to_path_buf();
        loop {
            let config_path = dir.join(".untangle.toml");
            if config_path.exists() {
                return Self::load(&config_path).ok();
            }
            if !dir.pop() {
                break;
            }
        }
        None
    }

    /// Return the exclude patterns (merge defaults from config).
    pub fn exclude_patterns(&self) -> &[String] {
        &self.defaults.exclude
    }

    /// Get the default language from config.
    pub fn default_language(&self) -> Option<&str> {
        self.defaults.lang.as_deref()
    }

    /// Get Ruby load paths, with defaults if not configured.
    pub fn ruby_load_paths(&self) -> Vec<PathBuf> {
        if self.ruby.load_path.is_empty() {
            vec![PathBuf::from("lib"), PathBuf::from("app")]
        } else {
            self.ruby.load_path.iter().map(PathBuf::from).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = Config::default();
        assert!(config.defaults.lang.is_none());
        assert!(config.defaults.exclude.is_empty());
        assert!(config.go.exclude_stdlib);
    }

    #[test]
    fn parse_config_toml() {
        let toml_str = r#"
[defaults]
lang = "python"
exclude = ["vendor/**"]

[thresholds]
max_fanout = 15

[fail_on]
conditions = ["fanout-increase", "new-scc"]

[go]
exclude_stdlib = true
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.defaults.lang.as_deref(), Some("python"));
        assert_eq!(config.defaults.exclude, vec!["vendor/**"]);
        assert_eq!(config.thresholds.max_fanout, Some(15));
        assert_eq!(config.fail_on.conditions.len(), 2);
    }
}
