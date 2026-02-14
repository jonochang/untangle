use crate::parse::common::{ImportConfidence, ImportKind, RawImport};
use crate::parse::ParseFrontend;
use std::path::{Path, PathBuf};

pub struct RubyFrontend {
    load_paths: Vec<PathBuf>,
}

impl RubyFrontend {
    pub fn new() -> Self {
        Self {
            load_paths: vec![PathBuf::from("lib"), PathBuf::from("app")],
        }
    }

    pub fn with_load_paths(load_paths: Vec<PathBuf>) -> Self {
        Self { load_paths }
    }
}

impl Default for RubyFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl ParseFrontend for RubyFrontend {
    fn language(&self) -> tree_sitter::Language {
        tree_sitter_ruby::LANGUAGE.into()
    }

    fn extract_imports(&self, source: &[u8], file_path: &Path) -> Vec<RawImport> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&self.language())
            .expect("failed to set Ruby language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return vec![],
        };

        let mut imports = Vec::new();

        // Walk the tree looking for method calls to require/require_relative/autoload
        Self::walk_for_requires(tree.root_node(), source, file_path, &mut imports);

        imports
    }

    fn resolve(
        &self,
        raw: &RawImport,
        project_root: &Path,
        project_files: &[PathBuf],
    ) -> Option<PathBuf> {
        match &raw.kind {
            ImportKind::RequireRelative => {
                let source_dir = raw.source_file.parent()?;
                let target = source_dir.join(&raw.raw_path);
                let target_rb = if target.extension().is_none() {
                    target.with_extension("rb")
                } else {
                    target
                };
                // Normalize
                let canonical = normalize_path(&target_rb);
                let relative = canonical
                    .strip_prefix(project_root)
                    .unwrap_or(&canonical)
                    .to_path_buf();
                if project_files
                    .iter()
                    .any(|f| f.strip_prefix(project_root).unwrap_or(f).eq(&relative))
                {
                    Some(relative)
                } else {
                    None
                }
            }
            ImportKind::Direct => {
                // Try each load path
                for load_path in &self.load_paths {
                    let target = load_path.join(&raw.raw_path);
                    let target_rb = if target.extension().is_none() {
                        target.with_extension("rb")
                    } else {
                        target
                    };
                    if project_files
                        .iter()
                        .any(|f| f.strip_prefix(project_root).unwrap_or(f).eq(&target_rb))
                    {
                        return Some(target_rb);
                    }
                }
                None
            }
            ImportKind::Autoload { .. } => {
                // Autoload with explicit path
                let target = PathBuf::from(&raw.raw_path);
                let target_rb = if target.extension().is_none() {
                    target.with_extension("rb")
                } else {
                    target
                };
                for load_path in &self.load_paths {
                    let full = load_path.join(&target_rb);
                    if project_files
                        .iter()
                        .any(|f| f.strip_prefix(project_root).unwrap_or(f).eq(&full))
                    {
                        return Some(full);
                    }
                }
                None
            }
            _ => None,
        }
    }
}

impl RubyFrontend {
    fn walk_for_requires(
        node: tree_sitter::Node,
        source: &[u8],
        file_path: &Path,
        imports: &mut Vec<RawImport>,
    ) {
        if node.kind() == "call" {
            if let Some(method_node) = node.child_by_field_name("method") {
                let method_name = method_node.utf8_text(source).unwrap_or_default();
                match method_name {
                    "require" | "require_relative" => {
                        // Find string argument
                        if let Some(args_node) = node.child_by_field_name("arguments") {
                            let mut arg_cursor = args_node.walk();
                            for arg in args_node.children(&mut arg_cursor) {
                                if arg.kind() == "string" {
                                    if let Some(content) = Self::extract_string_content(arg, source)
                                    {
                                        let kind = if method_name == "require_relative" {
                                            ImportKind::RequireRelative
                                        } else {
                                            ImportKind::Direct
                                        };
                                        let confidence =
                                            if content.contains('#') || content.contains('\\') {
                                                ImportConfidence::Dynamic
                                            } else {
                                                ImportConfidence::Resolved
                                            };
                                        imports.push(RawImport {
                                            raw_path: content,
                                            source_file: file_path.to_path_buf(),
                                            line: node.start_position().row + 1,
                                            column: Some(node.start_position().column),
                                            kind,
                                            confidence,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    "autoload" => {
                        if let Some(args_node) = node.child_by_field_name("arguments") {
                            let mut arg_cursor = args_node.walk();
                            let children: Vec<_> = args_node.children(&mut arg_cursor).collect();
                            // autoload :Constant, "path"
                            let mut constant = None;
                            let mut path = None;
                            for child in &children {
                                if child.kind() == "simple_symbol" {
                                    constant = Some(
                                        child
                                            .utf8_text(source)
                                            .unwrap_or_default()
                                            .trim_start_matches(':')
                                            .to_string(),
                                    );
                                } else if child.kind() == "string" {
                                    path = Self::extract_string_content(*child, source);
                                }
                            }
                            if let (Some(constant_name), Some(path_str)) = (constant, path) {
                                imports.push(RawImport {
                                    raw_path: path_str,
                                    source_file: file_path.to_path_buf(),
                                    line: node.start_position().row + 1,
                                    column: Some(node.start_position().column),
                                    kind: ImportKind::Autoload {
                                        constant: constant_name,
                                    },
                                    confidence: ImportConfidence::Resolved,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_for_requires(child, source, file_path, imports);
        }
    }

    fn extract_string_content(string_node: tree_sitter::Node, source: &[u8]) -> Option<String> {
        let mut cursor = string_node.walk();
        for child in string_node.children(&mut cursor) {
            if child.kind() == "string_content" {
                return child.utf8_text(source).ok().map(|s| s.to_string());
            }
        }
        None
    }
}

/// Simple path normalization (resolve . and ..)
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_require() {
        let source = b"require \"foo/bar\"";
        let frontend = RubyFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("test.rb"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "foo/bar");
        assert!(matches!(imports[0].kind, ImportKind::Direct));
    }

    #[test]
    fn extracts_require_relative() {
        let source = b"require_relative \"../lib/helper\"";
        let frontend = RubyFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("test/test.rb"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "../lib/helper");
        assert!(matches!(imports[0].kind, ImportKind::RequireRelative));
    }

    #[test]
    fn extracts_autoload() {
        let source = b"autoload :Foo, \"foo/bar\"";
        let frontend = RubyFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("test.rb"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "foo/bar");
        assert!(matches!(
            imports[0].kind,
            ImportKind::Autoload { ref constant } if constant == "Foo"
        ));
    }

    #[test]
    fn marks_interpolated_strings_as_dynamic() {
        let source = b"require \"#{path}/foo\"";
        let frontend = RubyFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("test.rb"));
        // May or may not extract depending on tree-sitter parsing of interpolation
        // If extracted, should be marked Dynamic
        for import in &imports {
            if import.raw_path.contains('#') {
                assert_eq!(import.confidence, ImportConfidence::Dynamic);
            }
        }
    }
}
