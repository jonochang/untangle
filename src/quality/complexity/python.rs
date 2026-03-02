use crate::quality::complexity::{binary_has_ops, count_decisions, ComplexityFrontend};
use crate::quality::FunctionInfo;
use crate::walk::Language;
use std::path::Path;

pub struct PythonComplexity;

impl PythonComplexity {
    fn cyclomatic_complexity(node: tree_sitter::Node, source: &[u8]) -> usize {
        let decisions = count_decisions(node, source, |n, src| match n.kind() {
            "if_statement"
            | "elif_clause"
            | "while_statement"
            | "for_statement"
            | "except_clause"
            | "conditional_expression" => true,
            "boolean_operator" => binary_has_ops(n, src, &["and", "or"]),
            _ => false,
        });
        1 + decisions
    }

    fn extract_functions_with_context(
        node: tree_sitter::Node,
        source: &[u8],
        file_path: &Path,
        class_stack: &mut Vec<String>,
        out: &mut Vec<FunctionInfo>,
    ) {
        let kind = node.kind();
        if kind == "class_definition" {
            if let Some(name) = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source).ok())
            {
                class_stack.push(name.to_string());
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    Self::extract_functions_with_context(child, source, file_path, class_stack, out);
                }
                class_stack.pop();
                return;
            }
        }

        if kind == "function_definition" {
            let name = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source).ok())
                .unwrap_or("<anonymous>");
            let prefix = if class_stack.is_empty() {
                "".to_string()
            } else {
                format!("{}.", class_stack.join("."))
            };
            let full_name = format!("{prefix}{name}");
            let start_line = node.start_position().row + 1;
            let end_line = node.end_position().row + 1;
            let cc = Self::cyclomatic_complexity(node, source);
            out.push(FunctionInfo {
                name: full_name,
                file: file_path.to_path_buf(),
                start_line,
                end_line,
                cyclomatic_complexity: cc,
                language: Language::Python,
            });
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::extract_functions_with_context(child, source, file_path, class_stack, out);
        }
    }
}

impl ComplexityFrontend for PythonComplexity {
    fn language(&self) -> tree_sitter::Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn extract_functions(&self, source: &[u8], file_path: &Path) -> Vec<FunctionInfo> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&self.language())
            .expect("failed to set Python language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return vec![],
        };

        let mut out = Vec::new();
        let mut stack = Vec::new();
        Self::extract_functions_with_context(tree.root_node(), source, file_path, &mut stack, &mut out);
        out
    }
}
