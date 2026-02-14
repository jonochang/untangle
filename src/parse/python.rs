use crate::parse::common::{ImportConfidence, ImportKind, RawImport};
use crate::parse::ParseFrontend;
use std::path::{Path, PathBuf};
use streaming_iterator::StreamingIterator;

pub struct PythonFrontend;

impl PythonFrontend {
    pub fn new() -> Self {
        Self
    }

    fn extract_relative_imports(
        tree: &tree_sitter::Tree,
        source: &[u8],
        file_path: &Path,
        imports: &mut Vec<RawImport>,
    ) {
        Self::walk_for_relative_imports(tree.root_node(), source, file_path, imports);
    }

    fn walk_for_relative_imports(
        node: tree_sitter::Node,
        source: &[u8],
        file_path: &Path,
        imports: &mut Vec<RawImport>,
    ) {
        if node.kind() == "import_from_statement" {
            if let Some(module_node) = node.child_by_field_name("module_name") {
                if module_node.kind() == "relative_import" {
                    let text = module_node
                        .utf8_text(source)
                        .unwrap_or_default()
                        .to_string();
                    let level = text.chars().take_while(|&c| c == '.').count();
                    let module_part = text[level..].to_string();

                    let mut names = Vec::new();
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.kind() == "dotted_name" {
                            if let Ok(name) = child.utf8_text(source) {
                                names.push(name.to_string());
                            }
                        }
                    }

                    imports.push(RawImport {
                        raw_path: text,
                        source_file: file_path.to_path_buf(),
                        line: module_node.start_position().row + 1,
                        column: Some(module_node.start_position().column),
                        kind: ImportKind::RelativeImport {
                            level,
                            module: if module_part.is_empty() {
                                None
                            } else {
                                Some(module_part)
                            },
                            names,
                        },
                        confidence: ImportConfidence::Resolved,
                    });
                    return;
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_for_relative_imports(child, source, file_path, imports);
        }
    }
}

impl Default for PythonFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl ParseFrontend for PythonFrontend {
    fn language(&self) -> tree_sitter::Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn extract_imports(&self, source: &[u8], file_path: &Path) -> Vec<RawImport> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&self.language())
            .expect("failed to set Python language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return vec![],
        };

        let lang = self.language();
        let mut imports = Vec::new();

        // Query for `import x` and `import x.y.z`
        let import_query_str = r#"(import_statement name: (dotted_name) @import_path)"#;
        let import_query = tree_sitter::Query::new(&lang, import_query_str)
            .expect("failed to compile Python import query");

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&import_query, tree.root_node(), source);
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let text = node.utf8_text(source).unwrap_or_default().to_string();
                if !text.is_empty() {
                    imports.push(RawImport {
                        raw_path: text,
                        source_file: file_path.to_path_buf(),
                        line: node.start_position().row + 1,
                        column: Some(node.start_position().column),
                        kind: ImportKind::Direct,
                        confidence: ImportConfidence::Resolved,
                    });
                }
            }
        }

        // Query for `from x import y` (absolute)
        let from_query_str = r#"(import_from_statement module_name: (dotted_name) @module name: (dotted_name) @name)"#;
        let from_query = tree_sitter::Query::new(&lang, from_query_str)
            .expect("failed to compile Python from-import query");

        let mut cursor2 = tree_sitter::QueryCursor::new();
        let mut matches2 = cursor2.matches(&from_query, tree.root_node(), source);
        while let Some(m) = matches2.next() {
            let module_capture = m.captures.iter().find(|c| c.index == 0);
            let name_captures: Vec<_> = m.captures.iter().filter(|c| c.index == 1).collect();

            if let Some(module_cap) = module_capture {
                let module = module_cap
                    .node
                    .utf8_text(source)
                    .unwrap_or_default()
                    .to_string();
                let names: Vec<String> = name_captures
                    .iter()
                    .map(|c| c.node.utf8_text(source).unwrap_or_default().to_string())
                    .collect();

                if !module.is_empty() {
                    imports.push(RawImport {
                        raw_path: module.clone(),
                        source_file: file_path.to_path_buf(),
                        line: module_cap.node.start_position().row + 1,
                        column: Some(module_cap.node.start_position().column),
                        kind: ImportKind::FromImport { module, names },
                        confidence: ImportConfidence::Resolved,
                    });
                }
            }
        }

        // Relative imports via tree walk
        Self::extract_relative_imports(&tree, source, file_path, &mut imports);

        imports
    }

    fn resolve(
        &self,
        raw: &RawImport,
        project_root: &Path,
        project_files: &[PathBuf],
    ) -> Option<PathBuf> {
        match &raw.kind {
            ImportKind::Direct | ImportKind::FromImport { .. } => {
                let module_path = raw.raw_path.replace('.', "/");
                // Try as package (__init__.py)
                let init_path = PathBuf::from(format!("{module_path}/__init__.py"));
                if project_files
                    .iter()
                    .any(|f| f.strip_prefix(project_root).unwrap_or(f) == init_path)
                {
                    return Some(init_path);
                }
                // Try as module file
                let file_path = PathBuf::from(format!("{module_path}.py"));
                if project_files
                    .iter()
                    .any(|f| f.strip_prefix(project_root).unwrap_or(f) == file_path)
                {
                    return Some(file_path);
                }
                None
            }
            ImportKind::RelativeImport { level, module, .. } => {
                let source_dir = raw.source_file.parent()?;
                let mut base = source_dir.to_path_buf();
                for _ in 1..*level {
                    base = base.parent()?.to_path_buf();
                }
                if let Some(mod_name) = module {
                    let mod_path = mod_name.replace('.', "/");
                    base = base.join(mod_path);
                }
                let init_path = base.join("__init__.py");
                let relative_init = init_path
                    .strip_prefix(project_root)
                    .unwrap_or(&init_path)
                    .to_path_buf();
                if project_files
                    .iter()
                    .any(|f| f.strip_prefix(project_root).unwrap_or(f) == relative_init)
                {
                    return Some(relative_init);
                }
                let py_path = base.with_extension("py");
                let relative_py = py_path
                    .strip_prefix(project_root)
                    .unwrap_or(&py_path)
                    .to_path_buf();
                if project_files
                    .iter()
                    .any(|f| f.strip_prefix(project_root).unwrap_or(f) == relative_py)
                {
                    return Some(relative_py);
                }
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_simple_import() {
        let source = b"import os";
        let frontend = PythonFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("test.py"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "os");
    }

    #[test]
    fn extracts_dotted_import() {
        let source = b"import os.path";
        let frontend = PythonFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("test.py"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "os.path");
    }

    #[test]
    fn extracts_from_import() {
        let source = b"from foo.bar import baz";
        let frontend = PythonFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("test.py"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "foo.bar");
        assert!(matches!(imports[0].kind, ImportKind::FromImport { .. }));
    }

    #[test]
    fn extracts_relative_import() {
        let source = b"from . import utils";
        let frontend = PythonFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("pkg/test.py"));
        assert_eq!(imports.len(), 1);
        assert!(matches!(
            imports[0].kind,
            ImportKind::RelativeImport { level: 1, .. }
        ));
    }
}
