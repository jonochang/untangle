use crate::parse::common::SourceLocation;
use petgraph::graph::DiGraph;
use serde::Serialize;
use std::path::PathBuf;

/// Unique identifier for a graph node
pub type NodeId = petgraph::graph::NodeIndex;

/// The dependency graph
pub type DepGraph = DiGraph<GraphNode, GraphEdge>;

#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    /// Discriminator for future function-level granularity
    pub kind: NodeKind,
    /// Canonical path relative to project root
    pub path: PathBuf,
    /// Human-readable name (e.g., "src.api.handler" for Python)
    pub name: String,
    /// Line span — populated for Function nodes (future)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<(usize, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Module,
    Function, // future
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    /// All import statements that contributed to this edge
    pub source_locations: Vec<SourceLocation>,
    /// Edge weight (always 1 for binary weighting in v1)
    pub weight: usize,
}
