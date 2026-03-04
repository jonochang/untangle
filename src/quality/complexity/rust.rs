use crate::quality::complexity::{binary_has_ops, count_decisions, ComplexityFrontend};
use crate::quality::FunctionInfo;
use crate::walk::Language;
use std::path::Path;

pub struct RustComplexity;

impl RustComplexity {
    fn function_name(node: tree_sitter::Node, source: &[u8]) -> String {
        let name = node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source).ok())
            .unwrap_or("<anonymous>");

        // Attempt to prefix with impl type if available
        let mut parent = node.parent();
        while let Some(p) = parent {
            if p.kind() == "impl_item" {
                let type_name = p
                    .child_by_field_name("type")
                    .or_else(|| {
                        p.children(&mut p.walk())
                            .find(|c| c.kind() == "type_identifier")
                    })
                    .or_else(|| {
                        p.children(&mut p.walk())
                            .find(|c| c.kind() == "scoped_type_identifier")
                    })
                    .and_then(|n| n.utf8_text(source).ok())
                    .unwrap_or("");
                if !type_name.is_empty() {
                    return format!("{type_name}::{name}");
                }
                break;
            }
            parent = p.parent();
        }

        name.to_string()
    }

    fn cyclomatic_complexity(node: tree_sitter::Node, source: &[u8]) -> usize {
        let decisions = count_decisions(node, source, |n, src| match n.kind() {
            "if_expression"
            | "while_expression"
            | "while_let_expression"
            | "loop_expression"
            | "for_expression"
            | "match_arm"
            | "try_expression" => true,
            "binary_expression" => binary_has_ops(n, src, &["&&", "||"]),
            _ => false,
        });
        1 + decisions
    }
}

impl ComplexityFrontend for RustComplexity {
    fn language(&self) -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn extract_functions(&self, source: &[u8], file_path: &Path) -> Vec<FunctionInfo> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&self.language())
            .expect("failed to set Rust language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return vec![],
        };

        let mut functions = Vec::new();
        let mut cursor = tree.root_node().walk();
        let mut stack = vec![tree.root_node()];

        while let Some(node) = stack.pop() {
            if node.kind() == "function_item" {
                let name = Self::function_name(node, source);
                let start_line = node.start_position().row + 1;
                let end_line = node.end_position().row + 1;
                let cc = Self::cyclomatic_complexity(node, source);
                functions.push(FunctionInfo {
                    name,
                    file: file_path.to_path_buf(),
                    start_line,
                    end_line,
                    cyclomatic_complexity: cc,
                    language: Language::Rust,
                });
            }
            cursor.reset(node);
            for child in node.children(&mut cursor) {
                stack.push(child);
            }
        }

        functions
    }
}
