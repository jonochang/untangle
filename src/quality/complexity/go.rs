use crate::quality::complexity::{binary_has_ops, count_decisions, ComplexityFrontend};
use crate::quality::FunctionInfo;
use crate::walk::Language;
use std::path::Path;

pub struct GoComplexity;

impl GoComplexity {
    fn cyclomatic_complexity(node: tree_sitter::Node, source: &[u8]) -> usize {
        let decisions = count_decisions(node, source, |n, src| match n.kind() {
            "if_statement" | "for_statement" | "case_clause" | "type_case_clause"
            | "communication_case" => true,
            "binary_expression" => binary_has_ops(n, src, &["&&", "||"]),
            _ => false,
        });
        1 + decisions
    }

    fn receiver_type(node: tree_sitter::Node, source: &[u8]) -> Option<String> {
        let recv = node.child_by_field_name("receiver")?;
        let mut stack = vec![recv];
        while let Some(current) = stack.pop() {
            match current.kind() {
                "type_identifier" => {
                    return current.utf8_text(source).ok().map(|s| s.to_string());
                }
                "pointer_type" => {
                    if let Some(inner) = current.child(1) {
                        return inner.utf8_text(source).ok().map(|s| s.to_string());
                    }
                }
                _ => {
                    let mut cursor = current.walk();
                    for child in current.children(&mut cursor) {
                        stack.push(child);
                    }
                }
            }
        }
        None
    }
}

impl ComplexityFrontend for GoComplexity {
    fn language(&self) -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn extract_functions(&self, source: &[u8], file_path: &Path) -> Vec<FunctionInfo> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&self.language())
            .expect("failed to set Go language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return vec![],
        };

        let mut functions = Vec::new();
        let mut cursor = tree.root_node().walk();
        let mut stack = vec![tree.root_node()];

        while let Some(node) = stack.pop() {
            if node.kind() == "function_declaration" || node.kind() == "method_declaration" {
                let name = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source).ok())
                    .unwrap_or("<anonymous>");
                let full_name = if node.kind() == "method_declaration" {
                    if let Some(recv) = Self::receiver_type(node, source) {
                        format!("{recv}.{name}")
                    } else {
                        name.to_string()
                    }
                } else {
                    name.to_string()
                };
                let start_line = node.start_position().row + 1;
                let end_line = node.end_position().row + 1;
                let cc = Self::cyclomatic_complexity(node, source);
                functions.push(FunctionInfo {
                    name: full_name,
                    file: file_path.to_path_buf(),
                    start_line,
                    end_line,
                    cyclomatic_complexity: cc,
                    language: Language::Go,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_functions_and_method_receivers() {
        let source = br#"
package main

type Service struct{}

func plain(a bool) {
    if a && true {
    }
}

func (s Service) ValueMethod() {
    plain(true)
}

func (s *Service) PointerMethod(flag bool) {
    if flag || false {
    }
}
"#;

        let functions = GoComplexity.extract_functions(source, Path::new("main.go"));
        let names: Vec<_> = functions.iter().map(|f| f.name.as_str()).collect();

        assert!(names.contains(&"plain"));
        assert!(names.contains(&"Service.ValueMethod"));
        assert!(names.contains(&"Service.PointerMethod"));
        let pointer = functions
            .iter()
            .find(|f| f.name == "Service.PointerMethod")
            .unwrap();
        assert!(pointer.cyclomatic_complexity >= 2);
    }
}
