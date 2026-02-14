use crate::config::resolve::{resolve_config, CliOverrides};
use crate::errors::{Result, UntangleError};
use crate::graph::builder::{GraphBuilder, ResolvedImport};
use crate::graph::diff::{
    DiffResult, EdgeChange, FanoutChange, SccChange, SccChanges, SummaryDelta, Verdict,
};
use crate::graph::ir::DepGraph;
use crate::metrics::scc::find_non_trivial_sccs;
use crate::metrics::summary::Summary;
use crate::output::OutputFormat;
use crate::parse::common::{ImportConfidence, SourceLocation};
use crate::parse::{
    go::GoFrontend, python::PythonFrontend, ruby::RubyFrontend, rust::RustFrontend, ParseFrontend,
};
use crate::walk::Language;
use clap::Args;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Args)]
pub struct DiffArgs {
    /// Path to analyze (defaults to current directory)
    pub path: Option<PathBuf>,

    /// Base git ref
    #[arg(long)]
    pub base: String,

    /// Head git ref
    #[arg(long)]
    pub head: String,

    /// Language to analyze
    #[arg(long, value_parser = parse_language)]
    pub lang: Option<Language>,

    /// Output format
    #[arg(long)]
    pub format: Option<OutputFormat>,

    /// Fail-on conditions (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub fail_on: Vec<String>,

    /// Include test files
    #[arg(long)]
    pub include_tests: bool,

    /// Suppress progress output
    #[arg(long)]
    pub quiet: bool,
}

impl DiffArgs {
    fn to_cli_overrides(&self) -> CliOverrides {
        CliOverrides {
            lang: self.lang,
            format: self.format,
            quiet: self.quiet,
            include_tests: self.include_tests,
            fail_on: self.fail_on.clone(),
            ..Default::default()
        }
    }
}

fn parse_language(s: &str) -> std::result::Result<Language, String> {
    s.parse()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailCondition {
    FanoutIncrease,
    FanoutThreshold(usize),
    NewScc,
    SccGrowth,
    EntropyIncrease,
    NewEdge,
}

impl FailCondition {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "fanout-increase" => Some(Self::FanoutIncrease),
            "new-scc" => Some(Self::NewScc),
            "scc-growth" => Some(Self::SccGrowth),
            "entropy-increase" => Some(Self::EntropyIncrease),
            "new-edge" => Some(Self::NewEdge),
            s if s.starts_with("fanout-threshold") => {
                // fanout-threshold=N or fanout-threshold
                s.split('=')
                    .nth(1)
                    .and_then(|n| n.parse().ok())
                    .map(Self::FanoutThreshold)
            }
            _ => None,
        }
    }
}

pub fn run(args: &DiffArgs) -> Result<()> {
    let start = Instant::now();

    let path = args.path.clone().unwrap_or_else(|| PathBuf::from("."));
    let root = path
        .canonicalize()
        .map_err(|_| UntangleError::NoFiles { path: path.clone() })?;

    // Resolve config
    let overrides = args.to_cli_overrides();
    let config = resolve_config(&root, &overrides)?;

    let repo = crate::git::open_repo(&root)?;

    let lang = match config.lang {
        Some(l) => l,
        None => crate::walk::detect_language(&root)
            .ok_or_else(|| UntangleError::NoFiles { path: root.clone() })?,
    };

    // Build graph at base ref
    let base_graph = build_graph_at_ref(&repo, &args.base, &root, lang)?;
    // Build graph at head ref
    let head_graph = build_graph_at_ref(&repo, &args.head, &root, lang)?;

    // Compute diff
    let diff = compute_graph_diff(&base_graph, &head_graph, &args.base, &args.head);

    // Parse fail-on conditions from resolved config
    let conditions: Vec<FailCondition> = config
        .fail_on
        .iter()
        .filter_map(|s| FailCondition::parse(s))
        .collect();

    // Evaluate policies
    let (verdict, reasons) = evaluate_policies(&diff, &conditions);

    let elapsed = start.elapsed();
    let elapsed_ms = elapsed.as_millis() as u64;
    let total_nodes = base_graph.node_count() + head_graph.node_count();
    let modules_per_second = if elapsed_ms > 0 {
        total_nodes as f64 / (elapsed_ms as f64 / 1000.0)
    } else {
        0.0
    };

    let result = DiffResult {
        base_ref: args.base.clone(),
        head_ref: args.head.clone(),
        verdict,
        reasons,
        elapsed_ms,
        modules_per_second: (modules_per_second * 10.0).round() / 10.0,
        summary_delta: diff.summary_delta,
        new_edges: diff.new_edges,
        removed_edges: diff.removed_edges,
        fanout_changes: diff.fanout_changes,
        scc_changes: diff.scc_changes,
    };

    let format: OutputFormat = config.format.parse().unwrap_or_default();

    let mut stdout = std::io::stdout();
    match format {
        OutputFormat::Json => {
            crate::output::json::write_diff_json(&mut stdout, &result)?;
        }
        OutputFormat::Sarif => {
            // For diff, write as JSON for now
            crate::output::json::write_diff_json(&mut stdout, &result)?;
        }
        _ => {
            crate::output::json::write_diff_json(&mut stdout, &result)?;
        }
    }

    // Exit code based on verdict
    if result.verdict == Verdict::Fail {
        std::process::exit(1);
    }

    Ok(())
}

fn build_graph_at_ref(
    repo: &git2::Repository,
    reference: &str,
    root: &Path,
    lang: Language,
) -> Result<DepGraph> {
    let extensions = lang.extensions();
    let files = crate::git::list_files_at_ref(repo, reference, extensions)?;

    let frontend: Box<dyn ParseFrontend> = match lang {
        Language::Go => {
            // Try to read go.mod at this ref
            let go_mod = crate::git::read_file_at_ref(repo, reference, Path::new("go.mod")).ok();
            let module_path = go_mod.and_then(|content| {
                String::from_utf8(content).ok().and_then(|s| {
                    s.lines()
                        .find(|l| l.trim().starts_with("module "))
                        .map(|l| l.trim().strip_prefix("module ").unwrap().trim().to_string())
                })
            });
            Box::new(match module_path {
                Some(mp) => GoFrontend::with_module_path(mp),
                None => GoFrontend::new(),
            })
        }
        Language::Python => Box::new(PythonFrontend::new()),
        Language::Ruby => Box::new(RubyFrontend::new()),
        Language::Rust => {
            // Try to read Cargo.toml at this ref
            let cargo_toml =
                crate::git::read_file_at_ref(repo, reference, Path::new("Cargo.toml")).ok();
            let crate_name = cargo_toml.and_then(|content| {
                String::from_utf8(content)
                    .ok()
                    .and_then(|s| RustFrontend::parse_crate_name(&s))
            });
            Box::new(match crate_name {
                Some(name) => RustFrontend::with_crate_name(name),
                None => RustFrontend::new(),
            })
        }
    };

    let mut builder = GraphBuilder::new();

    for file_path in &files {
        let source = match crate::git::read_file_at_ref(repo, reference, file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let imports = frontend.extract_imports(&source, file_path);
        let file_paths: Vec<PathBuf> = files.clone();

        for raw in &imports {
            if raw.confidence == ImportConfidence::External
                || raw.confidence == ImportConfidence::Dynamic
                || raw.confidence == ImportConfidence::Unresolvable
            {
                continue;
            }

            if let Some(target) = frontend.resolve(raw, root, &file_paths) {
                builder.add_import(&ResolvedImport {
                    source_module: file_path.clone(),
                    target_module: target,
                    location: SourceLocation {
                        file: file_path.clone(),
                        line: raw.line,
                        column: raw.column,
                    },
                });
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
}

fn compute_graph_diff(
    base: &DepGraph,
    head: &DepGraph,
    _base_ref: &str,
    _head_ref: &str,
) -> RawDiff {
    // Collect node names
    let base_nodes: HashSet<String> = base.node_indices().map(|i| base[i].name.clone()).collect();
    let head_nodes: HashSet<String> = head.node_indices().map(|i| head[i].name.clone()).collect();

    let nodes_added = head_nodes.difference(&base_nodes).count();
    let nodes_removed = base_nodes.difference(&head_nodes).count();

    // Collect edges as (from_name, to_name)
    let base_edges: HashSet<(String, String)> = base
        .edge_indices()
        .map(|e| {
            let (s, t) = base.edge_endpoints(e).unwrap();
            (base[s].name.clone(), base[t].name.clone())
        })
        .collect();
    let head_edges: HashSet<(String, String)> = head
        .edge_indices()
        .map(|e| {
            let (s, t) = head.edge_endpoints(e).unwrap();
            (head[s].name.clone(), head[t].name.clone())
        })
        .collect();

    let new_edge_set: Vec<_> = head_edges.difference(&base_edges).cloned().collect();
    let removed_edge_set: Vec<_> = base_edges.difference(&head_edges).cloned().collect();

    let edges_added = new_edge_set.len();
    let edges_removed = removed_edge_set.len();

    // Build edge change details
    let head_node_map: HashMap<String, petgraph::graph::NodeIndex> = head
        .node_indices()
        .map(|i| (head[i].name.clone(), i))
        .collect();

    let new_edges: Vec<EdgeChange> = new_edge_set
        .iter()
        .map(|(from, to)| {
            let source_locs = if let (Some(&from_idx), Some(&to_idx)) =
                (head_node_map.get(from), head_node_map.get(to))
            {
                head.find_edge(from_idx, to_idx)
                    .map(|e| head[e].source_locations.clone())
                    .unwrap_or_default()
            } else {
                vec![]
            };
            EdgeChange {
                from: from.clone(),
                to: to.clone(),
                source_locations: source_locs,
            }
        })
        .collect();

    let removed_edges: Vec<EdgeChange> = removed_edge_set
        .iter()
        .map(|(from, to)| EdgeChange {
            from: from.clone(),
            to: to.clone(),
            source_locations: vec![],
        })
        .collect();

    // Fan-out changes for nodes that exist in both
    let base_node_map: HashMap<String, petgraph::graph::NodeIndex> = base
        .node_indices()
        .map(|i| (base[i].name.clone(), i))
        .collect();

    let mut fanout_changes = Vec::new();
    for name in base_nodes.intersection(&head_nodes) {
        if let (Some(&base_idx), Some(&head_idx)) =
            (base_node_map.get(name), head_node_map.get(name))
        {
            let fo_before = crate::metrics::fanout::fan_out(base, base_idx);
            let fo_after = crate::metrics::fanout::fan_out(head, head_idx);

            if fo_before != fo_after {
                let base_weights: Vec<usize> = base
                    .edges_directed(base_idx, Direction::Outgoing)
                    .map(|e| e.weight().weight)
                    .collect();
                let head_weights: Vec<usize> = head
                    .edges_directed(head_idx, Direction::Outgoing)
                    .map(|e| e.weight().weight)
                    .collect();

                let entropy_before = crate::metrics::entropy::shannon_entropy(&base_weights);
                let entropy_after = crate::metrics::entropy::shannon_entropy(&head_weights);

                // Find new targets for this node
                let base_targets: HashSet<String> = base
                    .edges_directed(base_idx, Direction::Outgoing)
                    .map(|e| base[e.target()].name.clone())
                    .collect();
                let head_targets: HashSet<String> = head
                    .edges_directed(head_idx, Direction::Outgoing)
                    .map(|e| head[e.target()].name.clone())
                    .collect();

                let new_target_edges: Vec<EdgeChange> = head_targets
                    .difference(&base_targets)
                    .map(|target| {
                        let locs = head_node_map
                            .get(target)
                            .and_then(|&tidx| {
                                head.find_edge(head_idx, tidx)
                                    .map(|e| head[e].source_locations.clone())
                            })
                            .unwrap_or_default();
                        EdgeChange {
                            from: name.clone(),
                            to: target.clone(),
                            source_locations: locs,
                        }
                    })
                    .collect();

                fanout_changes.push(FanoutChange {
                    node: name.clone(),
                    fanout_before: fo_before,
                    fanout_after: fo_after,
                    delta: fo_after as isize - fo_before as isize,
                    entropy_before: (entropy_before * 100.0).round() / 100.0,
                    entropy_after: (entropy_after * 100.0).round() / 100.0,
                    new_targets: new_target_edges,
                });
            }
        }
    }

    // SCC changes
    let base_sccs = find_non_trivial_sccs(base);
    let head_sccs = find_non_trivial_sccs(head);

    let base_summary = Summary::from_graph(base);
    let head_summary = Summary::from_graph(head);

    // Simple SCC diff: match by Jaccard similarity
    let mut matched_base: HashSet<usize> = HashSet::new();
    let mut matched_head: HashSet<usize> = HashSet::new();
    let mut enlarged = Vec::new();

    for (hi, h_scc) in head_sccs.iter().enumerate() {
        let h_members: HashSet<&String> = h_scc.members.iter().collect();
        let mut best_match = None;
        let mut best_jaccard = 0.0f64;

        for (bi, b_scc) in base_sccs.iter().enumerate() {
            if matched_base.contains(&bi) {
                continue;
            }
            let b_members: HashSet<&String> = b_scc.members.iter().collect();
            let intersection = h_members.intersection(&b_members).count() as f64;
            let union = h_members.union(&b_members).count() as f64;
            let jaccard = if union > 0.0 {
                intersection / union
            } else {
                0.0
            };
            if jaccard > best_jaccard {
                best_jaccard = jaccard;
                best_match = Some(bi);
            }
        }

        if best_jaccard > 0.5 {
            if let Some(bi) = best_match {
                matched_base.insert(bi);
                matched_head.insert(hi);
                if h_scc.size > base_sccs[bi].size {
                    enlarged.push(SccChange {
                        members: h_scc.members.clone(),
                        size: h_scc.size,
                    });
                }
            }
        }
    }

    let new_sccs: Vec<SccChange> = head_sccs
        .iter()
        .enumerate()
        .filter(|(i, _)| !matched_head.contains(i))
        .map(|(_, s)| SccChange {
            members: s.members.clone(),
            size: s.size,
        })
        .collect();

    let resolved_sccs: Vec<SccChange> = base_sccs
        .iter()
        .enumerate()
        .filter(|(i, _)| !matched_base.contains(i))
        .map(|(_, s)| SccChange {
            members: s.members.clone(),
            size: s.size,
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
            max_depth_delta: head_summary.max_depth as isize - base_summary.max_depth as isize,
            total_complexity_delta: head_summary.total_complexity as isize
                - base_summary.total_complexity as isize,
        },
        new_edges,
        removed_edges,
        fanout_changes,
        scc_changes: SccChanges {
            new_sccs,
            enlarged_sccs: enlarged,
            resolved_sccs,
        },
    }
}

fn evaluate_policies(diff: &RawDiff, conditions: &[FailCondition]) -> (Verdict, Vec<String>) {
    let mut reasons = Vec::new();

    for condition in conditions {
        match condition {
            FailCondition::FanoutIncrease => {
                if diff.fanout_changes.iter().any(|c| c.delta > 0) {
                    reasons.push("fanout-increase".to_string());
                }
            }
            FailCondition::FanoutThreshold(threshold) => {
                if diff
                    .fanout_changes
                    .iter()
                    .any(|c| c.fanout_after > *threshold)
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
                if diff.summary_delta.mean_fanout_delta > 0.0 {
                    reasons.push("entropy-increase".to_string());
                }
            }
            FailCondition::NewEdge => {
                if !diff.new_edges.is_empty() {
                    reasons.push("new-edge".to_string());
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
