use crate::analysis_context::{RustPackage, RustWorkspaceContext};
use crate::parse::common::{ImportConfidence, ImportKind, RawImport};
use crate::parse::ParseFrontend;
use std::path::{Path, PathBuf};
use streaming_iterator::StreamingIterator;

pub struct RustFrontend {
    workspace: Option<RustWorkspaceContext>,
}

impl RustFrontend {
    pub fn new() -> Self {
        Self { workspace: None }
    }

    pub fn with_workspace(workspace: RustWorkspaceContext) -> Self {
        Self {
            workspace: Some(workspace),
        }
    }

    pub fn with_crate_name(name: String) -> Self {
        Self::with_workspace(RustWorkspaceContext::from_packages(vec![RustPackage {
            normalized_name: normalize_crate_segment(&name),
            name,
            manifest_dir: PathBuf::new(),
            source_roots: vec![PathBuf::from("src")],
            entry_source_root: PathBuf::from("src"),
        }]))
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

    fn classify_import(&self, path: &str, file_path: &Path) -> ImportConfidence {
        let first_segment = path.split("::").next().unwrap_or(path);
        match first_segment {
            "crate" | "super" | "self" => ImportConfidence::Resolved,
            "std" | "core" | "alloc" => ImportConfidence::External,
            _ => {
                if let Some(workspace) = &self.workspace {
                    if workspace
                        .package_by_name
                        .contains_key(&normalize_crate_segment(first_segment))
                    {
                        return ImportConfidence::Resolved;
                    }

                    if let Some(package) = workspace.find_package_for_file(file_path, Path::new(""))
                    {
                        if package.normalized_name == normalize_crate_segment(first_segment) {
                            return ImportConfidence::Resolved;
                        }
                    }
                }

                ImportConfidence::External
            }
        }
    }

    fn current_package<'a>(
        &'a self,
        file_path: &Path,
        project_root: &Path,
    ) -> Option<&'a RustPackage> {
        self.workspace
            .as_ref()?
            .find_package_for_file(file_path, project_root)
    }

    fn current_source_root(&self, file_path: &Path, project_root: &Path) -> Option<PathBuf> {
        let package = self.current_package(file_path, project_root)?;
        let absolute = absolutize(file_path, project_root);
        package
            .source_root_for_file(&absolute)
            .map(Path::to_path_buf)
            .or_else(|| Some(package.entry_source_root.clone()))
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
                let text = node.utf8_text(source).unwrap_or_default().to_string();
                let full = if prefix.is_empty() {
                    text
                } else {
                    format!("{prefix}::{text}")
                };
                paths.push(full);
            }
            "scoped_use_list" => {
                let mut path_prefix = String::new();
                let mut use_list_node = None;

                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    match child.kind() {
                        "use_list" => {
                            use_list_node = Some(child);
                        }
                        "::" | "{" | "}" => {}
                        _ => {
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
                if let Some(child) = node.child(0) {
                    Self::collect_paths(child, source, prefix, paths);
                }
            }
            "use_wildcard" => {
                let text = node.utf8_text(source).unwrap_or_default().to_string();
                let full = if prefix.is_empty() {
                    text
                } else {
                    format!("{prefix}::{text}")
                };
                paths.push(full);
            }
            "use_list" => {
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
                    let confidence = self.classify_import(&path, file_path);
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

        let workspace = self.workspace.as_ref()?;
        let current_package = workspace.find_package_for_file(&raw.source_file, project_root)?;
        let current_source_root = self.current_source_root(&raw.source_file, project_root)?;

        let path = raw.raw_path.strip_suffix("::*").unwrap_or(&raw.raw_path);
        let first_segment = path.split("::").next().unwrap_or(path);

        match first_segment {
            "crate" => {
                let rest = path.strip_prefix("crate::")?;
                let candidate = current_source_root.join(rest.replace("::", "/"));
                Self::find_module_file(current_package, &candidate, project_files, project_root)
            }
            "super" => {
                let absolute_source = absolutize(&raw.source_file, project_root);
                let source_dir = absolute_source.parent()?;
                let base_dir = if absolute_source.file_name()?.to_str()? == "mod.rs" {
                    source_dir.parent()?
                } else {
                    source_dir
                };
                let parent_dir = base_dir.parent()?;
                let rest = path.strip_prefix("super::")?;
                let candidate = parent_dir.join(rest.replace("::", "/"));
                Self::find_module_file(current_package, &candidate, project_files, project_root)
            }
            "self" => {
                let absolute_source = absolutize(&raw.source_file, project_root);
                let source_dir = absolute_source.parent()?;
                let rest = path.strip_prefix("self::")?;
                let candidate = source_dir.join(rest.replace("::", "/"));
                Self::find_module_file(current_package, &candidate, project_files, project_root)
            }
            _ => {
                let target_package = workspace
                    .package_by_name
                    .get(&normalize_crate_segment(first_segment))?;
                let rest = path.strip_prefix(first_segment)?.strip_prefix("::")?;
                let candidate = target_package
                    .entry_source_root
                    .join(rest.replace("::", "/"));
                Self::find_module_file(target_package, &candidate, project_files, project_root)
            }
        }
    }
}

impl RustFrontend {
    fn find_module_file(
        package: &RustPackage,
        candidate: &Path,
        project_files: &[PathBuf],
        project_root: &Path,
    ) -> Option<PathBuf> {
        let mut path = candidate.to_path_buf();
        loop {
            let rs_file = path.with_extension("rs");
            if Self::file_exists_in_project(&rs_file, project_root, project_files) {
                return package.module_id_for_file(&rs_file);
            }

            let mod_file = path.join("mod.rs");
            if Self::file_exists_in_project(&mod_file, project_root, project_files) {
                return package.module_id_for_file(&mod_file);
            }

            match path.parent() {
                Some(parent) if parent != path && parent.file_name().is_some() => {
                    path = parent.to_path_buf();
                }
                _ => break,
            }
        }
        None
    }

    fn file_exists_in_project(
        relative_or_absolute: &Path,
        project_root: &Path,
        project_files: &[PathBuf],
    ) -> bool {
        let absolute_candidate = absolutize(relative_or_absolute, project_root);
        project_files.iter().any(|file| {
            let absolute_file = absolutize(file, project_root);
            absolute_file == absolute_candidate
        })
    }
}

fn absolutize(path: &Path, project_root: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}

fn normalize_crate_segment(segment: &str) -> String {
    segment.replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> RustWorkspaceContext {
        RustWorkspaceContext::from_packages(vec![
            RustPackage {
                name: "my-crate".to_string(),
                normalized_name: "my_crate".to_string(),
                manifest_dir: PathBuf::from("/project"),
                source_roots: vec![PathBuf::from("/project/src")],
                entry_source_root: PathBuf::from("/project/src"),
            },
            RustPackage {
                name: "other-crate".to_string(),
                normalized_name: "other_crate".to_string(),
                manifest_dir: PathBuf::from("/project/crates/other"),
                source_roots: vec![PathBuf::from("/project/crates/other/src")],
                entry_source_root: PathBuf::from("/project/crates/other/src"),
            },
        ])
    }

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
        let frontend = RustFrontend::with_workspace(workspace());
        let imports = frontend.extract_imports(source, Path::new("/project/src/main.rs"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "crate::module::Item");
        assert_eq!(imports[0].confidence, ImportConfidence::Resolved);
    }

    #[test]
    fn extracts_scoped_use_list() {
        let source = b"use crate::module::{Foo, Bar};";
        let frontend = RustFrontend::with_workspace(workspace());
        let imports = frontend.extract_imports(source, Path::new("/project/src/main.rs"));
        assert_eq!(imports.len(), 2);
        let paths: Vec<&str> = imports.iter().map(|i| i.raw_path.as_str()).collect();
        assert!(paths.contains(&"crate::module::Foo"));
        assert!(paths.contains(&"crate::module::Bar"));
    }

    #[test]
    fn extracts_super_import() {
        let source = b"use super::something;";
        let frontend = RustFrontend::with_workspace(workspace());
        let imports = frontend.extract_imports(source, Path::new("/project/src/sub/mod.rs"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "super::something");
        assert_eq!(imports[0].confidence, ImportConfidence::Resolved);
    }

    #[test]
    fn classifies_workspace_crate_import_as_resolved() {
        let frontend = RustFrontend::with_workspace(workspace());
        assert_eq!(
            frontend.classify_import("my_crate::module::Item", Path::new("/project/src/main.rs")),
            ImportConfidence::Resolved
        );
        assert_eq!(
            frontend.classify_import(
                "other_crate::module::Item",
                Path::new("/project/src/main.rs")
            ),
            ImportConfidence::Resolved
        );
    }

    #[test]
    fn resolves_workspace_crate_name_import() {
        let frontend = RustFrontend::with_workspace(workspace());
        let project_root = Path::new("/project");
        let project_files = vec![
            PathBuf::from("/project/src/main.rs"),
            PathBuf::from("/project/src/module.rs"),
            PathBuf::from("/project/crates/other/src/lib.rs"),
            PathBuf::from("/project/crates/other/src/module.rs"),
        ];
        let raw = RawImport {
            raw_path: "other_crate::module::Item".into(),
            source_file: PathBuf::from("/project/src/main.rs"),
            line: 1,
            column: None,
            kind: ImportKind::Direct,
            confidence: ImportConfidence::Resolved,
        };
        let resolved = frontend.resolve(&raw, project_root, &project_files);
        assert_eq!(resolved, Some(PathBuf::from("other_crate/src/module.rs")));
    }

    #[test]
    fn resolves_glob_import() {
        let frontend = RustFrontend::with_workspace(workspace());
        let project_root = Path::new("/project");
        let project_files = vec![
            PathBuf::from("/project/src/main.rs"),
            PathBuf::from("/project/src/module.rs"),
        ];
        let raw = RawImport {
            raw_path: "crate::module::*".into(),
            source_file: PathBuf::from("/project/src/main.rs"),
            line: 1,
            column: None,
            kind: ImportKind::Direct,
            confidence: ImportConfidence::Resolved,
        };
        let resolved = frontend.resolve(&raw, project_root, &project_files);
        assert_eq!(resolved, Some(PathBuf::from("my_crate/src/module.rs")));
    }

    #[test]
    fn resolves_self_with_absolute_source_path() {
        let frontend = RustFrontend::with_workspace(workspace());
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
        assert_eq!(resolved, Some(PathBuf::from("my_crate/src/foo/bar.rs")));
    }

    #[test]
    fn resolves_crate_import() {
        let frontend = RustFrontend::with_workspace(workspace());
        let project_root = Path::new("/project");
        let project_files = vec![
            PathBuf::from("/project/src/main.rs"),
            PathBuf::from("/project/src/module.rs"),
        ];
        let raw = RawImport {
            raw_path: "crate::module::Item".into(),
            source_file: PathBuf::from("/project/src/main.rs"),
            line: 1,
            column: None,
            kind: ImportKind::Direct,
            confidence: ImportConfidence::Resolved,
        };
        let resolved = frontend.resolve(&raw, project_root, &project_files);
        assert_eq!(resolved, Some(PathBuf::from("my_crate/src/module.rs")));
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
}
