use crate::quality::FunctionInfo;
use std::path::Path;

pub mod go;
pub mod python;
pub mod ruby;
pub mod rust;

/// Implemented per language.
pub trait ComplexityFrontend {
    fn language(&self) -> tree_sitter::Language;
    fn extract_functions(&self, source: &[u8], file_path: &Path) -> Vec<FunctionInfo>;
}

fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

fn matches_binary_op(node: &tree_sitter::Node, source: &[u8], ops: &[&str]) -> bool {
    let text = node_text(node, source);
    ops.iter().any(|op| text.contains(op))
}

pub fn count_decisions<F>(root: tree_sitter::Node, source: &[u8], mut is_decision: F) -> usize
where
    F: FnMut(&tree_sitter::Node, &[u8]) -> bool,
{
    let mut count = 0usize;
    let mut cursor = root.walk();
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        if is_decision(&node, source) {
            count += 1;
        }
        cursor.reset(node);
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }

    count
}

pub fn binary_has_ops(node: &tree_sitter::Node, source: &[u8], ops: &[&str]) -> bool {
    matches_binary_op(node, source, ops)
}
