use crate::quality::complexity::{binary_has_ops, count_decisions, ComplexityFrontend};
use crate::quality::FunctionInfo;
use crate::walk::Language;
use std::path::Path;

pub struct RubyComplexity;

impl RubyComplexity {
    fn cyclomatic_complexity(node: tree_sitter::Node, source: &[u8]) -> usize {
        let decisions = count_decisions(node, source, |n, src| match n.kind() {
            "if" | "unless" | "elsif" | "while" | "until" | "for" | "when" | "rescue"
            | "conditional" => true,
            "binary" => binary_has_ops(n, src, &["&&", "||", " and ", " or "]),
            _ => false,
        });
        1 + decisions
    }

    fn extract_with_context(
        node: tree_sitter::Node,
        source: &[u8],
        file_path: &Path,
        context: &mut Vec<String>,
        out: &mut Vec<FunctionInfo>,
    ) {
        let kind = node.kind();
        if kind == "class" || kind == "module" {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source) {
                    context.push(name.to_string());
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        Self::extract_with_context(child, source, file_path, context, out);
                    }
                    context.pop();
                    return;
                }
            }
        }

        if kind == "method" || kind == "singleton_method" {
            let name = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source).ok())
                .unwrap_or("<anonymous>");
            let prefix = if context.is_empty() {
                "".to_string()
            } else {
                format!("{}.", context.join("."))
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
                language: Language::Ruby,
            });
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::extract_with_context(child, source, file_path, context, out);
        }
    }
}

impl ComplexityFrontend for RubyComplexity {
    fn language(&self) -> tree_sitter::Language {
        tree_sitter_ruby::LANGUAGE.into()
    }

    fn extract_functions(&self, source: &[u8], file_path: &Path) -> Vec<FunctionInfo> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&self.language())
            .expect("failed to set Ruby language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return vec![],
        };

        let mut out = Vec::new();
        let mut context = Vec::new();
        Self::extract_with_context(tree.root_node(), source, file_path, &mut context, &mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_nested_methods_and_singleton_methods() {
        let source = br#"
module Outer
  class Inner
    def call(flag)
      if flag && true
        work
      end
    end
  end

  def self.build(enabled)
    rescue_value = 1
    unless enabled or false
      rescue_value += 1
    end
    rescue
      nil
  end
end
"#;

        let functions = RubyComplexity.extract_functions(source, Path::new("app.rb"));
        let names: Vec<_> = functions.iter().map(|f| f.name.as_str()).collect();

        assert!(names.contains(&"Outer.Inner.call"));
        assert!(names.contains(&"Outer.build"));
        let call = functions
            .iter()
            .find(|f| f.name == "Outer.Inner.call")
            .unwrap();
        assert!(call.cyclomatic_complexity >= 2);
    }
}
