pub mod ignore;
pub mod overrides;
pub mod provenance;
pub mod resolve;
pub mod schema;
pub mod show;

use crate::formats::{
    AnalyzeReportFormat, ArchitectureCheckFormat, ArchitectureFormat, DiffFormat, GraphFormat, QualityFormat,
    ServiceGraphFormat,
};
use crate::walk::Language;
use provenance::ProvenanceMap;
use serde::Serialize;
use std::path::PathBuf;

pub(crate) mod keys {
    pub const DEFAULTS_LANG: &str = "defaults.lang";
    pub const DEFAULTS_QUIET: &str = "defaults.quiet";
    pub const DEFAULTS_INCLUDE_TESTS: &str = "defaults.include_tests";
    pub const ANALYZE_REPORT_FORMAT: &str = "analyze.report.format";
    pub const ANALYZE_REPORT_TOP: &str = "analyze.report.top";
    pub const ANALYZE_REPORT_INSIGHTS: &str = "analyze.report.insights";
    pub const ANALYZE_REPORT_THRESHOLD_FANOUT: &str = "analyze.report.threshold_fanout";
    pub const ANALYZE_REPORT_THRESHOLD_SCC: &str = "analyze.report.threshold_scc";
    pub const ANALYZE_GRAPH_FORMAT: &str = "analyze.graph.format";
    pub const ANALYZE_ARCHITECTURE_FORMAT: &str = "analyze.architecture.format";
    pub const ANALYZE_ARCHITECTURE_LEVEL: &str = "analyze.architecture.level";
    pub const ANALYZE_ARCHITECTURE_CHECK_FORMAT: &str = "analyze.architecture.check_format";
    pub const ANALYZE_ARCHITECTURE_FAIL_ON_VIOLATIONS: &str =
        "analyze.architecture.fail_on_violations";
    pub const ANALYZE_ARCHITECTURE_FAIL_ON_CYCLES: &str =
        "analyze.architecture.fail_on_cycles";
    pub const ANALYZE_ARCHITECTURE_IGNORED_COMPONENTS: &str =
        "analyze.architecture.ignored_components";
    pub const ANALYZE_ARCHITECTURE_ALLOWED_DEPENDENCIES: &str =
        "analyze.architecture.allowed_dependencies";
    pub const ANALYZE_ARCHITECTURE_FORBIDDEN_DEPENDENCIES: &str =
        "analyze.architecture.forbidden_dependencies";
    pub const ANALYZE_ARCHITECTURE_EXCEPTIONS: &str = "analyze.architecture.exceptions";
    pub const DIFF_FORMAT: &str = "diff.format";
    pub const QUALITY_FUNCTIONS_FORMAT: &str = "quality.functions.format";
    pub const QUALITY_FUNCTIONS_TOP: &str = "quality.functions.top";
    pub const QUALITY_PROJECT_FORMAT: &str = "quality.project.format";
    pub const QUALITY_PROJECT_TOP: &str = "quality.project.top";
    pub const SERVICE_GRAPH_FORMAT: &str = "service_graph.format";
    pub const RULES_HIGH_FANOUT_ENABLED: &str = "rules.high_fanout.enabled";
    pub const RULES_HIGH_FANOUT_MIN_FANOUT: &str = "rules.high_fanout.min_fanout";
    pub const RULES_HIGH_FANOUT_RELATIVE_TO_P90: &str = "rules.high_fanout.relative_to_p90";
    pub const RULES_HIGH_FANOUT_WARNING_MULTIPLIER: &str = "rules.high_fanout.warning_multiplier";
    pub const RULES_GOD_MODULE_ENABLED: &str = "rules.god_module.enabled";
    pub const RULES_GOD_MODULE_MIN_FANOUT: &str = "rules.god_module.min_fanout";
    pub const RULES_GOD_MODULE_MIN_FANIN: &str = "rules.god_module.min_fanin";
    pub const RULES_GOD_MODULE_RELATIVE_TO_P90: &str = "rules.god_module.relative_to_p90";
    pub const RULES_CIRCULAR_DEPENDENCY_ENABLED: &str = "rules.circular_dependency.enabled";
    pub const RULES_CIRCULAR_DEPENDENCY_WARNING_MIN_SIZE: &str =
        "rules.circular_dependency.warning_min_size";
    pub const RULES_DEEP_CHAIN_ENABLED: &str = "rules.deep_chain.enabled";
    pub const RULES_DEEP_CHAIN_ABSOLUTE_DEPTH: &str = "rules.deep_chain.absolute_depth";
    pub const RULES_DEEP_CHAIN_RELATIVE_MULTIPLIER: &str = "rules.deep_chain.relative_multiplier";
    pub const RULES_DEEP_CHAIN_RELATIVE_MIN_DEPTH: &str = "rules.deep_chain.relative_min_depth";
    pub const RULES_HIGH_ENTROPY_ENABLED: &str = "rules.high_entropy.enabled";
    pub const RULES_HIGH_ENTROPY_MIN_ENTROPY: &str = "rules.high_entropy.min_entropy";
    pub const RULES_HIGH_ENTROPY_MIN_FANOUT: &str = "rules.high_entropy.min_fanout";
    pub const GO_EXCLUDE_STDLIB: &str = "go.exclude_stdlib";
    pub const PYTHON_RESOLVE_RELATIVE: &str = "python.resolve_relative";
    pub const RUBY_ZEITWERK: &str = "ruby.zeitwerk";
    pub const RUBY_LOAD_PATH: &str = "ruby.load_path";

    pub const ALL: &[&str] = &[
        DEFAULTS_LANG,
        DEFAULTS_QUIET,
        DEFAULTS_INCLUDE_TESTS,
        ANALYZE_REPORT_FORMAT,
        ANALYZE_REPORT_TOP,
        ANALYZE_REPORT_INSIGHTS,
        ANALYZE_REPORT_THRESHOLD_FANOUT,
        ANALYZE_REPORT_THRESHOLD_SCC,
        ANALYZE_GRAPH_FORMAT,
        ANALYZE_ARCHITECTURE_FORMAT,
        ANALYZE_ARCHITECTURE_LEVEL,
        ANALYZE_ARCHITECTURE_CHECK_FORMAT,
        ANALYZE_ARCHITECTURE_FAIL_ON_VIOLATIONS,
        ANALYZE_ARCHITECTURE_FAIL_ON_CYCLES,
        ANALYZE_ARCHITECTURE_IGNORED_COMPONENTS,
        ANALYZE_ARCHITECTURE_ALLOWED_DEPENDENCIES,
        ANALYZE_ARCHITECTURE_FORBIDDEN_DEPENDENCIES,
        ANALYZE_ARCHITECTURE_EXCEPTIONS,
        DIFF_FORMAT,
        QUALITY_FUNCTIONS_FORMAT,
        QUALITY_FUNCTIONS_TOP,
        QUALITY_PROJECT_FORMAT,
        QUALITY_PROJECT_TOP,
        SERVICE_GRAPH_FORMAT,
        RULES_HIGH_FANOUT_ENABLED,
        RULES_HIGH_FANOUT_MIN_FANOUT,
        RULES_HIGH_FANOUT_RELATIVE_TO_P90,
        RULES_HIGH_FANOUT_WARNING_MULTIPLIER,
        RULES_GOD_MODULE_ENABLED,
        RULES_GOD_MODULE_MIN_FANOUT,
        RULES_GOD_MODULE_MIN_FANIN,
        RULES_GOD_MODULE_RELATIVE_TO_P90,
        RULES_CIRCULAR_DEPENDENCY_ENABLED,
        RULES_CIRCULAR_DEPENDENCY_WARNING_MIN_SIZE,
        RULES_DEEP_CHAIN_ENABLED,
        RULES_DEEP_CHAIN_ABSOLUTE_DEPTH,
        RULES_DEEP_CHAIN_RELATIVE_MULTIPLIER,
        RULES_DEEP_CHAIN_RELATIVE_MIN_DEPTH,
        RULES_HIGH_ENTROPY_ENABLED,
        RULES_HIGH_ENTROPY_MIN_ENTROPY,
        RULES_HIGH_ENTROPY_MIN_FANOUT,
        GO_EXCLUDE_STDLIB,
        PYTHON_RESOLVE_RELATIVE,
        RUBY_ZEITWERK,
        RUBY_LOAD_PATH,
    ];
}

/// Fully resolved configuration — no Option fields.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    // Operational
    pub lang: Option<Language>,
    pub quiet: bool,
    pub include_tests: bool,

    // Targeting
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub ignore_patterns: Vec<String>,

    // Command defaults
    pub analyze_report: ResolvedAnalyzeReportConfig,
    pub analyze_graph: ResolvedGraphConfig,
    pub analyze_architecture: ResolvedArchitectureConfig,
    pub diff: ResolvedDiffConfig,
    pub quality_functions: ResolvedQualityConfig,
    pub quality_project: ResolvedQualityConfig,
    pub service_graph: ResolvedServiceGraphConfig,

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

    // Services for cross-service dependency tracking
    pub services: Vec<ResolvedService>,

    // Provenance
    pub provenance: ProvenanceMap,
    pub loaded_files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ResolvedAnalyzeReportConfig {
    pub format: AnalyzeReportFormat,
    pub top: Option<usize>,
    pub insights: InsightsConfig,
    pub threshold_fanout: Option<usize>,
    pub threshold_scc: Option<usize>,
}

impl Default for ResolvedAnalyzeReportConfig {
    fn default() -> Self {
        Self {
            format: AnalyzeReportFormat::Json,
            top: None,
            insights: InsightsConfig::Auto,
            threshold_fanout: None,
            threshold_scc: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedGraphConfig {
    pub format: GraphFormat,
}

impl Default for ResolvedGraphConfig {
    fn default() -> Self {
        Self {
            format: GraphFormat::Dot,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedArchitectureConfig {
    pub format: ArchitectureFormat,
    pub level: usize,
    pub check_format: ArchitectureCheckFormat,
    pub fail_on_violations: bool,
    pub fail_on_cycles: bool,
    pub ignored_components: Vec<String>,
    pub allowed_dependencies: std::collections::BTreeMap<String, Vec<String>>,
    pub forbidden_dependencies: Vec<ArchitectureForbiddenDependency>,
    pub exceptions: Vec<ArchitectureException>,
}

impl Default for ResolvedArchitectureConfig {
    fn default() -> Self {
        Self {
            format: ArchitectureFormat::Dot,
            level: 1,
            check_format: ArchitectureCheckFormat::Text,
            fail_on_violations: true,
            fail_on_cycles: true,
            ignored_components: Vec::new(),
            allowed_dependencies: std::collections::BTreeMap::new(),
            forbidden_dependencies: Vec::new(),
            exceptions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ArchitectureForbiddenDependency {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ArchitectureException {
    pub from_component: Option<String>,
    pub to_component: Option<String>,
    pub from_module: Option<String>,
    pub to_module: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedDiffConfig {
    pub format: DiffFormat,
}

impl Default for ResolvedDiffConfig {
    fn default() -> Self {
        Self {
            format: DiffFormat::Json,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedQualityConfig {
    pub format: QualityFormat,
    pub top: Option<usize>,
}

impl Default for ResolvedQualityConfig {
    fn default() -> Self {
        Self {
            format: QualityFormat::Json,
            top: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedServiceGraphConfig {
    pub format: ServiceGraphFormat,
}

impl Default for ResolvedServiceGraphConfig {
    fn default() -> Self {
        Self {
            format: ServiceGraphFormat::Json,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InsightsConfig {
    Auto,
    On,
    Off,
}

/// Resolved service declaration for cross-service dependency tracking.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedService {
    pub name: String,
    pub root: PathBuf,
    pub lang: Option<Language>,
    pub graphql_schemas: Vec<PathBuf>,
    pub openapi_specs: Vec<PathBuf>,
    pub base_urls: Vec<String>,
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
