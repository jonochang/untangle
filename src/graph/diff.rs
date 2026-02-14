use crate::graph::ir::DepGraph;
use crate::parse::common::SourceLocation;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DiffResult {
    pub base_ref: String,
    pub head_ref: String,
    pub verdict: Verdict,
    pub reasons: Vec<String>,
    pub elapsed_ms: u64,
    pub modules_per_second: f64,
    pub summary_delta: SummaryDelta,
    pub new_edges: Vec<EdgeChange>,
    pub removed_edges: Vec<EdgeChange>,
    pub fanout_changes: Vec<FanoutChange>,
    pub scc_changes: SccChanges,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Fail,
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

/// Compute the diff between two dependency graphs.
pub fn compute_diff(
    _base: &DepGraph,
    _head: &DepGraph,
    _base_ref: &str,
    _head_ref: &str,
) -> DiffResult {
    todo!("Phase 3: implement graph diff computation")
}
