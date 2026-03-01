use crate::parse::common::{ImportConfidence, ImportKind, RawImport};
use crate::parse::ParseFrontend;
use std::path::{Path, PathBuf};
use streaming_iterator::StreamingIterator;

pub struct RustFrontend {
    crate_name: Option<String>,
}

impl RustFrontend {
    pub fn new() -> Self {
        Self { crate_name: None }
    }

    pub fn with_crate_name(name: String) -> Self {
        Self {
            crate_name: Some(name),
        }
    }

    /// Read the crate name from Cargo.toml at the given project root.
    pub fn read_cargo_toml(root: &Path) -> Option<String> {
        let cargo_path = root.join("Cargo.toml");
        let content = std::fs::read_to_string(cargo_path).ok()?;
        Self::parse_crate_name(&content)
    }

    /// Parse crate name from Cargo.toml content string.
    pub fn parse_crate_name(content: &str) -> Option<String> {
        let table: toml::Table = content.parse().ok()?;
        table
            .get("package")?
            .get("name")?
            .as_str()
            .map(|s| s.to_string())
    }

    /// Classify an import path.
    fn classify_import(&self, path: &str) -> ImportConfidence {
        let first_segment = path.split("::").next().unwrap_or(path);
        match first_segment {
            "crate" | "super" | "self" => ImportConfidence::Resolved,
            "std" | "core" | "alloc" => ImportConfidence::External,
            _ => {
                // Check if the import matches the crate's own name (with - normalized to _)
                if let Some(ref crate_name) = self.crate_name {
                    let normalized_crate = crate_name.replace('-', "_");
                    let normalized_segment = first_segment.replace('-', "_");
                    if normalized_crate == normalized_segment {
                        return ImportConfidence::Resolved;
                    }
                }
                ImportConfidence::External
            }
        }
    }

    /// Recursively walk a use_declaration argument subtree to collect full import paths.
    fn collect_paths(
        node: tree_sitter::Node,
        source: &[u8],
        prefix: &str,
        paths: &mut Vec<String>,
    ) {
        match node.kind() {
            "scoped_identifier" => {
                // path::name — build full path from text
                let text = node.utf8_text(source).unwrap_or_default().to_string();
                let full = if prefix.is_empty() {
                    text
                } else {
                    format!("{prefix}::{text}")
                };
                paths.push(full);
            }
            "scoped_use_list" => {
                // path::{item1, item2}
                // The path part is typically the first child (scoped_identifier or identifier)
                // The use_list is the child with kind "use_list"
                let mut path_prefix = String::new();
                let mut use_list_node = None;

                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    match child.kind() {
                        "use_list" => {
                            use_list_node = Some(child);
                        }
                        // Skip punctuation
                        "::" | "{" | "}" => {}
                        _ => {
                            // This is the path prefix part
                            let text = child.utf8_text(source).unwrap_or_default();
                            if !text.is_empty() {
                                path_prefix = if prefix.is_empty() {
                                    text.to_string()
                                } else {
                                    format!("{prefix}::{text}")
                                };
                            }
                        }
                    }
                }

                if let Some(list) = use_list_node {
                    let mut list_cursor = list.walk();
                    for item in list.children(&mut list_cursor) {
                        if item.kind() == "," {
                            continue;
                        }
                        Self::collect_paths(item, source, &path_prefix, paths);
                    }
                }
            }
            "use_as_clause" => {
                // `Foo as Bar` — use the original path (first child)
                if let Some(child) = node.child(0) {
                    Self::collect_paths(child, source, prefix, paths);
                }
            }
            "use_wildcard" => {
                // `path::*`
                let text = node.utf8_text(source).unwrap_or_default().to_string();
                let full = if prefix.is_empty() {
                    text
                } else {
                    format!("{prefix}::{text}")
                };
                paths.push(full);
            }
            "use_list" => {
                // Bare use list: {item1, item2}
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if !child.is_named() {
                        continue;
                    }
                    Self::collect_paths(child, source, prefix, paths);
                }
            }
            "identifier" | "self" | "super" | "crate" => {
                let text = node.utf8_text(source).unwrap_or_default().to_string();
                let full = if prefix.is_empty() {
                    text
                } else {
                    format!("{prefix}::{text}")
                };
                paths.push(full);
            }
            _ => {
                // Only handle named nodes (skip punctuation like ::, {, }, etc.)
                if node.is_named() {
                    let text = node.utf8_text(source).unwrap_or_default().to_string();
                    if !text.is_empty() {
                        let full = if prefix.is_empty() {
                            text
                        } else {
                            format!("{prefix}::{text}")
                        };
                        paths.push(full);
                    }
                }
            }
        }
    }
}

impl Default for RustFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl ParseFrontend for RustFrontend {
    fn language(&self) -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn extract_imports(&self, source: &[u8], file_path: &Path) -> Vec<RawImport> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&self.language())
            .expect("failed to set Rust language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return vec![],
        };

        let query_str = r#"(use_declaration argument: (_) @arg)"#;
        let query = tree_sitter::Query::new(&self.language(), query_str)
            .expect("failed to compile Rust use query");

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), source);

        let mut imports = Vec::new();
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let line = node.start_position().row + 1;
                let column = Some(node.start_position().column);

                let mut paths = Vec::new();
                Self::collect_paths(node, source, "", &mut paths);

                for path in paths {
                    if path.is_empty() {
                        continue;
                    }
                    let confidence = self.classify_import(&path);
                    imports.push(RawImport {
                        raw_path: path,
                        source_file: file_path.to_path_buf(),
                        line,
                        column,
                        kind: ImportKind::Direct,
                        confidence,
                    });
                }
            }
        }

        imports
    }

    fn resolve(
        &self,
        raw: &RawImport,
        project_root: &Path,
        project_files: &[PathBuf],
    ) -> Option<PathBuf> {
        if raw.confidence != ImportConfidence::Resolved {
            return None;
        }

        let path = &raw.raw_path;
        // Strip trailing ::* for glob imports (resolve the parent module)
        let path = path.strip_suffix("::*").unwrap_or(path);
        let first_segment = path.split("::").next().unwrap_or(path);

        // Normalize source_file to project-relative for super/self resolution
        let relative_source = raw
            .source_file
            .strip_prefix(project_root)
            .unwrap_or(&raw.source_file);

        match first_segment {
            "crate" => {
                // crate::foo::bar -> strip "crate::", convert :: to /, look for src/foo/bar.rs or src/foo/bar/mod.rs
                let rest = path.strip_prefix("crate::")?;
                let module_path = Self::to_file_module_path(rest);
                let candidate = PathBuf::from("src").join(module_path.replace("::", "/"));
                Self::find_module_file(&candidate, project_root, project_files)
            }
            "super" => {
                // super::foo -> go up from source file's directory
                let source_dir = relative_source.parent()?;
                // If source file is foo/mod.rs, its module IS foo/, so super is foo's parent.
                // If source file is foo.rs, its module is foo (at the same level as foo.rs), so super is foo's parent.
                let base_dir = if relative_source.file_name()?.to_str()? == "mod.rs" {
                    source_dir.parent()?
                } else {
                    source_dir.parent()?
                };
                let rest = path.strip_prefix("super::")?;
                let module_path = Self::to_file_module_path(rest);
                let candidate = base_dir.join(module_path.replace("::", "/"));
                Self::find_module_file(&candidate, project_root, project_files)
            }
            "self" => {
                // self::foo -> same directory as source file
                let source_dir = relative_source.parent()?;
                // If mod.rs, self::foo is a child of the current directory.
                // If foo.rs, self::foo is NOT possible in standard Rust (must be in a module).
                // But we handle it by looking in the same directory.
                let rest = path.strip_prefix("self::")?;
                let module_path = Self::to_file_module_path(rest);
                let candidate = source_dir.join(module_path.replace("::", "/"));
                Self::find_module_file(&candidate, project_root, project_files)
            }
            _ => {
                // Handle crate-name imports: `use my_crate::foo::bar` → treat like `crate::foo::bar`
                if let Some(ref crate_name) = self.crate_name {
                    let normalized_crate = crate_name.replace('-', "_");
                    let normalized_segment = first_segment.replace('-', "_");
                    if normalized_crate == normalized_segment {
                        let rest = path.strip_prefix(first_segment)?.strip_prefix("::")?;
                        let module_path = Self::to_file_module_path(rest);
                        let candidate = PathBuf::from("src").join(module_path.replace("::", "/"));
                        return Self::find_module_file(&candidate, project_root, project_files);
                    }
                }
                None
            }
        }
    }
}

impl RustFrontend {
    /// Extract the file-level module path from a use path.
    /// For `foo::bar::Baz`, we want `foo::bar` if `Baz` is a type/function,
    /// but we can't tell statically. We try the longest path first, then shorter.
    /// In practice, Rust modules map to files, so we try the full path first.
    fn to_file_module_path(rest: &str) -> &str {
        // Return the full path — resolve will try to find matching files
        rest
    }

    /// Try to find a module file matching the candidate path.
    /// Looks for `candidate.rs` or `candidate/mod.rs`, trying progressively
    /// shorter paths to handle `crate::module::Type` → `src/module.rs`.
    fn find_module_file(
        candidate: &Path,
        project_root: &Path,
        project_files: &[PathBuf],
    ) -> Option<PathBuf> {
        // Try full path first, then progressively shorter
        let mut path = candidate.to_path_buf();
        loop {
            // Try path.rs
            let rs_file = path.with_extension("rs");
            if Self::file_exists_in_project(&rs_file, project_root, project_files) {
                return Some(rs_file);
            }

            // Try path/mod.rs
            let mod_file = path.join("mod.rs");
            if Self::file_exists_in_project(&mod_file, project_root, project_files) {
                return Some(mod_file);
            }

            // Strip last component and try again (handles Type names at the end)
            match path.parent() {
                Some(parent) if parent != path && parent.file_name().is_some() && parent != Path::new("src") => {
                    path = parent.to_path_buf();
                }
                _ => break,
            }
        }
        None
    }

    fn file_exists_in_project(
        relative: &Path,
        project_root: &Path,
        project_files: &[PathBuf],
    ) -> bool {
        project_files
            .iter()
            .any(|f| f.strip_prefix(project_root).unwrap_or(f).eq(relative))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_simple_use() {
        let source = b"use std::collections::HashMap;";
        let frontend = RustFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("src/main.rs"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "std::collections::HashMap");
        assert_eq!(imports[0].confidence, ImportConfidence::External);
    }

    #[test]
    fn extracts_crate_import() {
        let source = b"use crate::module::Item;";
        let frontend = RustFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("src/main.rs"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "crate::module::Item");
        assert_eq!(imports[0].confidence, ImportConfidence::Resolved);
    }

    #[test]
    fn extracts_scoped_use_list() {
        let source = b"use crate::module::{Foo, Bar};";
        let frontend = RustFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("src/main.rs"));
        assert_eq!(imports.len(), 2);
        let paths: Vec<&str> = imports.iter().map(|i| i.raw_path.as_str()).collect();
        assert!(paths.contains(&"crate::module::Foo"));
        assert!(paths.contains(&"crate::module::Bar"));
    }

    #[test]
    fn extracts_super_import() {
        let source = b"use super::something;";
        let frontend = RustFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("src/sub/mod.rs"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "super::something");
        assert_eq!(imports[0].confidence, ImportConfidence::Resolved);
    }

    #[test]
    fn classifies_crate_as_resolved() {
        let frontend = RustFrontend::new();
        assert_eq!(
            frontend.classify_import("crate::foo::bar"),
            ImportConfidence::Resolved
        );
    }

    #[test]
    fn classifies_std_as_external() {
        let frontend = RustFrontend::new();
        assert_eq!(
            frontend.classify_import("std::collections::HashMap"),
            ImportConfidence::External
        );
        assert_eq!(
            frontend.classify_import("core::fmt::Debug"),
            ImportConfidence::External
        );
        assert_eq!(
            frontend.classify_import("alloc::vec::Vec"),
            ImportConfidence::External
        );
    }

    #[test]
    fn classifies_crate_name_import_as_resolved() {
        let frontend = RustFrontend::with_crate_name("my-crate".to_string());
        assert_eq!(
            frontend.classify_import("my_crate::module::Item"),
            ImportConfidence::Resolved
        );
    }

    #[test]
    fn resolves_crate_name_import() {
        let frontend = RustFrontend::with_crate_name("my-crate".to_string());
        let project_root = Path::new("/project");
        let project_files = vec![
            PathBuf::from("/project/src/main.rs"),
            PathBuf::from("/project/src/module.rs"),
        ];
        let raw = RawImport {
            raw_path: "my_crate::module::Item".into(),
            source_file: PathBuf::from("src/main.rs"),
            line: 1,
            column: None,
            kind: ImportKind::Direct,
            confidence: ImportConfidence::Resolved,
        };
        let resolved = frontend.resolve(&raw, project_root, &project_files);
        assert_eq!(resolved, Some(PathBuf::from("src/module.rs")));
    }

    #[test]
    fn resolves_glob_import() {
        let frontend = RustFrontend::new();
        let project_root = Path::new("/project");
        let project_files = vec![
            PathBuf::from("/project/src/main.rs"),
            PathBuf::from("/project/src/module.rs"),
        ];
        let raw = RawImport {
            raw_path: "crate::module::*".into(),
            source_file: PathBuf::from("src/main.rs"),
            line: 1,
            column: None,
            kind: ImportKind::Direct,
            confidence: ImportConfidence::Resolved,
        };
        let resolved = frontend.resolve(&raw, project_root, &project_files);
        assert_eq!(resolved, Some(PathBuf::from("src/module.rs")));
    }

    #[test]
    fn resolves_self_with_absolute_source_path() {
        let frontend = RustFrontend::new();
        let project_root = Path::new("/project");
        let project_files = vec![
            PathBuf::from("/project/src/foo/mod.rs"),
            PathBuf::from("/project/src/foo/bar.rs"),
        ];
        let raw = RawImport {
            raw_path: "self::bar::Thing".into(),
            source_file: PathBuf::from("/project/src/foo/mod.rs"),
            line: 1,
            column: None,
            kind: ImportKind::Direct,
            confidence: ImportConfidence::Resolved,
        };
        let resolved = frontend.resolve(&raw, project_root, &project_files);
        assert_eq!(resolved, Some(PathBuf::from("src/foo/bar.rs")));
    }

    #[test]
    fn resolves_crate_import() {
        let frontend = RustFrontend::new();
        let project_root = Path::new("/project");
        let project_files = vec![
            PathBuf::from("/project/src/main.rs"),
            PathBuf::from("/project/src/module.rs"),
        ];
        let raw = RawImport {
            raw_path: "crate::module::Item".into(),
            source_file: PathBuf::from("src/main.rs"),
            line: 1,
            column: None,
            kind: ImportKind::Direct,
            confidence: ImportConfidence::Resolved,
        };
        let resolved = frontend.resolve(&raw, project_root, &project_files);
        assert_eq!(resolved, Some(PathBuf::from("src/module.rs")));
    }

    #[test]
    fn extracts_nested_scoped_list() {
        let source = b"use std::collections::{HashMap, BTreeMap};";
        let frontend = RustFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("src/main.rs"));
        assert_eq!(imports.len(), 2);
        let paths: Vec<&str> = imports.iter().map(|i| i.raw_path.as_str()).collect();
        assert!(paths.contains(&"std::collections::HashMap"));
        assert!(paths.contains(&"std::collections::BTreeMap"));
    }

    #[test]
    fn extracts_use_as() {
        let source = b"use std::io::Result as IoResult;";
        let frontend = RustFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("src/main.rs"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "std::io::Result");
    }

    #[test]
    fn extracts_self_import() {
        let source = b"use self::submodule::Thing;";
        let frontend = RustFrontend::new();
        let imports = frontend.extract_imports(source, Path::new("src/lib.rs"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "self::submodule::Thing");
        assert_eq!(imports[0].confidence, ImportConfidence::Resolved);
    }

    #[test]
    fn parse_crate_name_from_toml() {
        let content = r#"
[package]
name = "my_crate"
version = "0.1.0"
edition = "2021"
"#;
        assert_eq!(
            RustFrontend::parse_crate_name(content),
            Some("my_crate".to_string())
        );
    }
}
