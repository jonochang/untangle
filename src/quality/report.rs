use crate::analysis_context::resolve_project_root;
use crate::analysis_report::build_analysis_snapshot;
use crate::architecture::{
    self,
    policy::{self, ArchitectureCheckResult, ArchitectureComponentMetric, ArchitectureCycle, ArchitectureVerdict},
    ArchitectureEdge, ArchitectureEdgeRef, ArchitectureLayer, ArchitectureMetadata, ArchitectureNode,
};
use crate::config::ResolvedConfig;
use crate::errors::Result;
use crate::insights::Insight;
use crate::metrics::scc::SccInfo;
use crate::metrics::summary::Summary;
use crate::output::json::{build_hotspots, Hotspot, Metadata};
use crate::quality::engine::{self, QualityRunConfig};
use crate::quality::{QualityMetadata, QualityMetricKind, QualityResult};
use crate::walk::Language;
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

fn format_coverage_label(coverage_pct: Option<f64>) -> String {
    match coverage_pct {
        Some(value) => format!("{value:.1}%"),
        None => "N/A".to_string(),
    }
}

pub struct UnifiedRunConfig {
    pub root: PathBuf,
    pub lang: Option<Language>,
    pub coverage_file: Option<PathBuf>,
    pub top: Option<usize>,
    pub min_cc: usize,
    pub min_score: f64,
    pub architecture_level: Option<usize>,
    pub quiet: bool,
    pub resolved: ResolvedConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnifiedQualityReport {
    pub metadata: UnifiedQualityMetadata,
    pub structural: StructuralSection,
    pub functions: FunctionSection,
    pub architecture: ArchitectureSection,
    pub guidance: GuidanceSection,
    pub priorities: Vec<PriorityAction>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnifiedQualityMetadata {
    pub root: PathBuf,
    pub coverage_file: Option<PathBuf>,
    pub languages: Vec<String>,
    pub files_parsed: usize,
    pub functions: usize,
    pub timestamp: String,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StructuralSection {
    pub metadata: Metadata,
    pub summary: Summary,
    pub hotspots: Vec<Hotspot>,
    pub sccs: Vec<SccInfo>,
    pub insights: Vec<Insight>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionSection {
    pub metadata: QualityMetadata,
    pub summary: FunctionSummary,
    pub results: Vec<QualityResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionSummary {
    pub metric: QualityMetricKind,
    pub functions_reported: usize,
    pub mean_score: f64,
    pub p90_score: f64,
    pub max_score: f64,
    pub high_risk: usize,
    pub moderate_risk: usize,
    pub low_risk: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureSection {
    pub level: usize,
    pub metadata: ArchitectureMetadata,
    pub summary: ArchitectureSummary,
    pub nodes: Vec<ArchitectureNode>,
    pub edges: Vec<ArchitectureEdge>,
    pub feedback_edges: Vec<ArchitectureEdgeRef>,
    pub layers: Vec<ArchitectureLayer>,
    pub component_metrics: Vec<ArchitectureComponentMetric>,
    pub cycles: Vec<ArchitectureCycle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<ArchitecturePolicySummary>,
    pub dot: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureSummary {
    pub component_count: usize,
    pub dependency_count: usize,
    pub feedback_edge_count: usize,
    pub layer_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitecturePolicySummary {
    pub verdict: ArchitectureVerdict,
    pub violation_count: usize,
    pub cycle_count: usize,
    pub top_violations: Vec<policy::ArchitectureViolation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GuidanceSection {
    pub pressure: GuidancePressure,
    pub remediation_mode: GuidanceRemediationMode,
    pub ai_actionability: GuidanceActionability,
    pub ai_guidance: String,
    pub why: Vec<GuidanceLine>,
    #[serde(rename = "where")]
    pub where_: Vec<GuidanceHotspot>,
    pub how: Vec<GuidanceAction>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuidancePressure {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuidanceRemediationMode {
    Stable,
    Local,
    Split,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuidanceActionability {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize)]
pub struct GuidanceLine {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GuidanceHotspot {
    pub category: PriorityCategory,
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GuidanceAction {
    pub confidence: u8,
    pub label: String,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorityCategory {
    Structural,
    Function,
    Architecture,
}

#[derive(Debug, Clone, Serialize)]
pub struct PriorityAction {
    pub rank: usize,
    pub category: PriorityCategory,
    pub score: f64,
    pub title: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_components: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
}

pub fn run(config: UnifiedRunConfig) -> Result<UnifiedQualityReport> {
    let start = Instant::now();
    let function_metric = if config.coverage_file.is_some() {
        QualityMetricKind::Crap
    } else {
        QualityMetricKind::Complexity
    };
    let limit = config.top.or(config.resolved.quality_project.top);
    let project_root = resolve_project_root(&config.root, config.lang);

    let function_report = engine::run(QualityRunConfig {
        root: config.root.clone(),
        lang: config.lang,
        metric: function_metric,
        coverage_file: config.coverage_file.clone(),
        top: None,
        min_cc: config.min_cc,
        min_score: config.min_score,
        include_tests: config.resolved.include_tests,
        include: config.resolved.include.clone(),
        exclude: config.resolved.exclude.clone(),
        ignore_patterns: config.resolved.ignore_patterns.clone(),
        quiet: config.quiet,
    })?;

    let snapshot = build_analysis_snapshot(&config.root, &project_root, &config.resolved, false)?;
    let structural_hotspots = build_hotspots(&snapshot.graph, &snapshot.sccs, limit);
    let architecture_level = config
        .architecture_level
        .unwrap_or(config.resolved.analyze_architecture.level)
        .max(1);
    let architecture =
        architecture::project_architecture(&snapshot.graph, &project_root, architecture_level);
    let architecture_check = policy::check_graph(
        &snapshot.graph,
        &project_root,
        &config.resolved.analyze_architecture,
        Some(architecture_level),
    );
    let architecture_dot = render_architecture_dot(&architecture)?;
    let function_summary = summarize_function_results(function_metric, &function_report.results);
    let function_results = limit_results(function_report.results.clone(), limit);
    let priorities = prioritize(
        &snapshot,
        &structural_hotspots,
        &function_report.results,
        &architecture,
        limit.unwrap_or(10).max(1),
    );
    let guidance = build_guidance(
        &snapshot,
        &function_summary,
        &architecture,
        &architecture_check,
        &priorities,
    );

    Ok(UnifiedQualityReport {
        metadata: UnifiedQualityMetadata {
            root: config.root,
            coverage_file: config.coverage_file,
            languages: function_report.metadata.languages.clone(),
            files_parsed: snapshot.metadata.files_parsed,
            functions: function_report.metadata.functions,
            timestamp: chrono_now(),
            elapsed_ms: start.elapsed().as_millis() as u64,
        },
        structural: StructuralSection {
            metadata: snapshot.metadata,
            summary: snapshot.summary,
            hotspots: structural_hotspots,
            sccs: snapshot.sccs,
            insights: snapshot.insights.unwrap_or_default(),
        },
        functions: FunctionSection {
            metadata: function_report.metadata,
            summary: function_summary,
            results: function_results,
        },
        architecture: ArchitectureSection {
            level: architecture.level,
            metadata: architecture.metadata,
            summary: ArchitectureSummary {
                component_count: architecture.nodes.len(),
                dependency_count: architecture.edges.len(),
                feedback_edge_count: architecture.feedback_edges.len(),
                layer_count: architecture.layers.len(),
            },
            nodes: architecture.nodes,
            edges: architecture.edges,
            feedback_edges: architecture.feedback_edges,
            layers: architecture.layers,
            component_metrics: architecture_check.components.clone(),
            cycles: architecture_check.cycles.clone(),
            policy: architecture_policy_summary(&config.resolved, &architecture_check, limit),
            dot: architecture_dot,
        },
        guidance,
        priorities,
    })
}

fn render_architecture_dot(architecture: &architecture::ArchitectureOutput) -> Result<String> {
    let mut buf = Vec::new();
    architecture::write_dot(&mut buf, architecture)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn architecture_policy_summary(
    resolved: &ResolvedConfig,
    check: &ArchitectureCheckResult,
    limit: Option<usize>,
) -> Option<ArchitecturePolicySummary> {
    let has_policy = !resolved.analyze_architecture.allowed_dependencies.is_empty()
        || !resolved.analyze_architecture.forbidden_dependencies.is_empty()
        || !resolved.analyze_architecture.exceptions.is_empty()
        || !resolved.analyze_architecture.ignored_components.is_empty();
    if !has_policy {
        return None;
    }

    let mut top_violations = check.violations.clone();
    if let Some(limit) = limit {
        top_violations.truncate(limit.min(5));
    } else {
        top_violations.truncate(5);
    }

    Some(ArchitecturePolicySummary {
        verdict: check.summary.verdict.clone(),
        violation_count: check.summary.violation_count,
        cycle_count: check.summary.cycle_count,
        top_violations,
    })
}

fn build_guidance(
    snapshot: &crate::analysis_report::AnalysisSnapshot,
    functions: &FunctionSummary,
    architecture: &architecture::ArchitectureOutput,
    architecture_check: &ArchitectureCheckResult,
    priorities: &[PriorityAction],
) -> GuidanceSection {
    let structural_severe = snapshot.summary.scc_count > 0
        || snapshot.summary.max_fanout >= 4
        || snapshot.summary.total_complexity >= 12;
    let function_severe = functions.high_risk > 0 || functions.max_score >= 10.0;
    let architecture_severe = architecture_check.summary.violation_count > 0
        || architecture_check.summary.cycle_count > 0
        || !architecture.feedback_edges.is_empty();
    let severe_domains = [structural_severe, function_severe, architecture_severe]
        .into_iter()
        .filter(|flag| *flag)
        .count();

    let pressure = match severe_domains {
        0 => GuidancePressure::Low,
        1 => GuidancePressure::Medium,
        _ => GuidancePressure::High,
    };
    let distinct_priority_categories = priorities
        .iter()
        .take(3)
        .map(|action| priority_label(action.category))
        .collect::<Vec<_>>();
    let distinct_priority_categories = distinct_priority_categories
        .iter()
        .enumerate()
        .filter(|(idx, label)| distinct_priority_categories[..*idx].iter().all(|seen| seen != *label))
        .count();
    let remediation_mode = if severe_domains >= 2 || distinct_priority_categories >= 2 {
        GuidanceRemediationMode::Split
    } else if severe_domains == 0 {
        GuidanceRemediationMode::Stable
    } else {
        GuidanceRemediationMode::Local
    };
    let ai_actionability = match remediation_mode {
        GuidanceRemediationMode::Stable => GuidanceActionability::Low,
        GuidanceRemediationMode::Local => GuidanceActionability::Medium,
        GuidanceRemediationMode::Split => GuidanceActionability::High,
    };
    let ai_guidance = match remediation_mode {
        GuidanceRemediationMode::Stable => {
            "No immediate refactor is recommended; keep the current hotspots under observation."
        }
        GuidanceRemediationMode::Local => {
            "Target the top hotspot directly before attempting broad structural cleanup."
        }
        GuidanceRemediationMode::Split => {
            "Pressure is spread across multiple areas; split the work by concern before local tuning."
        }
    }
    .to_string();

    let why = vec![
        GuidanceLine {
            label: "max_fanout".to_string(),
            value: snapshot.summary.max_fanout.to_string(),
        },
        GuidanceLine {
            label: "scc_count".to_string(),
            value: snapshot.summary.scc_count.to_string(),
        },
        GuidanceLine {
            label: "high_risk_functions".to_string(),
            value: functions.high_risk.to_string(),
        },
        GuidanceLine {
            label: "architecture_feedback_edges".to_string(),
            value: architecture.feedback_edges.len().to_string(),
        },
        GuidanceLine {
            label: "architecture_violations".to_string(),
            value: architecture_check.summary.violation_count.to_string(),
        },
        GuidanceLine {
            label: "architecture_cycles".to_string(),
            value: architecture_check.summary.cycle_count.to_string(),
        },
    ];

    let where_ = priorities
        .iter()
        .take(3)
        .map(|action| GuidanceHotspot {
            category: action.category,
            title: action.title.clone(),
            summary: action.summary.clone(),
        })
        .collect();

    let mut how = Vec::new();
    if architecture_check.summary.violation_count > 0 {
        how.push(GuidanceAction {
            confidence: 3,
            label: "HIGH".to_string(),
            text: "Fix architecture boundary violations before tuning lower-level metrics."
                .to_string(),
        });
    }
    if architecture_check.summary.cycle_count > 0 || !architecture.feedback_edges.is_empty() {
        how.push(GuidanceAction {
            confidence: 3,
            label: "HIGH".to_string(),
            text: "Break component cycles or feedback edges before moving code between modules."
                .to_string(),
        });
    }
    if functions.high_risk > 0 {
        how.push(GuidanceAction {
            confidence: 3,
            label: "HIGH".to_string(),
            text: "Reduce the highest-risk functions before broad cleanup so behavior stays easier to validate."
                .to_string(),
        });
    }
    if snapshot.summary.scc_count > 0 {
        how.push(GuidanceAction {
            confidence: 2,
            label: "MEDIUM".to_string(),
            text: "Untangle module cycles before extracting helpers or renaming modules."
                .to_string(),
        });
    }
    if snapshot.summary.max_fanout >= 4 {
        how.push(GuidanceAction {
            confidence: 2,
            label: "MEDIUM".to_string(),
            text: "Reduce fan-out in hotspot modules instead of applying repository-wide cleanup."
                .to_string(),
        });
    }
    if how.is_empty() {
        how.push(GuidanceAction {
            confidence: 3,
            label: "HIGH".to_string(),
            text: "No immediate refactor is recommended; leave the current structure in place."
                .to_string(),
        });
    }
    how.truncate(4);

    GuidanceSection {
        pressure,
        remediation_mode,
        ai_actionability,
        ai_guidance,
        why,
        where_,
        how,
    }
}

fn summarize_function_results(
    metric: QualityMetricKind,
    results: &[QualityResult],
) -> FunctionSummary {
    if results.is_empty() {
        return FunctionSummary {
            metric,
            functions_reported: 0,
            mean_score: 0.0,
            p90_score: 0.0,
            max_score: 0.0,
            high_risk: 0,
            moderate_risk: 0,
            low_risk: 0,
        };
    }

    let mut scores: Vec<f64> = results.iter().map(|result| result.score).collect();
    scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mean = scores.iter().sum::<f64>() / scores.len() as f64;
    let p90_idx = (scores.len() as f64 * 0.9).ceil() as usize;
    let p90 = scores[p90_idx.min(scores.len()) - 1];
    let max = *scores.last().unwrap_or(&0.0);

    let mut high_risk = 0usize;
    let mut moderate_risk = 0usize;
    let mut low_risk = 0usize;
    for result in results {
        match result.risk_band.as_deref() {
            Some("high") => high_risk += 1,
            Some("moderate") => moderate_risk += 1,
            Some("low") => low_risk += 1,
            _ => {}
        }
    }

    FunctionSummary {
        metric,
        functions_reported: results.len(),
        mean_score: (mean * 100.0).round() / 100.0,
        p90_score: (p90 * 100.0).round() / 100.0,
        max_score: (max * 100.0).round() / 100.0,
        high_risk,
        moderate_risk,
        low_risk,
    }
}

fn limit_results(mut results: Vec<QualityResult>, limit: Option<usize>) -> Vec<QualityResult> {
    if let Some(limit) = limit {
        results.truncate(limit);
    }
    results
}

fn prioritize(
    snapshot: &crate::analysis_report::AnalysisSnapshot,
    hotspots: &[Hotspot],
    function_results: &[QualityResult],
    architecture: &architecture::ArchitectureOutput,
    limit: usize,
) -> Vec<PriorityAction> {
    let mut actions = Vec::new();
    let module_paths = module_path_map(snapshot);

    for edge in &architecture.feedback_edges {
        let aggregate = architecture
            .edges
            .iter()
            .find(|candidate| candidate.from == edge.from && candidate.to == edge.to);
        let edge_count = aggregate.map(|edge| edge.count).unwrap_or(1);
        let source_locations = aggregate
            .map(|edge| edge.source_location_count)
            .unwrap_or(0);
        actions.push(PriorityAction {
            rank: 0,
            category: PriorityCategory::Architecture,
            score: 90.0 + edge_count as f64 * 5.0,
            title: format!("Break architecture feedback from {} to {}", edge.from, edge.to),
            summary: format!(
                "Projected components '{}' and '{}' form a feedback relationship across {} aggregated edge(s).",
                edge.from, edge.to, edge_count
            ),
            file: None,
            module: None,
            function: None,
            related_components: vec![edge.from.clone(), edge.to.clone()],
            evidence: vec![format!(
                "{} source location(s) contribute to this projected dependency.",
                source_locations
            )],
        });
    }

    for result in function_results
        .iter()
        .filter(|result| result.score >= 5.0)
        .take(3)
    {
        actions.push(PriorityAction {
            rank: 0,
            category: PriorityCategory::Function,
            score: result.score * 2.0 + result.cyclomatic_complexity as f64,
            title: format!("Reduce {} score in {}", result.metric, result.function),
            summary: format!(
                "Function '{}' has cc={} and score={:.1} with {} coverage.",
                result.function,
                result.cyclomatic_complexity,
                result.score,
                format_coverage_label(result.coverage_pct)
            ),
            file: Some(result.file.clone()),
            module: None,
            function: Some(result.function.clone()),
            related_components: Vec::new(),
            evidence: vec![format!(
                "Lines {}-{} in {}.",
                result.start_line,
                result.end_line,
                result.file.display()
            )],
        });
    }

    for hotspot in hotspots.iter().filter(|hotspot| hotspot.fanout > 0).take(3) {
        actions.push(PriorityAction {
            rank: 0,
            category: PriorityCategory::Structural,
            score: hotspot.fanout as f64 * 10.0
                + hotspot.fanin as f64 * 3.0
                + hotspot.scc_adjusted_entropy,
            title: format!("Reduce module fan-out in {}", hotspot.node),
            summary: format!(
                "Module '{}' has fan-out={}, fan-in={}, entropy={:.2}.",
                hotspot.node, hotspot.fanout, hotspot.fanin, hotspot.entropy
            ),
            file: module_paths.get(&hotspot.node).cloned(),
            module: Some(hotspot.node.clone()),
            function: None,
            related_components: Vec::new(),
            evidence: hotspot
                .fanout_edges
                .iter()
                .take(3)
                .map(|edge| {
                    if let Some(location) = edge.source_locations.first() {
                        format!(
                            "Depends on {} at {}:{}.",
                            edge.to,
                            location.file.display(),
                            location.line
                        )
                    } else {
                        format!("Depends on {}.", edge.to)
                    }
                })
                .collect(),
        });
    }

    for insight in snapshot.insights.iter().flatten().take(3) {
        actions.push(PriorityAction {
            rank: 0,
            category: PriorityCategory::Structural,
            score: match insight.severity {
                crate::insights::InsightSeverity::Warning => 80.0,
                crate::insights::InsightSeverity::Info => 55.0,
            },
            title: insight.category.to_string(),
            summary: insight.message.clone(),
            file: module_paths.get(&insight.module).cloned(),
            module: Some(insight.module.clone()),
            function: None,
            related_components: Vec::new(),
            evidence: Vec::new(),
        });
    }

    actions.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.title.cmp(&b.title))
    });
    actions.truncate(limit);
    for (idx, action) in actions.iter_mut().enumerate() {
        action.rank = idx + 1;
    }
    actions
}

fn module_path_map(
    snapshot: &crate::analysis_report::AnalysisSnapshot,
) -> HashMap<String, PathBuf> {
    snapshot
        .graph
        .node_indices()
        .map(|idx| {
            (
                snapshot.graph[idx].name.clone(),
                snapshot.graph[idx].path.clone(),
            )
        })
        .collect()
}

fn chrono_now() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let days = (secs / 86400) as i64;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

impl std::fmt::Display for crate::insights::InsightCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            crate::insights::InsightCategory::GodModule => "god_module",
            crate::insights::InsightCategory::HighFanout => "high_fanout",
            crate::insights::InsightCategory::CircularDependency => "circular_dependency",
            crate::insights::InsightCategory::DeepChain => "deep_chain",
            crate::insights::InsightCategory::HighEntropy => "high_entropy",
        };
        write!(f, "{label}")
    }
}

pub fn write_json<W: Write>(writer: &mut W, report: &UnifiedQualityReport) -> Result<()> {
    serde_json::to_writer_pretty(
        writer,
        &serde_json::json!({
            "kind": "quality.report",
            "schema_version": 5,
            "report": report,
        }),
    )?;
    Ok(())
}

pub fn write_text<W: Write>(writer: &mut W, report: &UnifiedQualityReport) -> Result<()> {
    writeln!(writer, "Untangle Quality Report")?;
    writeln!(writer, "======================")?;
    writeln!(writer)?;
    writeln!(writer, "Root:      {}", report.metadata.root.display())?;
    writeln!(writer, "Files:     {}", report.metadata.files_parsed)?;
    writeln!(writer, "Functions: {}", report.metadata.functions)?;
    if let Some(ref coverage) = report.metadata.coverage_file {
        writeln!(writer, "Coverage:  {}", coverage.display())?;
    }
    writeln!(writer)?;
    write_guidance_section(writer, &report.guidance)?;
    write_priority_actions(writer, &report.priorities)?;
    write_structural_section(writer, &report.structural)?;
    write_function_section(writer, &report.functions)?;
    write_architecture_section(writer, &report.architecture)?;
    Ok(())
}

fn write_guidance_section<W: Write>(writer: &mut W, guidance: &GuidanceSection) -> Result<()> {
    writeln!(writer, "Guidance")?;
    writeln!(writer, "--------")?;
    writeln!(writer, "Pressure:         {}", guidance_pressure_label(guidance.pressure))?;
    writeln!(
        writer,
        "Remediation Mode: {}",
        guidance_remediation_mode_label(guidance.remediation_mode)
    )?;
    writeln!(
        writer,
        "AI Actionability: {}",
        guidance_actionability_label(guidance.ai_actionability)
    )?;
    writeln!(writer, "AI Guidance:      {}", guidance.ai_guidance)?;
    if !guidance.why.is_empty() {
        writeln!(writer, "Why:")?;
        for line in &guidance.why {
            writeln!(writer, "  - {}={}", line.label, line.value)?;
        }
    }
    if !guidance.where_.is_empty() {
        writeln!(writer, "Where:")?;
        for hotspot in &guidance.where_ {
            writeln!(
                writer,
                "  - [{}] {}",
                priority_label(hotspot.category),
                hotspot.title
            )?;
            writeln!(writer, "    {}", hotspot.summary)?;
        }
    }
    if !guidance.how.is_empty() {
        writeln!(writer, "How:")?;
        for action in &guidance.how {
            writeln!(writer, "  - {}: {}", action.label, action.text)?;
        }
    }
    writeln!(writer)?;
    Ok(())
}

fn write_priority_actions<W: Write>(writer: &mut W, actions: &[PriorityAction]) -> Result<()> {
    writeln!(writer, "Priority Actions")?;
    writeln!(writer, "----------------")?;
    for action in actions {
        write_priority_action(writer, action)?;
    }
    writeln!(writer)?;
    Ok(())
}

fn write_priority_action<W: Write>(writer: &mut W, action: &PriorityAction) -> Result<()> {
    writeln!(
        writer,
        "{}. [{}] {} ({:.1})",
        action.rank,
        priority_label(action.category),
        action.title,
        action.score
    )?;
    writeln!(writer, "   {}", action.summary)?;
    if let Some(location) = action_location(action) {
        writeln!(writer, "   Location: {location}")?;
    }
    if let Some(module) = action.module.as_deref() {
        writeln!(writer, "   Module: {module}")?;
    }
    if !action.related_components.is_empty() {
        writeln!(
            writer,
            "   Components: {}",
            action.related_components.join(", ")
        )?;
    }
    if !action.evidence.is_empty() {
        writeln!(writer, "   Evidence:")?;
        for item in &action.evidence {
            writeln!(writer, "     - {item}")?;
        }
    }
    Ok(())
}

fn write_structural_section<W: Write>(
    writer: &mut W,
    structural: &StructuralSection,
) -> Result<()> {
    writeln!(writer, "Structural Analysis")?;
    writeln!(writer, "-------------------")?;
    writeln!(writer, "Nodes:      {}", structural.metadata.node_count)?;
    writeln!(writer, "Edges:      {}", structural.metadata.edge_count)?;
    writeln!(
        writer,
        "Density:    {:.4}",
        structural.metadata.edge_density
    )?;
    writeln!(
        writer,
        "Fan-out:    mean={:.2} p90={} max={}",
        structural.summary.mean_fanout,
        structural.summary.p90_fanout,
        structural.summary.max_fanout
    )?;
    writeln!(
        writer,
        "Fan-in:     mean={:.2} p90={} max={}",
        structural.summary.mean_fanin, structural.summary.p90_fanin, structural.summary.max_fanin
    )?;
    writeln!(
        writer,
        "SCCs:       {} (largest: {})",
        structural.summary.scc_count, structural.summary.largest_scc_size
    )?;
    writeln!(writer)?;
    for hotspot in structural.hotspots.iter().take(5) {
        writeln!(
            writer,
            "  - {} fan-out={} fan-in={} entropy={:.2}",
            hotspot.node, hotspot.fanout, hotspot.fanin, hotspot.entropy
        )?;
    }
    writeln!(writer)?;
    Ok(())
}

fn write_function_section<W: Write>(writer: &mut W, functions: &FunctionSection) -> Result<()> {
    writeln!(writer, "Function Quality")?;
    writeln!(writer, "----------------")?;
    writeln!(writer, "Metric:     {}", functions.metadata.metric)?;
    writeln!(
        writer,
        "Scores:     mean={:.2} p90={:.2} max={:.2}",
        functions.summary.mean_score, functions.summary.p90_score, functions.summary.max_score
    )?;
    writeln!(
        writer,
        "Risk:       high={} moderate={} low={}",
        functions.summary.high_risk, functions.summary.moderate_risk, functions.summary.low_risk
    )?;
    writeln!(writer)?;
    for result in functions.results.iter().take(5) {
        let risk = result.risk_band.as_deref().unwrap_or("-");
        writeln!(
            writer,
            "  - {} {} score={:.1} cc={} cov={} risk={}",
            result.file.display(),
            result.function,
            result.score,
            result.cyclomatic_complexity,
            format_coverage_label(result.coverage_pct),
            risk
        )?;
    }
    writeln!(writer)?;
    Ok(())
}

fn write_architecture_section<W: Write>(
    writer: &mut W,
    architecture: &ArchitectureSection,
) -> Result<()> {
    writeln!(writer, "Architecture")?;
    writeln!(writer, "------------")?;
    writeln!(
        writer,
        "Components: {}  Edges: {}  Feedback: {}  Layers: {}",
        architecture.summary.component_count,
        architecture.summary.dependency_count,
        architecture.summary.feedback_edge_count,
        architecture.summary.layer_count
    )?;
    for layer in &architecture.layers {
        writeln!(
            writer,
            "  Layer {}: {}",
            layer.index,
            layer.nodes.join(", ")
        )?;
    }
    if !architecture.feedback_edges.is_empty() {
        writeln!(writer, "  Feedback edges:")?;
        for edge in &architecture.feedback_edges {
            writeln!(writer, "    {} -> {}", edge.from, edge.to)?;
        }
    }
    if !architecture.component_metrics.is_empty() {
        writeln!(writer, "  Component metrics:")?;
        for component in architecture.component_metrics.iter().take(5) {
            writeln!(
                writer,
                "    {} fan_in={} fan_out={} instability={:.3}",
                component.id, component.fan_in, component.fan_out, component.instability
            )?;
        }
    }
    if let Some(policy) = &architecture.policy {
        writeln!(
            writer,
            "  Policy: {:?} violations={} cycles={}",
            policy.verdict, policy.violation_count, policy.cycle_count
        )?;
        for violation in &policy.top_violations {
            writeln!(
                writer,
                "    violation: {} -> {} ({:?})",
                violation.from, violation.to, violation.kind
            )?;
        }
    }
    Ok(())
}

fn action_location(action: &PriorityAction) -> Option<String> {
    action.file.as_ref().map(|file| {
        let mut location = file.display().to_string();
        if let Some(function) = action.function.as_deref() {
            location.push_str("::");
            location.push_str(function);
        }
        location
    })
}

fn priority_label(category: PriorityCategory) -> &'static str {
    match category {
        PriorityCategory::Structural => "structural",
        PriorityCategory::Function => "function",
        PriorityCategory::Architecture => "architecture",
    }
}

fn guidance_pressure_label(pressure: GuidancePressure) -> &'static str {
    match pressure {
        GuidancePressure::Low => "low",
        GuidancePressure::Medium => "medium",
        GuidancePressure::High => "high",
    }
}

fn guidance_remediation_mode_label(mode: GuidanceRemediationMode) -> &'static str {
    match mode {
        GuidanceRemediationMode::Stable => "stable",
        GuidanceRemediationMode::Local => "local",
        GuidanceRemediationMode::Split => "split",
    }
}

fn guidance_actionability_label(actionability: GuidanceActionability) -> &'static str {
    match actionability {
        GuidanceActionability::Low => "low",
        GuidanceActionability::Medium => "medium",
        GuidanceActionability::High => "high",
    }
}
