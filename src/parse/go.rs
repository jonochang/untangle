use crate::parse::common::{ImportConfidence, ImportKind, RawImport};
use crate::parse::ParseFrontend;
use std::path::{Path, PathBuf};
use streaming_iterator::StreamingIterator;

/// Parse the module path from go.mod content string.
pub fn parse_go_mod_module(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("module ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

pub struct GoFrontend {
    /// Module path from go.mod (e.g., "github.com/user/project")
    module_path: Option<String>,
    /// Whether to exclude stdlib imports (default: true)
    exclude_stdlib: bool,
}

impl GoFrontend {
    pub fn new() -> Self {
        Self {
            module_path: None,
            exclude_stdlib: true,
        }
    }

    /// Create a GoFrontend with the module path read from go.mod.
    pub fn with_module_path(module_path: String) -> Self {
        Self {
            module_path: Some(module_path),
            exclude_stdlib: true,
        }
    }

    /// Set whether to exclude stdlib imports.
    pub fn with_exclude_stdlib(mut self, exclude: bool) -> Self {
        self.exclude_stdlib = exclude;
        self
    }

    /// Read the module path from a go.mod file.
    pub fn read_go_mod(project_root: &Path) -> Option<String> {
        let go_mod_path = project_root.join("go.mod");
        let content = std::fs::read_to_string(go_mod_path).ok()?;
        parse_go_mod_module(&content)
    }

    /// Classify an import path as internal, stdlib, or external.
    fn classify_import(&self, import_path: &str) -> ImportConfidence {
        if let Some(ref module_path) = self.module_path {
            if import_path.starts_with(module_path) {
                return ImportConfidence::Resolved;
            }
        }
        // stdlib packages have no dots in their path
        if !import_path.contains('.') {
            return if self.exclude_stdlib {
                ImportConfidence::External
            } else {
                // Include stdlib as resolved when exclude_stdlib is false
                ImportConfidence::Resolved
            };
        }
        ImportConfidence::External
    }
}

impl Default for GoFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl ParseFrontend for GoFrontend {
    fn language(&self) -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn extract_imports(&self, source: &[u8], file_path: &Path) -> Vec<RawImport> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&self.language())
            .expect("failed to set Go language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return vec![],
        };

        let query_str = r#"(import_spec path: (interpreted_string_literal) @import_path)"#;
        let query = tree_sitter::Query::new(&self.language(), query_str)
            .expect("failed to compile Go import query");

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), source);

        let mut imports = Vec::new();
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let text = node
                    .utf8_text(source)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string();

                if text.is_empty() {
                    continue;
                }

                let confidence = self.classify_import(&text);

                imports.push(RawImport {
                    raw_path: text,
                    source_file: file_path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: Some(node.start_position().column),
                    kind: ImportKind::Direct,
                    confidence,
                });
            }
        }

        imports
    }

    fn resolve(
        &self,
        raw: &RawImport,
        _project_root: &Path,
        project_files: &[PathBuf],
    ) -> Option<PathBuf> {
        if raw.confidence != ImportConfidence::Resolved {
            return None;
        }

        if let Some(ref module_path) = self.module_path {
            let relative = raw
                .raw_path
                .strip_prefix(module_path)?
                .trim_start_matches('/');

            if relative.is_empty() {
                return None;
            }

            Some(PathBuf::from(relative))
        } else {
            // Fallback: no go.mod — try directory-based matching
            // Check if any project file lives under a directory matching the import path
            let import_path = &raw.raw_path;
            for file in project_files {
                let file_str = file.to_string_lossy();
                if file_str.contains(import_path) {
                    return Some(PathBuf::from(import_path));
                }
            }
            tracing::warn!(
                "No go.mod found; could not resolve import '{}'",
                raw.raw_path
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_go_mod_module_extracts_path() {
        let content = "module github.com/example/web\n\ngo 1.21\n";
        assert_eq!(
            parse_go_mod_module(content),
            Some("github.com/example/web".to_string())
        );
    }

    #[test]
    fn parse_go_mod_module_none_for_empty() {
        assert_eq!(parse_go_mod_module(""), None);
        assert_eq!(parse_go_mod_module("go 1.21\n"), None);
    }

    #[test]
    fn extracts_single_import() {
        let source = br#"package main

import "fmt"
"#;
        let frontend = GoFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("main.go"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "fmt");
        assert_eq!(imports[0].line, 3);
    }

    #[test]
    fn extracts_grouped_imports() {
        let source = br#"package main

import (
    "fmt"
    "os"
    "github.com/user/project/pkg/foo"
)
"#;
        let frontend = GoFrontend::with_module_path("github.com/user/project".into());
        let imports = frontend.extract_imports(source, Path::new("main.go"));
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].raw_path, "fmt");
        assert_eq!(imports[1].raw_path, "os");
        assert_eq!(imports[2].raw_path, "github.com/user/project/pkg/foo");
        assert_eq!(imports[2].confidence, ImportConfidence::Resolved);
    }

    #[test]
    fn classifies_stdlib_as_external() {
        let frontend = GoFrontend::with_module_path("github.com/user/project".into());
        assert_eq!(frontend.classify_import("fmt"), ImportConfidence::External);
        assert_eq!(
            frontend.classify_import("net/http"),
            ImportConfidence::External
        );
    }

    #[test]
    fn classifies_internal_as_resolved() {
        let frontend = GoFrontend::with_module_path("github.com/user/project".into());
        assert_eq!(
            frontend.classify_import("github.com/user/project/pkg/foo"),
            ImportConfidence::Resolved
        );
    }

    #[test]
    fn resolves_internal_import() {
        let frontend = GoFrontend::with_module_path("github.com/user/project".into());
        let raw = RawImport {
            raw_path: "github.com/user/project/pkg/foo".into(),
            source_file: PathBuf::from("main.go"),
            line: 3,
            column: None,
            kind: ImportKind::Direct,
            confidence: ImportConfidence::Resolved,
        };
        let resolved = frontend.resolve(&raw, Path::new("."), &[]);
        assert_eq!(resolved, Some(PathBuf::from("pkg/foo")));
    }

    #[test]
    fn skips_external_imports() {
        let frontend = GoFrontend::with_module_path("github.com/user/project".into());
        let raw = RawImport {
            raw_path: "fmt".into(),
            source_file: PathBuf::from("main.go"),
            line: 3,
            column: None,
            kind: ImportKind::Direct,
            confidence: ImportConfidence::External,
        };
        assert_eq!(frontend.resolve(&raw, Path::new("."), &[]), None);
    }
}
