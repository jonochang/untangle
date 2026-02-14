use std::collections::BTreeMap;
use std::path::PathBuf;

/// Where a configuration value came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Default,
    UserConfig(PathBuf),
    ProjectConfig(PathBuf),
    EnvVar(String),
    CliFlag(String),
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::Default => write!(f, "default"),
            Source::UserConfig(path) => write!(f, "user config ({})", path.display()),
            Source::ProjectConfig(path) => write!(f, "project config ({})", path.display()),
            Source::EnvVar(name) => write!(f, "env var ({})", name),
            Source::CliFlag(name) => write!(f, "CLI flag ({})", name),
        }
    }
}

/// Tracks the source of each configuration value by dotted key.
#[derive(Debug, Clone, Default)]
pub struct ProvenanceMap {
    entries: BTreeMap<String, Source>,
}

impl ProvenanceMap {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, key: impl Into<String>, source: Source) {
        self.entries.insert(key.into(), source);
    }

    pub fn get(&self, key: &str) -> Option<&Source> {
        self.entries.get(key)
    }

    /// Return all entries sorted by key.
    pub fn sorted_entries(&self) -> Vec<(&str, &Source)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v)).collect()
    }

    /// Return entries matching a prefix.
    pub fn entries_with_prefix(&self, prefix: &str) -> Vec<(&str, &Source)> {
        self.entries
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.as_str(), v))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let mut map = ProvenanceMap::new();
        map.set("defaults.format", Source::Default);
        map.set(
            "rules.high_fanout.min_fanout",
            Source::ProjectConfig(PathBuf::from("/project/.untangle.toml")),
        );

        assert_eq!(map.get("defaults.format"), Some(&Source::Default));
        assert_eq!(
            map.get("rules.high_fanout.min_fanout"),
            Some(&Source::ProjectConfig(PathBuf::from(
                "/project/.untangle.toml"
            )))
        );
        assert_eq!(map.get("nonexistent"), None);
    }

    #[test]
    fn sorted_entries_order() {
        let mut map = ProvenanceMap::new();
        map.set("rules.high_fanout.min_fanout", Source::Default);
        map.set("defaults.format", Source::Default);
        map.set("defaults.quiet", Source::Default);

        let entries = map.sorted_entries();
        let keys: Vec<&str> = entries.iter().map(|(k, _)| *k).collect();
        assert_eq!(
            keys,
            vec![
                "defaults.format",
                "defaults.quiet",
                "rules.high_fanout.min_fanout"
            ]
        );
    }

    #[test]
    fn entries_with_prefix() {
        let mut map = ProvenanceMap::new();
        map.set("rules.high_fanout.enabled", Source::Default);
        map.set("rules.high_fanout.min_fanout", Source::Default);
        map.set("rules.god_module.enabled", Source::Default);
        map.set("defaults.format", Source::Default);

        let hf = map.entries_with_prefix("rules.high_fanout");
        assert_eq!(hf.len(), 2);

        let rules = map.entries_with_prefix("rules.");
        assert_eq!(rules.len(), 3);
    }

    #[test]
    fn display_sources() {
        assert_eq!(format!("{}", Source::Default), "default");
        assert_eq!(
            format!(
                "{}",
                Source::ProjectConfig(PathBuf::from("/project/.untangle.toml"))
            ),
            "project config (/project/.untangle.toml)"
        );
        assert_eq!(
            format!("{}", Source::EnvVar("UNTANGLE_FORMAT".to_string())),
            "env var (UNTANGLE_FORMAT)"
        );
        assert_eq!(
            format!("{}", Source::CliFlag("--format".to_string())),
            "CLI flag (--format)"
        );
    }
}
