use crate::graph::ir::{DepGraph, EdgeKind, GraphEdge, GraphNode, NodeKind};
use crate::parse::common::SourceLocation;
use crate::walk::Language;
use std::collections::HashMap;
use std::path::PathBuf;

/// Resolved import ready for graph insertion
#[derive(Debug, Clone)]
pub struct ResolvedImport {
    pub source_module: PathBuf,
    pub target_module: PathBuf,
    pub location: SourceLocation,
    pub language: Option<Language>,
}

/// Builds a DepGraph from resolved imports with node and edge deduplication.
pub struct GraphBuilder {
    graph: DepGraph,
    node_map: HashMap<PathBuf, petgraph::graph::NodeIndex>,
}

impl GraphBuilder {
    pub fn new() -> Self {
        Self {
            graph: DepGraph::new(),
            node_map: HashMap::new(),
        }
    }

    /// Get or create a node for the given module path.
    fn ensure_node(
        &mut self,
        path: &PathBuf,
        language: Option<Language>,
    ) -> petgraph::graph::NodeIndex {
        if let Some(&idx) = self.node_map.get(path) {
            return idx;
        }
        let name = path
            .to_string_lossy()
            .replace(['/', '\\'], ".")
            .trim_end_matches(".py")
            .trim_end_matches(".rb")
            .trim_end_matches(".go")
            .trim_end_matches(".rs")
            .to_string();
        let idx = self.graph.add_node(GraphNode {
            kind: NodeKind::Module,
            path: path.clone(),
            name,
            span: None,
            language,
        });
        self.node_map.insert(path.clone(), idx);
        idx
    }

    /// Add a resolved import to the graph.
    pub fn add_import(&mut self, import: &ResolvedImport) {
        let source_idx = self.ensure_node(&import.source_module, import.language);
        let target_idx = self.ensure_node(&import.target_module, import.language);

        // Check if edge already exists
        if let Some(edge_idx) = self.graph.find_edge(source_idx, target_idx) {
            let edge = &mut self.graph[edge_idx];
            if !edge.source_locations.contains(&import.location) {
                edge.source_locations.push(import.location.clone());
            }
        } else {
            self.graph.add_edge(
                source_idx,
                target_idx,
                GraphEdge {
                    kind: EdgeKind::Import,
                    source_locations: vec![import.location.clone()],
                    weight: 1,
                },
            );
        }
    }

    /// Add all resolved imports to the graph.
    pub fn add_imports(&mut self, imports: &[ResolvedImport]) {
        for import in imports {
            self.add_import(import);
        }
    }

    /// Consume the builder and return the built graph.
    pub fn build(self) -> DepGraph {
        self.graph
    }

    /// Get the node map (path -> node index).
    pub fn node_map(&self) -> &HashMap<PathBuf, petgraph::graph::NodeIndex> {
        &self.node_map
    }
}

impl Default for GraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicates_nodes() {
        let mut builder = GraphBuilder::new();
        let import1 = ResolvedImport {
            source_module: PathBuf::from("a.go"),
            target_module: PathBuf::from("b.go"),
            location: SourceLocation {
                file: PathBuf::from("a.go"),
                line: 1,
                column: None,
            },
            language: None,
        };
        let import2 = ResolvedImport {
            source_module: PathBuf::from("a.go"),
            target_module: PathBuf::from("c.go"),
            location: SourceLocation {
                file: PathBuf::from("a.go"),
                line: 2,
                column: None,
            },
            language: None,
        };
        builder.add_import(&import1);
        builder.add_import(&import2);
        let graph = builder.build();
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);
    }

    #[test]
    fn deduplicates_edges_accumulates_locations() {
        let mut builder = GraphBuilder::new();
        let import1 = ResolvedImport {
            source_module: PathBuf::from("a.go"),
            target_module: PathBuf::from("b.go"),
            location: SourceLocation {
                file: PathBuf::from("a.go"),
                line: 1,
                column: None,
            },
            language: None,
        };
        let import2 = ResolvedImport {
            source_module: PathBuf::from("a.go"),
            target_module: PathBuf::from("b.go"),
            location: SourceLocation {
                file: PathBuf::from("a.go"),
                line: 5,
                column: None,
            },
            language: None,
        };
        builder.add_import(&import1);
        builder.add_import(&import2);
        let graph = builder.build();
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        let edge = graph.edge_weights().next().unwrap();
        assert_eq!(edge.source_locations.len(), 2);
    }
}
