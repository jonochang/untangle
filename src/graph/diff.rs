use crate::architecture::policy::{
    self, ArchitectureCheckResult, ArchitectureCycle, ArchitectureViolation,
};
use crate::config::ResolvedArchitectureConfig;
use crate::errors::Result;
use crate::graph::builder::{GraphBuilder, ResolvedImport};
use crate::graph::ir::DepGraph;
use crate::metrics::scc::find_non_trivial_sccs;
use crate::metrics::summary::Summary;
use crate::parse::common::{ImportConfidence, SourceLocation};
use crate::parse::go::GoFrontend;
use crate::parse::rust::RustFrontend;
use crate::parse::ParseFrontend;
use crate::walk::Language;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, Serialize)]
pub struct DiffResult {
    pub base_ref: String,
    pub head_ref: String,
    pub verdict: Verdict,
    pub comparison: Comparison,
    pub reasons: Vec<String>,
    pub elapsed_ms: u64,
    pub modules_per_second: f64,
    pub summary_delta: SummaryDelta,
    pub new_edges: Vec<EdgeChange>,
    pub removed_edges: Vec<EdgeChange>,
    pub fanout_changes: Vec<FanoutChange>,
    pub scc_changes: SccChanges,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture_policy_delta: Option<ArchitecturePolicyDelta>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
pub struct Comparison {
    pub verdict: ComparisonVerdict,
    pub summary: String,
    pub recommendation: String,
    pub drivers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonVerdict {
    Improved,
    Worse,
    Mixed,
    Unchanged,
}

#[derive(Debug, Clone, Serialize)]
pub struct SummaryDelta {
    pub nodes_added: usize,
    pub nodes_removed: usize,
    pub edges_added: usize,
    pub edges_removed: usize,
    pub net_edge_change: isize,
    pub scc_count_delta: isize,
    pub largest_scc_size_delta: isize,
    pub mean_fanout_delta: f64,
    pub mean_entropy_delta: f64,
    pub max_depth_delta: isize,
    pub total_complexity_delta: isize,
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgeChange {
    pub from: String,
    pub to: String,
    pub source_locations: Vec<SourceLocation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FanoutChange {
    pub node: String,
    pub fanout_before: usize,
    pub fanout_after: usize,
    pub delta: isize,
    pub entropy_before: f64,
    pub entropy_after: f64,
    pub new_targets: Vec<EdgeChange>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SccChanges {
    pub new_sccs: Vec<SccChange>,
    pub enlarged_sccs: Vec<SccChange>,
    pub resolved_sccs: Vec<SccChange>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SccChange {
    pub members: Vec<String>,
    pub size: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitecturePolicyDelta {
    pub new_violations: Vec<ArchitectureViolation>,
    pub new_cycles: Vec<ArchitectureCycle>,
    pub enlarged_cycles: Vec<ArchitectureCycle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailCondition {
    FanoutIncrease,
    FanoutThreshold(usize),
    NewScc,
    SccGrowth,
    EntropyIncrease,
    NewEdge,
    NewArchitectureViolation,
    NewArchitectureCycle,
    ArchitectureCycleGrowth,
}

impl FailCondition {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "fanout-increase" => Some(Self::FanoutIncrease),
            "new-scc" => Some(Self::NewScc),
            "scc-growth" => Some(Self::SccGrowth),
            "entropy-increase" => Some(Self::EntropyIncrease),
            "new-edge" => Some(Self::NewEdge),
            "new-architecture-violation" => Some(Self::NewArchitectureViolation),
            "new-architecture-cycle" => Some(Self::NewArchitectureCycle),
            "architecture-cycle-growth" => Some(Self::ArchitectureCycleGrowth),
            value if value.starts_with("fanout-threshold") => value
                .split('=')
                .nth(1)
                .and_then(|n| n.parse().ok())
                .map(Self::FanoutThreshold),
            _ => None,
        }
    }
}

pub struct DiffAnalysisRequest<'a> {
    pub repo: &'a git2::Repository,
    pub root: &'a Path,
    pub base_ref: &'a str,
    pub head_ref: &'a str,
    pub langs: &'a [Language],
    pub include: &'a [String],
    pub exclude: &'a [String],
    pub include_tests: bool,
    pub go_exclude_stdlib: bool,
    pub ruby_load_paths: &'a [PathBuf],
    pub ruby_zeitwerk: bool,
    pub conditions: &'a [FailCondition],
    pub architecture_config: Option<&'a ResolvedArchitectureConfig>,
}

pub fn analyze_repo_diff(request: DiffAnalysisRequest<'_>) -> Result<DiffResult> {
    let start = Instant::now();
    let base_graph = build_graph_at_ref(
        request.repo,
        request.base_ref,
        request.root,
        request.langs,
        request.include,
        request.exclude,
        request.include_tests,
        request.go_exclude_stdlib,
        request.ruby_load_paths,
        request.ruby_zeitwerk,
    )?;
    let head_graph = build_graph_at_ref(
        request.repo,
        request.head_ref,
        request.root,
        request.langs,
        request.include,
        request.exclude,
        request.include_tests,
        request.go_exclude_stdlib,
        request.ruby_load_paths,
        request.ruby_zeitwerk,
    )?;

    let diff = compute_raw_diff(&base_graph, &head_graph);
    let architecture_policy_delta = request
        .architecture_config
        .map(|config| compute_architecture_policy_delta(&base_graph, &head_graph, request.root, config));
    let (verdict, reasons) =
        evaluate_policies(&diff, architecture_policy_delta.as_ref(), request.conditions);
    let elapsed_ms = start.elapsed().as_millis() as u64;
    let total_nodes = base_graph.node_count() + head_graph.node_count();
    let modules_per_second = if elapsed_ms > 0 {
        total_nodes as f64 / (elapsed_ms as f64 / 1000.0)
    } else {
        0.0
    };

    Ok(DiffResult {
        base_ref: request.base_ref.to_string(),
        head_ref: request.head_ref.to_string(),
        verdict,
        comparison: compare_diff(&diff, architecture_policy_delta.as_ref()),
        reasons,
        elapsed_ms,
        modules_per_second: (modules_per_second * 10.0).round() / 10.0,
        summary_delta: diff.summary_delta,
        new_edges: diff.new_edges,
        removed_edges: diff.removed_edges,
        fanout_changes: diff.fanout_changes,
        scc_changes: diff.scc_changes,
        architecture_policy_delta,
    })
}

fn compare_diff(diff: &RawDiff, architecture_policy_delta: Option<&ArchitecturePolicyDelta>) -> Comparison {
    let mut improvements = Vec::new();
    let mut regressions = Vec::new();

    if diff.summary_delta.net_edge_change < 0 {
        improvements.push(format!(
            "net edge count decreased by {}",
            -diff.summary_delta.net_edge_change
        ));
    } else if diff.summary_delta.net_edge_change > 0 {
        regressions.push(format!(
            "net edge count increased by {}",
            diff.summary_delta.net_edge_change
        ));
    }

    if diff.summary_delta.scc_count_delta < 0 {
        improvements.push(format!(
            "scc count dropped by {}",
            -diff.summary_delta.scc_count_delta
        ));
    } else if diff.summary_delta.scc_count_delta > 0 {
        regressions.push(format!(
            "scc count increased by {}",
            diff.summary_delta.scc_count_delta
        ));
    }

    if diff.summary_delta.mean_fanout_delta < -0.05 {
        improvements.push(format!(
            "mean fan-out decreased by {:.2}",
            -diff.summary_delta.mean_fanout_delta
        ));
    } else if diff.summary_delta.mean_fanout_delta > 0.05 {
        regressions.push(format!(
            "mean fan-out increased by {:.2}",
            diff.summary_delta.mean_fanout_delta
        ));
    }

    if diff.summary_delta.total_complexity_delta < 0 {
        improvements.push(format!(
            "total complexity dropped by {}",
            -diff.summary_delta.total_complexity_delta
        ));
    } else if diff.summary_delta.total_complexity_delta > 0 {
        regressions.push(format!(
            "total complexity increased by {}",
            diff.summary_delta.total_complexity_delta
        ));
    }

    if let Some(delta) = architecture_policy_delta {
        if !delta.new_violations.is_empty() {
            regressions.push(format!(
                "{} new architecture violation(s)",
                delta.new_violations.len()
            ));
        }
        if !delta.new_cycles.is_empty() {
            regressions.push(format!(
                "{} new architecture cycle(s)",
                delta.new_cycles.len()
            ));
        }
        if !delta.enlarged_cycles.is_empty() {
            regressions.push(format!(
                "{} enlarged architecture cycle(s)",
                delta.enlarged_cycles.len()
            ));
        }
    }

    let verdict = if improvements.is_empty() && regressions.is_empty() {
        ComparisonVerdict::Unchanged
    } else if improvements.is_empty() {
        ComparisonVerdict::Worse
    } else if regressions.is_empty() {
        ComparisonVerdict::Improved
    } else {
        ComparisonVerdict::Mixed
    };

    let summary = match verdict {
        ComparisonVerdict::Unchanged => {
            "No material structural or architecture change was detected.".to_string()
        }
        ComparisonVerdict::Improved => {
            format!("Change appears improved: {}.", improvements.join(", "))
        }
        ComparisonVerdict::Worse => {
            format!("Change appears worse: {}.", regressions.join(", "))
        }
        ComparisonVerdict::Mixed => format!(
            "Change is mixed: improved in {}, worse in {}.",
            improvements.join(", "),
            regressions.join(", ")
        ),
    };

    let recommendation = if !regressions.is_empty() {
        if regressions.iter().any(|item| item.contains("architecture")) {
            "Review boundary changes first; architecture regressions tend to create lasting drag."
                .to_string()
        } else if regressions.iter().any(|item| item.contains("scc")) {
            "Break new cycles before adding more dependencies on top of them.".to_string()
        } else {
            "Review the added coupling before treating this change as complete.".to_string()
        }
    } else if !improvements.is_empty() {
        "The change reduced structural pressure; keep the same direction for adjacent cleanup."
            .to_string()
    } else {
        "No follow-up is needed from the structural diff alone.".to_string()
    };

    let mut drivers = Vec::new();
    drivers.extend(improvements.into_iter().take(2));
    drivers.extend(regressions.into_iter().take(3));

    Comparison {
        verdict,
        summary,
        recommendation,
        drivers,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_graph_at_ref(
    repo: &git2::Repository,
    reference: &str,
    root: &Path,
    langs: &[Language],
    include: &[String],
    exclude: &[String],
    include_tests: bool,
    go_exclude_stdlib: bool,
    ruby_load_paths: &[PathBuf],
    zeitwerk: bool,
) -> Result<DepGraph> {
    let extensions: Vec<&str> = langs
        .iter()
        .flat_map(|lang| lang.extensions().iter().copied())
        .collect();
    let all_files = crate::git::list_files_at_ref(repo, reference, &extensions)?;

    let exclude_set = if !exclude.is_empty() {
        let mut builder = globset::GlobSetBuilder::new();
        for pattern in exclude {
            if let Ok(glob) = globset::Glob::new(pattern) {
                builder.add(glob);
            }
        }
        builder.build().ok()
    } else {
        None
    };
    let include_set = if !include.is_empty() {
        let mut builder = globset::GlobSetBuilder::new();
        for pattern in include {
            if let Ok(glob) = globset::Glob::new(pattern) {
                builder.add(glob);
            }
        }
        builder.build().ok()
    } else {
        None
    };

    let mut files_by_lang: HashMap<Language, Vec<PathBuf>> = HashMap::new();
    for file in all_files {
        let lang = match crate::walk::language_for_file(&file) {
            Some(lang) if langs.contains(&lang) => lang,
            _ => continue,
        };

        let path_str = file.to_string_lossy();
        if !include_tests && lang == Language::Go && path_str.ends_with("_test.go") {
            continue;
        }
        if let Some(ref set) = exclude_set {
            if set.is_match(&file) {
                continue;
            }
        }
        if let Some(ref set) = include_set {
            if !set.is_match(&file) {
                continue;
            }
        }

        files_by_lang.entry(lang).or_default().push(file);
    }

    let go_module_map: HashMap<PathBuf, String> = if langs.contains(&Language::Go) {
        crate::git::find_files_by_name_at_ref(repo, reference, "go.mod")
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(path, content)| {
                let dir = path.parent()?.to_path_buf();
                let source = String::from_utf8(content).ok()?;
                let module_path = crate::parse::go::parse_go_mod_module(&source)?;
                Some((dir, module_path))
            })
            .collect()
    } else {
        HashMap::new()
    };

    let root_go_module = go_module_map
        .get(Path::new(""))
        .or_else(|| go_module_map.get(Path::new(".")))
        .cloned()
        .or_else(|| {
            crate::git::read_file_at_ref(repo, reference, Path::new("go.mod"))
                .ok()
                .and_then(|content| {
                    String::from_utf8(content)
                        .ok()
                        .and_then(|source| crate::parse::go::parse_go_mod_module(&source))
                })
        });

    let go_resolvers: HashMap<PathBuf, Box<dyn ParseFrontend>> = go_module_map
        .iter()
        .map(|(mod_root, mod_path)| {
            let frontend = GoFrontend::with_module_path(mod_path.clone())
                .with_exclude_stdlib(go_exclude_stdlib);
            (
                mod_root.clone(),
                Box::new(frontend) as Box<dyn ParseFrontend>,
            )
        })
        .collect();

    let go_files_by_module: HashMap<PathBuf, Vec<PathBuf>> = {
        let mut by_module: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        if let Some(go_files) = files_by_lang.get(&Language::Go) {
            for file in go_files {
                let mod_root = crate::walk::find_go_module_root(file, &go_module_map)
                    .map(|(root, _)| root.to_path_buf())
                    .unwrap_or_default();
                by_module
                    .entry(mod_root)
                    .or_default()
                    .push(file.to_path_buf());
            }
        }
        by_module
    };

    let mut frontends: HashMap<Language, Box<dyn ParseFrontend>> = HashMap::new();
    for &lang in langs {
        if lang == Language::Go {
            let frontend = match &root_go_module {
                Some(module_path) => GoFrontend::with_module_path(module_path.clone()),
                None => GoFrontend::new(),
            };
            frontends.insert(
                lang,
                Box::new(frontend.with_exclude_stdlib(go_exclude_stdlib)),
            );
            continue;
        }

        let frontend: Box<dyn ParseFrontend> = match lang {
            Language::Python => Box::new(crate::parse::python::PythonFrontend::new()),
            Language::Ruby => Box::new(
                crate::parse::ruby::RubyFrontend::with_load_paths(ruby_load_paths.to_vec())
                    .with_zeitwerk(zeitwerk),
            ),
            Language::Rust => {
                let cargo_toml =
                    crate::git::read_file_at_ref(repo, reference, Path::new("Cargo.toml")).ok();
                let crate_name = cargo_toml.and_then(|content| {
                    String::from_utf8(content)
                        .ok()
                        .and_then(|source| RustFrontend::parse_crate_name(&source))
                });
                Box::new(match crate_name {
                    Some(name) => RustFrontend::with_crate_name(name),
                    None => RustFrontend::new(),
                })
            }
            Language::Go => unreachable!(),
        };
        frontends.insert(lang, frontend);
    }

    let mut builder = GraphBuilder::new();

    for (&lang, files) in &files_by_lang {
        for file_path in files {
            let source = match crate::git::read_file_at_ref(repo, reference, file_path) {
                Ok(source) => source,
                Err(_) => continue,
            };

            let (frontend, resolve_files): (&dyn ParseFrontend, &[PathBuf]) =
                if lang == Language::Go {
                    let mod_root = crate::walk::find_go_module_root(file_path, &go_module_map)
                        .map(|(root, _)| root.to_path_buf())
                        .unwrap_or_default();
                    let resolver = go_resolvers
                        .get(&mod_root)
                        .map(|resolver| resolver.as_ref())
                        .unwrap_or_else(|| frontends.get(&Language::Go).unwrap().as_ref());
                    let mod_files = go_files_by_module
                        .get(&mod_root)
                        .map(|files| files.as_slice())
                        .unwrap_or(&[]);
                    (resolver, mod_files)
                } else {
                    let frontend = frontends.get(&lang).unwrap().as_ref();
                    (frontend, files.as_slice())
                };

            let imports = frontend.extract_imports(&source, file_path);

            for raw in &imports {
                if matches!(
                    raw.confidence,
                    ImportConfidence::External
                        | ImportConfidence::Dynamic
                        | ImportConfidence::Unresolvable
                ) {
                    continue;
                }

                if let Some(target) = frontend.resolve(raw, root, resolve_files) {
                    let source_module = if lang == Language::Go {
                        file_path.parent().unwrap_or(file_path).to_path_buf()
                    } else {
                        file_path.to_path_buf()
                    };
                    builder.add_import(&ResolvedImport {
                        source_module,
                        target_module: target,
                        location: SourceLocation {
                            file: file_path.to_path_buf(),
                            line: raw.line,
                            column: raw.column,
                        },
                        language: Some(lang),
                    });
                }
            }
        }
    }

    Ok(builder.build())
}

struct RawDiff {
    summary_delta: SummaryDelta,
    new_edges: Vec<EdgeChange>,
    removed_edges: Vec<EdgeChange>,
    fanout_changes: Vec<FanoutChange>,
    scc_changes: SccChanges,
    head_node_fanouts: Vec<(String, usize)>,
}

fn compute_raw_diff(base: &DepGraph, head: &DepGraph) -> RawDiff {
    let base_nodes: HashSet<String> = base
        .node_indices()
        .map(|idx| base[idx].name.clone())
        .collect();
    let head_nodes: HashSet<String> = head
        .node_indices()
        .map(|idx| head[idx].name.clone())
        .collect();

    let nodes_added = head_nodes.difference(&base_nodes).count();
    let nodes_removed = base_nodes.difference(&head_nodes).count();

    let base_edges: HashSet<(String, String)> = base
        .edge_indices()
        .map(|edge| {
            let (source, target) = base.edge_endpoints(edge).unwrap();
            (base[source].name.clone(), base[target].name.clone())
        })
        .collect();
    let head_edges: HashSet<(String, String)> = head
        .edge_indices()
        .map(|edge| {
            let (source, target) = head.edge_endpoints(edge).unwrap();
            (head[source].name.clone(), head[target].name.clone())
        })
        .collect();

    let new_edge_set: Vec<_> = head_edges.difference(&base_edges).cloned().collect();
    let removed_edge_set: Vec<_> = base_edges.difference(&head_edges).cloned().collect();
    let edges_added = new_edge_set.len();
    let edges_removed = removed_edge_set.len();

    let head_node_map: HashMap<String, petgraph::graph::NodeIndex> = head
        .node_indices()
        .map(|idx| (head[idx].name.clone(), idx))
        .collect();
    let base_node_map: HashMap<String, petgraph::graph::NodeIndex> = base
        .node_indices()
        .map(|idx| (base[idx].name.clone(), idx))
        .collect();

    let new_edges = new_edge_set
        .iter()
        .map(|(from, to)| {
            let source_locations = if let (Some(&from_idx), Some(&to_idx)) =
                (head_node_map.get(from), head_node_map.get(to))
            {
                head.find_edge(from_idx, to_idx)
                    .map(|edge| head[edge].source_locations.clone())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            EdgeChange {
                from: from.clone(),
                to: to.clone(),
                source_locations,
            }
        })
        .collect();

    let removed_edges = removed_edge_set
        .iter()
        .map(|(from, to)| EdgeChange {
            from: from.clone(),
            to: to.clone(),
            source_locations: Vec::new(),
        })
        .collect();

    let mut fanout_changes = Vec::new();
    for name in base_nodes.intersection(&head_nodes) {
        if let (Some(&base_idx), Some(&head_idx)) =
            (base_node_map.get(name), head_node_map.get(name))
        {
            let fanout_before = crate::metrics::fanout::fan_out(base, base_idx);
            let fanout_after = crate::metrics::fanout::fan_out(head, head_idx);

            if fanout_before != fanout_after {
                let base_weights: Vec<usize> = base
                    .edges_directed(base_idx, Direction::Outgoing)
                    .map(|edge| edge.weight().weight)
                    .collect();
                let head_weights: Vec<usize> = head
                    .edges_directed(head_idx, Direction::Outgoing)
                    .map(|edge| edge.weight().weight)
                    .collect();

                let entropy_before = crate::metrics::entropy::shannon_entropy(&base_weights);
                let entropy_after = crate::metrics::entropy::shannon_entropy(&head_weights);

                let base_targets: HashSet<String> = base
                    .edges_directed(base_idx, Direction::Outgoing)
                    .map(|edge| base[edge.target()].name.clone())
                    .collect();
                let head_targets: HashSet<String> = head
                    .edges_directed(head_idx, Direction::Outgoing)
                    .map(|edge| head[edge.target()].name.clone())
                    .collect();

                let new_targets = head_targets
                    .difference(&base_targets)
                    .map(|target| {
                        let source_locations = head_node_map
                            .get(target)
                            .and_then(|&target_idx| {
                                head.find_edge(head_idx, target_idx)
                                    .map(|edge| head[edge].source_locations.clone())
                            })
                            .unwrap_or_default();
                        EdgeChange {
                            from: name.clone(),
                            to: target.clone(),
                            source_locations,
                        }
                    })
                    .collect();

                fanout_changes.push(FanoutChange {
                    node: name.clone(),
                    fanout_before,
                    fanout_after,
                    delta: fanout_after as isize - fanout_before as isize,
                    entropy_before: (entropy_before * 100.0).round() / 100.0,
                    entropy_after: (entropy_after * 100.0).round() / 100.0,
                    new_targets,
                });
            }
        }
    }

    let base_sccs = find_non_trivial_sccs(base);
    let head_sccs = find_non_trivial_sccs(head);
    let base_summary = Summary::from_graph(base);
    let head_summary = Summary::from_graph(head);

    let base_mean_entropy = mean_entropy(base);
    let head_mean_entropy = mean_entropy(head);
    let head_node_fanouts = head
        .node_indices()
        .map(|idx| {
            (
                head[idx].name.clone(),
                crate::metrics::fanout::fan_out(head, idx),
            )
        })
        .collect();

    let mut matched_base: HashSet<usize> = HashSet::new();
    let mut matched_head: HashSet<usize> = HashSet::new();
    let mut enlarged_sccs = Vec::new();

    for (head_idx, head_scc) in head_sccs.iter().enumerate() {
        let head_members: HashSet<&String> = head_scc.members.iter().collect();
        let mut best_match = None;
        let mut best_jaccard = 0.0f64;

        for (base_idx, base_scc) in base_sccs.iter().enumerate() {
            if matched_base.contains(&base_idx) {
                continue;
            }

            let base_members: HashSet<&String> = base_scc.members.iter().collect();
            let intersection = head_members.intersection(&base_members).count() as f64;
            let union = head_members.union(&base_members).count() as f64;
            let jaccard = if union > 0.0 {
                intersection / union
            } else {
                0.0
            };

            if jaccard > best_jaccard {
                best_jaccard = jaccard;
                best_match = Some(base_idx);
            }
        }

        if best_jaccard > 0.5 {
            if let Some(base_idx) = best_match {
                matched_base.insert(base_idx);
                matched_head.insert(head_idx);
                if head_scc.size > base_sccs[base_idx].size {
                    enlarged_sccs.push(SccChange {
                        members: head_scc.members.clone(),
                        size: head_scc.size,
                    });
                }
            }
        }
    }

    let new_sccs = head_sccs
        .iter()
        .enumerate()
        .filter(|(idx, _)| !matched_head.contains(idx))
        .map(|(_, scc)| SccChange {
            members: scc.members.clone(),
            size: scc.size,
        })
        .collect();
    let resolved_sccs = base_sccs
        .iter()
        .enumerate()
        .filter(|(idx, _)| !matched_base.contains(idx))
        .map(|(_, scc)| SccChange {
            members: scc.members.clone(),
            size: scc.size,
        })
        .collect();

    RawDiff {
        summary_delta: SummaryDelta {
            nodes_added,
            nodes_removed,
            edges_added,
            edges_removed,
            net_edge_change: edges_added as isize - edges_removed as isize,
            scc_count_delta: head_sccs.len() as isize - base_sccs.len() as isize,
            largest_scc_size_delta: head_summary.largest_scc_size as isize
                - base_summary.largest_scc_size as isize,
            mean_fanout_delta: ((head_summary.mean_fanout - base_summary.mean_fanout) * 100.0)
                .round()
                / 100.0,
            mean_entropy_delta: ((head_mean_entropy - base_mean_entropy) * 100.0).round() / 100.0,
            max_depth_delta: head_summary.max_depth as isize - base_summary.max_depth as isize,
            total_complexity_delta: head_summary.total_complexity as isize
                - base_summary.total_complexity as isize,
        },
        new_edges,
        removed_edges,
        fanout_changes,
        scc_changes: SccChanges {
            new_sccs,
            enlarged_sccs,
            resolved_sccs,
        },
        head_node_fanouts,
    }
}

fn compute_architecture_policy_delta(
    base_graph: &DepGraph,
    head_graph: &DepGraph,
    root: &Path,
    config: &ResolvedArchitectureConfig,
) -> ArchitecturePolicyDelta {
    let base = policy::check_graph(base_graph, root, config, Some(config.level));
    let head = policy::check_graph(head_graph, root, config, Some(config.level));

    let base_violations: HashSet<(String, String)> = base
        .violations
        .iter()
        .map(|violation| (violation.from.clone(), violation.to.clone()))
        .collect();
    let new_violations = head
        .violations
        .iter()
        .filter(|violation| {
            !base_violations.contains(&(violation.from.clone(), violation.to.clone()))
        })
        .cloned()
        .collect();

    let (new_cycles, enlarged_cycles) = diff_cycles(&base, &head);

    ArchitecturePolicyDelta {
        new_violations,
        new_cycles,
        enlarged_cycles,
    }
}

fn diff_cycles(
    base: &ArchitectureCheckResult,
    head: &ArchitectureCheckResult,
) -> (Vec<ArchitectureCycle>, Vec<ArchitectureCycle>) {
    let mut matched_base: HashSet<usize> = HashSet::new();
    let mut matched_head: HashSet<usize> = HashSet::new();
    let mut enlarged_cycles = Vec::new();

    for (head_idx, head_cycle) in head.cycles.iter().enumerate() {
        let head_members: HashSet<&String> = head_cycle.members.iter().collect();
        let mut best_match = None;
        let mut best_jaccard = 0.0f64;

        for (base_idx, base_cycle) in base.cycles.iter().enumerate() {
            if matched_base.contains(&base_idx) {
                continue;
            }

            let base_members: HashSet<&String> = base_cycle.members.iter().collect();
            let intersection = head_members.intersection(&base_members).count() as f64;
            let union = head_members.union(&base_members).count() as f64;
            let jaccard = if union > 0.0 {
                intersection / union
            } else {
                0.0
            };

            if jaccard > best_jaccard {
                best_jaccard = jaccard;
                best_match = Some(base_idx);
            }
        }

        if best_jaccard > 0.5 {
            if let Some(base_idx) = best_match {
                matched_base.insert(base_idx);
                matched_head.insert(head_idx);
                if head_cycle.size > base.cycles[base_idx].size {
                    enlarged_cycles.push(head_cycle.clone());
                }
            }
        }
    }

    let new_cycles = head
        .cycles
        .iter()
        .enumerate()
        .filter(|(idx, _)| !matched_head.contains(idx))
        .map(|(_, cycle)| cycle.clone())
        .collect();

    (new_cycles, enlarged_cycles)
}

fn evaluate_policies(
    diff: &RawDiff,
    architecture_policy_delta: Option<&ArchitecturePolicyDelta>,
    conditions: &[FailCondition],
) -> (Verdict, Vec<String>) {
    let mut reasons = Vec::new();

    for condition in conditions {
        match condition {
            FailCondition::FanoutIncrease => {
                if diff.fanout_changes.iter().any(|change| change.delta > 0) {
                    reasons.push("fanout-increase".to_string());
                }
            }
            FailCondition::FanoutThreshold(threshold) => {
                if diff
                    .head_node_fanouts
                    .iter()
                    .any(|(_, fanout)| *fanout > *threshold)
                {
                    reasons.push(format!("fanout-threshold={threshold}"));
                }
            }
            FailCondition::NewScc => {
                if !diff.scc_changes.new_sccs.is_empty() {
                    reasons.push("new-scc".to_string());
                }
            }
            FailCondition::SccGrowth => {
                if !diff.scc_changes.enlarged_sccs.is_empty() {
                    reasons.push("scc-growth".to_string());
                }
            }
            FailCondition::EntropyIncrease => {
                if diff.summary_delta.mean_entropy_delta > 0.0 {
                    reasons.push("entropy-increase".to_string());
                }
            }
            FailCondition::NewEdge => {
                if !diff.new_edges.is_empty() {
                    reasons.push("new-edge".to_string());
                }
            }
            FailCondition::NewArchitectureViolation => {
                if architecture_policy_delta
                    .is_some_and(|delta| !delta.new_violations.is_empty())
                {
                    reasons.push("new-architecture-violation".to_string());
                }
            }
            FailCondition::NewArchitectureCycle => {
                if architecture_policy_delta.is_some_and(|delta| !delta.new_cycles.is_empty()) {
                    reasons.push("new-architecture-cycle".to_string());
                }
            }
            FailCondition::ArchitectureCycleGrowth => {
                if architecture_policy_delta
                    .is_some_and(|delta| !delta.enlarged_cycles.is_empty())
                {
                    reasons.push("architecture-cycle-growth".to_string());
                }
            }
        }
    }

    if reasons.is_empty() {
        (Verdict::Pass, reasons)
    } else {
        (Verdict::Fail, reasons)
    }
}

fn mean_entropy(graph: &DepGraph) -> f64 {
    let entropies: Vec<f64> = graph
        .node_indices()
        .map(|idx| {
            let weights: Vec<usize> = graph
                .edges_directed(idx, Direction::Outgoing)
                .map(|edge| edge.weight().weight)
                .collect();
            crate::metrics::entropy::shannon_entropy(&weights)
        })
        .collect();

    if entropies.is_empty() {
        0.0
    } else {
        entropies.iter().sum::<f64>() / entropies.len() as f64
    }
}
