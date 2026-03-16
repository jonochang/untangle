use crate::architecture::{layer_map, project_architecture, project_component_id, ArchitectureOutput};
use crate::config::{ArchitectureException, ResolvedArchitectureConfig};
use crate::errors::{Result, UntangleError};
use crate::graph::ir::DepGraph;
use crate::parse::common::SourceLocation;
use petgraph::algo::tarjan_scc;
use petgraph::graph::DiGraph;
use petgraph::visit::EdgeRef;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use toml_edit::{value, Array, ArrayOfTables, DocumentMut, Item, Table};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArchitectureVerdict {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArchitectureViolationKind {
    Allowlist,
    ForbiddenRule,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureCheckMetadata {
    pub root: PathBuf,
    pub level: usize,
    pub source_node_count: usize,
    pub source_edge_count: usize,
    pub component_count: usize,
    pub dependency_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureCheckSummary {
    pub verdict: ArchitectureVerdict,
    pub violation_count: usize,
    pub cycle_count: usize,
    pub ignored_component_count: usize,
    pub waived_dependency_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureComponentMetric {
    pub id: String,
    pub layer: usize,
    pub module_count: usize,
    pub fan_in: usize,
    pub fan_out: usize,
    pub instability: f64,
    pub feedback: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureDependency {
    pub from: String,
    pub to: String,
    pub count: usize,
    pub source_location_count: usize,
    pub feedback: bool,
    pub violated: bool,
    pub waived: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureViolationEvidence {
    pub from_module: String,
    pub to_module: String,
    pub source_locations: Vec<SourceLocation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureViolation {
    pub from: String,
    pub to: String,
    pub kind: ArchitectureViolationKind,
    pub evidence: Vec<ArchitectureViolationEvidence>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureCycle {
    pub members: Vec<String>,
    pub size: usize,
    pub edge_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureCheckResult {
    pub metadata: ArchitectureCheckMetadata,
    pub summary: ArchitectureCheckSummary,
    pub components: Vec<ArchitectureComponentMetric>,
    pub dependencies: Vec<ArchitectureDependency>,
    pub violations: Vec<ArchitectureViolation>,
    pub cycles: Vec<ArchitectureCycle>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StarterArchitecturePolicy {
    pub level: usize,
    pub fail_on_violations: bool,
    pub fail_on_cycles: bool,
    pub allowed_dependencies: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Default)]
struct DependencyStats {
    count: usize,
    source_location_count: usize,
}

type ComponentEdge = (String, String);
type ModuleEdge = (String, String);

struct ProjectionData {
    projected: ArchitectureOutput,
    active_components: BTreeSet<String>,
    dependency_stats: BTreeMap<ComponentEdge, DependencyStats>,
    evidence: BTreeMap<ComponentEdge, BTreeMap<ModuleEdge, Vec<SourceLocation>>>,
    feedback_edges: BTreeSet<ComponentEdge>,
}

pub fn check_graph(
    graph: &DepGraph,
    root: &Path,
    config: &ResolvedArchitectureConfig,
    level_override: Option<usize>,
) -> ArchitectureCheckResult {
    let level = level_override.unwrap_or(config.level).max(1);
    let data = collect_projection_data(graph, root, level, &config.ignored_components);
    let cycles = collect_cycles(&data.active_components, &data.dependency_stats);
    let feedback_members = feedback_members(&data.feedback_edges);
    let components = collect_component_metrics(&data, &feedback_members);
    let (dependencies, violations, waived_dependency_count) = evaluate_dependencies(&data, config);
    let verdict = if (!violations.is_empty() && config.fail_on_violations)
        || (!cycles.is_empty() && config.fail_on_cycles)
    {
        ArchitectureVerdict::Fail
    } else {
        ArchitectureVerdict::Pass
    };

    ArchitectureCheckResult {
        metadata: ArchitectureCheckMetadata {
            root: root.to_path_buf(),
            level,
            source_node_count: data.projected.metadata.source_node_count,
            source_edge_count: data.projected.metadata.source_edge_count,
            component_count: components.len(),
            dependency_count: dependencies.len(),
        },
        summary: ArchitectureCheckSummary {
            verdict,
            violation_count: violations.len(),
            cycle_count: cycles.len(),
            ignored_component_count: config.ignored_components.len(),
            waived_dependency_count,
        },
        components,
        dependencies,
        violations,
        cycles,
    }
}

pub fn infer_starter_policy(
    graph: &DepGraph,
    root: &Path,
    config: &ResolvedArchitectureConfig,
    level_override: Option<usize>,
) -> StarterArchitecturePolicy {
    let level = level_override.unwrap_or(config.level).max(1);
    let data = collect_projection_data(graph, root, level, &config.ignored_components);
    let mut allowed_dependencies = BTreeMap::new();

    for component in &data.active_components {
        allowed_dependencies.insert(component.clone(), Vec::new());
    }

    for (edge, _) in &data.dependency_stats {
        allowed_dependencies
            .entry(edge.0.clone())
            .or_default()
            .push(edge.1.clone());
    }

    for deps in allowed_dependencies.values_mut() {
        deps.sort();
        deps.dedup();
    }

    StarterArchitecturePolicy {
        level,
        fail_on_violations: true,
        fail_on_cycles: true,
        allowed_dependencies,
    }
}

pub fn write_check_json<W: Write>(writer: &mut W, result: &ArchitectureCheckResult) -> Result<()> {
    serde_json::to_writer_pretty(
        writer,
        &serde_json::json!({
            "kind": "analyze.architecture.check",
            "schema_version": 2,
            "metadata": result.metadata,
            "summary": result.summary,
            "components": result.components,
            "dependencies": result.dependencies,
            "violations": result.violations,
            "cycles": result.cycles,
        }),
    )?;
    Ok(())
}

pub fn write_check_text<W: Write>(writer: &mut W, result: &ArchitectureCheckResult) -> Result<()> {
    writeln!(writer, "Untangle Architecture Check")?;
    writeln!(writer, "==========================")?;
    writeln!(writer)?;
    writeln!(writer, "Root: {}", result.metadata.root.display())?;
    writeln!(writer, "Level: {}", result.metadata.level)?;
    writeln!(writer, "Components: {}", result.metadata.component_count)?;
    writeln!(writer, "Dependencies: {}", result.metadata.dependency_count)?;
    writeln!(writer, "Verdict: {:?}", result.summary.verdict)?;
    writeln!(writer)?;

    writeln!(writer, "Component Metrics")?;
    writeln!(writer, "-----------------")?;
    writeln!(
        writer,
        "{:<24} {:>6} {:>7} {:>7} {:>12} {:>9}",
        "Component", "Layer", "Modules", "FanIn", "FanOut", "Feedback"
    )?;
    for component in &result.components {
        writeln!(
            writer,
            "{:<24} {:>6} {:>7} {:>7} {:>12} {:>9}",
            component.id,
            component.layer,
            component.module_count,
            component.fan_in,
            component.fan_out,
            if component.feedback { "yes" } else { "no" }
        )?;
    }
    writeln!(writer)?;

    if !result.dependencies.is_empty() {
        writeln!(writer, "Component Dependencies")?;
        writeln!(writer, "----------------------")?;
        for dependency in &result.dependencies {
            let mut suffix = String::new();
            if dependency.feedback {
                suffix.push_str(" feedback");
            }
            if dependency.violated {
                suffix.push_str(" violation");
            } else if dependency.waived {
                suffix.push_str(" waived");
            }
            writeln!(
                writer,
                "{} -> {}  (count={}, sources={}){}",
                dependency.from,
                dependency.to,
                dependency.count,
                dependency.source_location_count,
                suffix
            )?;
        }
        writeln!(writer)?;
    }

    if !result.violations.is_empty() {
        writeln!(writer, "Boundary Violations")?;
        writeln!(writer, "-------------------")?;
        for violation in &result.violations {
            writeln!(
                writer,
                "{} -> {} ({:?})",
                violation.from, violation.to, violation.kind
            )?;
            for evidence in &violation.evidence {
                writeln!(
                    writer,
                    "  {} -> {}",
                    evidence.from_module, evidence.to_module
                )?;
                for location in &evidence.source_locations {
                    match location.column {
                        Some(column) => writeln!(
                            writer,
                            "    {}:{}:{}",
                            location.file.display(),
                            location.line,
                            column
                        )?,
                        None => writeln!(writer, "    {}:{}", location.file.display(), location.line)?,
                    }
                }
            }
        }
        writeln!(writer)?;
    }

    if !result.cycles.is_empty() {
        writeln!(writer, "Cycles")?;
        writeln!(writer, "------")?;
        for cycle in &result.cycles {
            writeln!(
                writer,
                "{} (size={}, edges={})",
                cycle.members.join(" -> "),
                cycle.size,
                cycle.edge_count
            )?;
        }
        writeln!(writer)?;
    }

    Ok(())
}

pub fn write_starter_policy_file(
    config_path: &Path,
    policy: &StarterArchitecturePolicy,
    force: bool,
) -> Result<()> {
    let mut doc = if config_path.exists() {
        let content = std::fs::read_to_string(config_path)?;
        content
            .parse::<DocumentMut>()
            .map_err(|e| UntangleError::Config(format!("Invalid config: {e}")))?
    } else {
        DocumentMut::new()
    };

    ensure_table(&mut doc, "analyze");
    ensure_nested_table(&mut doc, &["analyze", "architecture"]);

    let architecture = doc["analyze"]["architecture"]
        .as_table_mut()
        .expect("architecture should be a table");

    let has_existing_policy = architecture.contains_key("allowed_dependencies")
        || architecture.contains_key("forbidden_dependencies")
        || architecture.contains_key("exceptions")
        || architecture.contains_key("ignored_components")
        || architecture.contains_key("fail_on_violations")
        || architecture.contains_key("fail_on_cycles");
    if has_existing_policy && !force {
        return Err(UntangleError::Config(format!(
            "Architecture policy already exists in {}; rerun with --force to replace it",
            config_path.display()
        )));
    }

    architecture["level"] = value(policy.level as i64);
    architecture["fail_on_violations"] = value(policy.fail_on_violations);
    architecture["fail_on_cycles"] = value(policy.fail_on_cycles);
    architecture.remove("ignored_components");
    architecture.remove("forbidden_dependencies");
    architecture.remove("exceptions");
    architecture.remove("allowed_dependencies");

    let mut allowed_table = Table::new();
    for (component, dependencies) in &policy.allowed_dependencies {
        let mut array = Array::default();
        for dependency in dependencies {
            array.push(dependency.as_str());
        }
        allowed_table[component] = Item::Value(array.into());
    }
    architecture["allowed_dependencies"] = Item::Table(allowed_table);

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(config_path, doc.to_string())?;
    Ok(())
}

fn collect_projection_data(
    graph: &DepGraph,
    root: &Path,
    level: usize,
    ignored_components: &[String],
) -> ProjectionData {
    let projected = project_architecture(graph, root, level);
    let ignored: HashSet<&str> = ignored_components.iter().map(String::as_str).collect();
    let active_components: BTreeSet<String> = projected
        .nodes
        .iter()
        .filter(|node| !ignored.contains(node.id.as_str()))
        .map(|node| node.id.clone())
        .collect();

    let feedback_edges: BTreeSet<ComponentEdge> = projected
        .feedback_edges
        .iter()
        .filter(|edge| {
            active_components.contains(&edge.from) && active_components.contains(&edge.to)
        })
        .map(|edge| (edge.from.clone(), edge.to.clone()))
        .collect();

    let mut dependency_stats = BTreeMap::new();
    let mut evidence = BTreeMap::new();

    for edge in graph.edge_references() {
        let from_module = graph[edge.source()].name.clone();
        let to_module = graph[edge.target()].name.clone();
        let from = project_component_id(&graph[edge.source()], level);
        let to = project_component_id(&graph[edge.target()], level);
        if from == to
            || !active_components.contains(&from)
            || !active_components.contains(&to)
        {
            continue;
        }

        let key = (from.clone(), to.clone());
        let stats = dependency_stats.entry(key.clone()).or_insert_with(DependencyStats::default);
        stats.count += 1;
        stats.source_location_count += edge.weight().source_locations.len();

        evidence
            .entry(key)
            .or_insert_with(BTreeMap::new)
            .entry((from_module, to_module))
            .or_insert_with(Vec::new)
            .extend(edge.weight().source_locations.clone());
    }

    ProjectionData {
        projected,
        active_components,
        dependency_stats,
        evidence,
        feedback_edges,
    }
}

fn collect_component_metrics(
    data: &ProjectionData,
    feedback_members: &HashSet<String>,
) -> Vec<ArchitectureComponentMetric> {
    let layers = layer_map(&data.projected);
    let fan_in = neighbor_counts(&data.active_components, &data.dependency_stats, true);
    let fan_out = neighbor_counts(&data.active_components, &data.dependency_stats, false);

    let mut metrics = Vec::new();
    for node in &data.projected.nodes {
        if !data.active_components.contains(&node.id) {
            continue;
        }
        let in_count = *fan_in.get(&node.id).unwrap_or(&0);
        let out_count = *fan_out.get(&node.id).unwrap_or(&0);
        let instability = if in_count + out_count == 0 {
            0.0
        } else {
            out_count as f64 / (in_count + out_count) as f64
        };
        metrics.push(ArchitectureComponentMetric {
            id: node.id.clone(),
            layer: *layers.get(&node.id).unwrap_or(&0),
            module_count: node.module_count,
            fan_in: in_count,
            fan_out: out_count,
            instability: (instability * 1000.0).round() / 1000.0,
            feedback: feedback_members.contains(&node.id),
        });
    }
    metrics.sort_by(|a, b| a.id.cmp(&b.id));
    metrics
}

fn neighbor_counts(
    components: &BTreeSet<String>,
    edges: &BTreeMap<ComponentEdge, DependencyStats>,
    incoming: bool,
) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, BTreeSet<String>> = components
        .iter()
        .map(|component| (component.clone(), BTreeSet::new()))
        .collect();

    for edge in edges.keys() {
        if incoming {
            counts
                .entry(edge.1.clone())
                .or_default()
                .insert(edge.0.clone());
        } else {
            counts
                .entry(edge.0.clone())
                .or_default()
                .insert(edge.1.clone());
        }
    }

    counts
        .into_iter()
        .map(|(component, neighbors)| (component, neighbors.len()))
        .collect()
}

fn collect_cycles(
    components: &BTreeSet<String>,
    dependency_stats: &BTreeMap<ComponentEdge, DependencyStats>,
) -> Vec<ArchitectureCycle> {
    let mut graph = DiGraph::<String, ()>::new();
    let mut nodes = BTreeMap::new();
    for component in components {
        let idx = graph.add_node(component.clone());
        nodes.insert(component.clone(), idx);
    }
    for edge in dependency_stats.keys() {
        if let (Some(from), Some(to)) = (nodes.get(&edge.0), nodes.get(&edge.1)) {
            graph.add_edge(*from, *to, ());
        }
    }

    let mut cycles: Vec<ArchitectureCycle> = tarjan_scc(&graph)
        .into_iter()
        .filter(|members| members.len() > 1)
        .map(|members| {
            let mut names: Vec<String> = members.into_iter().map(|idx| graph[idx].clone()).collect();
            names.sort();
            let member_set: HashSet<_> = names.iter().cloned().collect();
            let edge_count = dependency_stats
                .keys()
                .filter(|edge| member_set.contains(&edge.0) && member_set.contains(&edge.1))
                .count();
            ArchitectureCycle {
                size: names.len(),
                edge_count,
                members: names,
            }
        })
        .collect();
    cycles.sort_by(|a, b| a.members.cmp(&b.members));
    cycles
}

fn feedback_members(edges: &BTreeSet<ComponentEdge>) -> HashSet<String> {
    let mut members = HashSet::new();
    for edge in edges {
        members.insert(edge.0.clone());
        members.insert(edge.1.clone());
    }
    members
}

fn evaluate_dependencies(
    data: &ProjectionData,
    config: &ResolvedArchitectureConfig,
) -> (Vec<ArchitectureDependency>, Vec<ArchitectureViolation>, usize) {
    let mut dependencies = Vec::new();
    let mut violations = Vec::new();
    let mut waived_dependency_count = 0;

    for (edge, stats) in &data.dependency_stats {
        let disallowed_by_allowlist =
            !config.allowed_dependencies.is_empty() && !allowlist_permits(config, &edge.0, &edge.1);
        let forbidden = config
            .forbidden_dependencies
            .iter()
            .any(|rule| rule.from == edge.0 && rule.to == edge.1);

        let mut violated = false;
        let mut waived = false;
        if disallowed_by_allowlist || forbidden {
            let evidence = build_violation_evidence(data, edge, &config.exceptions);
            if evidence.is_empty() {
                waived = true;
                waived_dependency_count += 1;
            } else {
                violated = true;
                violations.push(ArchitectureViolation {
                    from: edge.0.clone(),
                    to: edge.1.clone(),
                    kind: if forbidden {
                        ArchitectureViolationKind::ForbiddenRule
                    } else {
                        ArchitectureViolationKind::Allowlist
                    },
                    evidence,
                });
            }
        }

        dependencies.push(ArchitectureDependency {
            from: edge.0.clone(),
            to: edge.1.clone(),
            count: stats.count,
            source_location_count: stats.source_location_count,
            feedback: data.feedback_edges.contains(edge),
            violated,
            waived,
        });
    }

    dependencies.sort_by(|a, b| a.from.cmp(&b.from).then(a.to.cmp(&b.to)));
    violations.sort_by(|a, b| a.from.cmp(&b.from).then(a.to.cmp(&b.to)));
    (dependencies, violations, waived_dependency_count)
}

fn allowlist_permits(config: &ResolvedArchitectureConfig, from: &str, to: &str) -> bool {
    config
        .allowed_dependencies
        .get(from)
        .map(|deps| deps.iter().any(|dependency| dependency == "*" || dependency == to))
        .unwrap_or(false)
}

fn build_violation_evidence(
    data: &ProjectionData,
    edge: &ComponentEdge,
    exceptions: &[ArchitectureException],
) -> Vec<ArchitectureViolationEvidence> {
    let Some(module_edges) = data.evidence.get(edge) else {
        return Vec::new();
    };

    let mut evidence = Vec::new();
    for ((from_module, to_module), locations) in module_edges {
        if exceptions
            .iter()
            .any(|exception| exception_matches(exception, edge, from_module, to_module))
        {
            continue;
        }
        evidence.push(ArchitectureViolationEvidence {
            from_module: from_module.clone(),
            to_module: to_module.clone(),
            source_locations: locations.clone(),
        });
    }
    evidence.sort_by(|a, b| a.from_module.cmp(&b.from_module).then(a.to_module.cmp(&b.to_module)));
    evidence
}

fn exception_matches(
    exception: &ArchitectureException,
    edge: &ComponentEdge,
    from_module: &str,
    to_module: &str,
) -> bool {
    if let Some(ref from_component) = exception.from_component {
        if from_component != &edge.0 {
            return false;
        }
    }
    if let Some(ref to_component) = exception.to_component {
        if to_component != &edge.1 {
            return false;
        }
    }
    if let Some(ref expected_from) = exception.from_module {
        if expected_from != from_module {
            return false;
        }
    }
    if let Some(ref expected_to) = exception.to_module {
        if expected_to != to_module {
            return false;
        }
    }
    true
}

fn ensure_table(doc: &mut DocumentMut, key: &str) {
    if !doc.as_table().contains_key(key) {
        doc[key] = Item::Table(Table::new());
    }
}

fn ensure_nested_table(doc: &mut DocumentMut, path: &[&str]) {
    if path.is_empty() {
        return;
    }
    ensure_table(doc, path[0]);
    let mut item = doc
        .as_item_mut()
        .get_mut(path[0])
        .expect("top-level table should exist");
    for segment in &path[1..] {
        if !item.as_table().is_some_and(|table| table.contains_key(segment)) {
            item[*segment] = Item::Table(Table::new());
        }
        item = &mut item[*segment];
    }
}

#[allow(dead_code)]
fn _empty_array_of_tables() -> ArrayOfTables {
    ArrayOfTables::new()
}
