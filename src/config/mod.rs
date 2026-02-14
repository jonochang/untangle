pub mod ignore;
pub mod overrides;
pub mod provenance;
pub mod resolve;
pub mod schema;
pub mod show;

use crate::walk::Language;
use provenance::ProvenanceMap;
use std::path::PathBuf;

/// Fully resolved configuration — no Option fields.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    // Operational
    pub lang: Option<Language>,
    pub format: String,
    pub quiet: bool,
    pub top: Option<usize>,
    pub include_tests: bool,
    pub no_insights: bool,

    // Targeting
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub ignore_patterns: Vec<String>,

    // Ruleset
    pub rules: ResolvedRules,

    // CI
    pub fail_on: Vec<String>,

    // Language-specific
    pub go: ResolvedGoConfig,
    pub python: ResolvedPythonConfig,
    pub ruby: ResolvedRubyConfig,

    // Per-path overrides (compiled globs)
    pub overrides: Vec<(globset::GlobMatcher, OverrideEntry)>,

    // Provenance
    pub provenance: ProvenanceMap,
    pub loaded_files: Vec<PathBuf>,
}

/// Entry for a per-path override.
#[derive(Debug, Clone)]
pub struct OverrideEntry {
    pub enabled: bool,
    pub rules: Option<ResolvedRules>,
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedRules {
    pub high_fanout: HighFanoutRule,
    pub god_module: GodModuleRule,
    pub circular_dependency: CircularDependencyRule,
    pub deep_chain: DeepChainRule,
    pub high_entropy: HighEntropyRule,
}

#[derive(Debug, Clone)]
pub struct HighFanoutRule {
    pub enabled: bool,
    pub min_fanout: usize,
    pub relative_to_p90: bool,
    pub warning_multiplier: usize,
}

impl Default for HighFanoutRule {
    fn default() -> Self {
        Self {
            enabled: true,
            min_fanout: 5,
            relative_to_p90: true,
            warning_multiplier: 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GodModuleRule {
    pub enabled: bool,
    pub min_fanout: usize,
    pub min_fanin: usize,
    pub relative_to_p90: bool,
}

impl Default for GodModuleRule {
    fn default() -> Self {
        Self {
            enabled: true,
            min_fanout: 3,
            min_fanin: 3,
            relative_to_p90: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CircularDependencyRule {
    pub enabled: bool,
    pub warning_min_size: usize,
}

impl Default for CircularDependencyRule {
    fn default() -> Self {
        Self {
            enabled: true,
            warning_min_size: 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeepChainRule {
    pub enabled: bool,
    pub absolute_depth: usize,
    pub relative_multiplier: f64,
    pub relative_min_depth: usize,
}

impl Default for DeepChainRule {
    fn default() -> Self {
        Self {
            enabled: true,
            absolute_depth: 8,
            relative_multiplier: 2.0,
            relative_min_depth: 5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HighEntropyRule {
    pub enabled: bool,
    pub min_entropy: f64,
    pub min_fanout: usize,
}

impl Default for HighEntropyRule {
    fn default() -> Self {
        Self {
            enabled: true,
            min_entropy: 2.5,
            min_fanout: 5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedGoConfig {
    pub exclude_stdlib: bool,
}

impl Default for ResolvedGoConfig {
    fn default() -> Self {
        Self {
            exclude_stdlib: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedPythonConfig {
    pub resolve_relative: bool,
}

impl Default for ResolvedPythonConfig {
    fn default() -> Self {
        Self {
            resolve_relative: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedRubyConfig {
    pub zeitwerk: bool,
    pub load_path: Vec<String>,
}

impl Default for ResolvedRubyConfig {
    fn default() -> Self {
        Self {
            zeitwerk: false,
            load_path: vec!["lib".to_string(), "app".to_string()],
        }
    }
}

impl ResolvedConfig {
    /// Get Ruby load paths as PathBufs, for backward compat with existing code.
    pub fn ruby_load_paths(&self) -> Vec<PathBuf> {
        self.ruby.load_path.iter().map(PathBuf::from).collect()
    }
}
