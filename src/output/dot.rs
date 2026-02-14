use crate::errors::Result;
use crate::graph::ir::DepGraph;
use std::io::Write;

/// Write the dependency graph in Graphviz DOT format.
pub fn write_dot<W: Write>(writer: &mut W, graph: &DepGraph) -> Result<()> {
    writeln!(writer, "digraph dependencies {{")?;
    writeln!(writer, "    rankdir=LR;")?;
    writeln!(
        writer,
        "    node [shape=box, style=filled, fillcolor=lightblue];"
    )?;
    writeln!(writer)?;

    // Write nodes
    for idx in graph.node_indices() {
        let node = &graph[idx];
        let label = &node.name;
        writeln!(writer, "    \"{}\" [label=\"{}\"];", node.name, label)?;
    }
    writeln!(writer)?;

    // Write edges
    for edge in graph.edge_indices() {
        let (source, target) = graph.edge_endpoints(edge).unwrap();
        let source_name = &graph[source].name;
        let target_name = &graph[target].name;
        let weight = &graph[edge];
        let loc_count = weight.source_locations.len();
        if loc_count > 1 {
            writeln!(
                writer,
                "    \"{}\" -> \"{}\" [label=\"{} refs\"];",
                source_name, target_name, loc_count
            )?;
        } else {
            writeln!(writer, "    \"{}\" -> \"{}\";", source_name, target_name)?;
        }
    }

    writeln!(writer, "}}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::ir::{GraphEdge, GraphNode, NodeKind};
    use std::path::PathBuf;

    #[test]
    fn dot_output_basic() {
        let mut graph = DepGraph::new();
        let a = graph.add_node(GraphNode {
            kind: NodeKind::Module,
            path: PathBuf::from("a"),
            name: "a".to_string(),
            span: None,
        });
        let b = graph.add_node(GraphNode {
            kind: NodeKind::Module,
            path: PathBuf::from("b"),
            name: "b".to_string(),
            span: None,
        });
        graph.add_edge(
            a,
            b,
            GraphEdge {
                source_locations: vec![],
                weight: 1,
            },
        );

        let mut output = Vec::new();
        write_dot(&mut output, &graph).unwrap();
        let dot = String::from_utf8(output).unwrap();
        assert!(dot.contains("digraph dependencies"));
        assert!(dot.contains("\"a\" -> \"b\""));
    }
}
