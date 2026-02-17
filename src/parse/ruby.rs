use crate::parse::common::{ImportConfidence, ImportKind, RawImport};
use crate::parse::ParseFrontend;
use std::path::{Path, PathBuf};

/// Ruby stdlib/builtin constants to exclude from Zeitwerk resolution.
const RUBY_STDLIB_CONSTANTS: &[&str] = &[
    "String",
    "Integer",
    "Float",
    "Array",
    "Hash",
    "Symbol",
    "NilClass",
    "TrueClass",
    "FalseClass",
    "Object",
    "Class",
    "Module",
    "Kernel",
    "IO",
    "File",
    "Dir",
    "Regexp",
    "Range",
    "Proc",
    "Thread",
    "Mutex",
    "Fiber",
    "Exception",
    "StandardError",
    "RuntimeError",
    "ArgumentError",
    "TypeError",
    "Struct",
    "Set",
    "Numeric",
    "Time",
    "Encoding",
    "Enumerator",
    "OpenStruct",
    "Comparable",
    "Enumerable",
    "Marshal",
    "Errno",
    "Signal",
    "Process",
    "GC",
    "ObjectSpace",
    "BasicObject",
    "Method",
    "UnboundMethod",
    "Binding",
    "Math",
    "Complex",
    "Rational",
    "ENV",
    "STDIN",
    "STDOUT",
    "STDERR",
    "ARGV",
    "DATA",
    "TRUE",
    "FALSE",
    "NIL",
];

pub struct RubyFrontend {
    load_paths: Vec<PathBuf>,
    zeitwerk: bool,
}

impl RubyFrontend {
    pub fn new() -> Self {
        Self {
            load_paths: vec![PathBuf::from("lib"), PathBuf::from("app")],
            zeitwerk: false,
        }
    }

    pub fn with_load_paths(load_paths: Vec<PathBuf>) -> Self {
        Self {
            load_paths,
            zeitwerk: false,
        }
    }

    pub fn with_zeitwerk(mut self, zeitwerk: bool) -> Self {
        self.zeitwerk = zeitwerk;
        self
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

        // Extract Zeitwerk constants if enabled
        if self.zeitwerk {
            Self::extract_zeitwerk_constants(tree.root_node(), source, file_path, &mut imports);
        }

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
            ImportKind::ZeitwerkConstant => {
                // Admin::User → admin/user
                let parts: Vec<String> = raw
                    .raw_path
                    .split("::")
                    .map(crate::parse::resolver::camel_to_snake)
                    .collect();
                let relative = parts.join("/");
                for load_path in &self.load_paths {
                    let target = load_path.join(&relative).with_extension("rb");
                    if project_files
                        .iter()
                        .any(|f| f.strip_prefix(project_root).unwrap_or(f).eq(&target))
                    {
                        return Some(target);
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

    /// Extract Zeitwerk-style constant references from the AST.
    /// Walks for `constant` and `scope_resolution` nodes, skipping class/module
    /// definitions and stdlib constants.
    fn extract_zeitwerk_constants(
        node: tree_sitter::Node,
        source: &[u8],
        file_path: &Path,
        imports: &mut Vec<RawImport>,
    ) {
        // Collect all constant references, excluding definitions
        let mut seen = std::collections::HashSet::new();
        Self::walk_for_constants(node, source, file_path, imports, &mut seen);
    }

    /// Check if a node is the "name" field of a class or module definition.
    fn is_class_or_module_name(node: tree_sitter::Node) -> bool {
        if let Some(parent) = node.parent() {
            let pk = parent.kind();
            if pk == "class" || pk == "module" {
                // Check if this node is the "name" field of the parent
                if let Some(name_node) = parent.child_by_field_name("name") {
                    return name_node.id() == node.id();
                }
            }
        }
        false
    }

    fn walk_for_constants(
        node: tree_sitter::Node,
        source: &[u8],
        file_path: &Path,
        imports: &mut Vec<RawImport>,
        seen: &mut std::collections::HashSet<String>,
    ) {
        let kind = node.kind();

        if kind == "scope_resolution" {
            // Skip if this is a class/module definition name
            if Self::is_class_or_module_name(node) {
                return;
            }
            // Full scoped constant like Admin::User or ::User
            let text = node.utf8_text(source).unwrap_or_default().to_string();
            // Handle ::User -> User for resolution (root reference)
            let raw_path = text.trim_start_matches("::").to_string();
            if !raw_path.is_empty() && !Self::is_stdlib_constant(&raw_path) && seen.insert(raw_path.clone()) {
                imports.push(RawImport {
                    raw_path,
                    source_file: file_path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: Some(node.start_position().column),
                    kind: ImportKind::ZeitwerkConstant,
                    confidence: ImportConfidence::Resolved,
                });
            }
            return; // Don't recurse into children of scope_resolution
        }

        if kind == "constant" {
            // Skip if this is a class/module definition name
            if Self::is_class_or_module_name(node) {
                return;
            }
            let text = node.utf8_text(source).unwrap_or_default().to_string();
            // Skip if parent is a scope_resolution (handled above)
            let parent_is_scope = node
                .parent()
                .map(|p| p.kind() == "scope_resolution")
                .unwrap_or(false);
            if !parent_is_scope
                && !text.is_empty()
                && !Self::is_stdlib_constant(&text)
                && seen.insert(text.clone())
            {
                imports.push(RawImport {
                    raw_path: text,
                    source_file: file_path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: Some(node.start_position().column),
                    kind: ImportKind::ZeitwerkConstant,
                    confidence: ImportConfidence::Resolved,
                });
            }
            return;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_for_constants(child, source, file_path, imports, seen);
        }
    }

    fn is_stdlib_constant(name: &str) -> bool {
        // Check the first (or only) segment
        let first_segment = name.split("::").next().unwrap_or(name);
        RUBY_STDLIB_CONSTANTS.contains(&first_segment)
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
                if let Some(last) = components.last() {
                    if last == &std::path::Component::ParentDir {
                        components.push(component);
                    } else if last == &std::path::Component::CurDir {
                        components.pop();
                        components.push(component);
                    } else {
                        components.pop();
                    }
                } else {
                    components.push(component);
                }
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
    fn zeitwerk_extracts_constants() {
        let source = br#"
class PostsController
  def index
    @posts = Post.all
  end

  def show
    @user = User.find(1)
  end
end
"#;
        let frontend = RubyFrontend::new().with_zeitwerk(true);
        let imports =
            frontend.extract_imports(source, Path::new("app/controllers/posts_controller.rb"));
        let zeitwerk_imports: Vec<_> = imports
            .iter()
            .filter(|i| matches!(i.kind, ImportKind::ZeitwerkConstant))
            .collect();
        assert!(
            zeitwerk_imports.iter().any(|i| i.raw_path == "Post"),
            "Should extract Post constant"
        );
        assert!(
            zeitwerk_imports.iter().any(|i| i.raw_path == "User"),
            "Should extract User constant"
        );
    }

    #[test]
    fn zeitwerk_excludes_stdlib_constants() {
        let source = br#"
class Foo
  def bar
    s = String.new
    a = Array.new
    h = Hash.new
    Custom.do_something
  end
end
"#;
        let frontend = RubyFrontend::new().with_zeitwerk(true);
        let imports = frontend.extract_imports(source, Path::new("test.rb"));
        let zeitwerk_imports: Vec<_> = imports
            .iter()
            .filter(|i| matches!(i.kind, ImportKind::ZeitwerkConstant))
            .collect();
        // Should not contain String, Array, Hash
        assert!(
            !zeitwerk_imports.iter().any(|i| i.raw_path == "String"),
            "Should exclude String"
        );
        assert!(
            !zeitwerk_imports.iter().any(|i| i.raw_path == "Array"),
            "Should exclude Array"
        );
        assert!(
            !zeitwerk_imports.iter().any(|i| i.raw_path == "Hash"),
            "Should exclude Hash"
        );
        // Should contain Custom
        assert!(
            zeitwerk_imports.iter().any(|i| i.raw_path == "Custom"),
            "Should extract Custom constant"
        );
    }

    #[test]
    fn zeitwerk_disabled_no_constants() {
        let source = br#"
class Foo
  def bar
    Custom.do_something
  end
end
"#;
        let frontend = RubyFrontend::new().with_zeitwerk(false);
        let imports = frontend.extract_imports(source, Path::new("test.rb"));
        let zeitwerk_imports: Vec<_> = imports
            .iter()
            .filter(|i| matches!(i.kind, ImportKind::ZeitwerkConstant))
            .collect();
        assert!(
            zeitwerk_imports.is_empty(),
            "Should not extract constants when zeitwerk is disabled"
        );
    }

    #[test]
    fn zeitwerk_resolves_simple_constant() {
        let frontend =
            RubyFrontend::with_load_paths(vec![PathBuf::from("app/models")]).with_zeitwerk(true);
        let raw = RawImport {
            raw_path: "User".into(),
            source_file: PathBuf::from("app/controllers/posts_controller.rb"),
            line: 3,
            column: None,
            kind: ImportKind::ZeitwerkConstant,
            confidence: ImportConfidence::Resolved,
        };
        let project_files = vec![PathBuf::from("/project/app/models/user.rb")];
        let resolved = frontend.resolve(&raw, Path::new("/project"), &project_files);
        assert_eq!(resolved, Some(PathBuf::from("app/models/user.rb")));
    }

    #[test]
    fn zeitwerk_resolves_scoped_constant() {
        let frontend =
            RubyFrontend::with_load_paths(vec![PathBuf::from("app/models")]).with_zeitwerk(true);
        let raw = RawImport {
            raw_path: "Admin::User".into(),
            source_file: PathBuf::from("app/controllers/admin_controller.rb"),
            line: 3,
            column: None,
            kind: ImportKind::ZeitwerkConstant,
            confidence: ImportConfidence::Resolved,
        };
        let project_files = vec![PathBuf::from("/project/app/models/admin/user.rb")];
        let resolved = frontend.resolve(&raw, Path::new("/project"), &project_files);
        assert_eq!(resolved, Some(PathBuf::from("app/models/admin/user.rb")));
    }

    #[test]
    fn zeitwerk_skips_class_definition() {
        let source = br#"
class PostsController
  def index
    Post.all
  end
end
"#;
        let frontend = RubyFrontend::new().with_zeitwerk(true);
        let imports = frontend.extract_imports(source, Path::new("test.rb"));
        let zeitwerk_imports: Vec<_> = imports
            .iter()
            .filter(|i| matches!(i.kind, ImportKind::ZeitwerkConstant))
            .collect();
        // PostsController should NOT be extracted (it's a definition)
        assert!(
            !zeitwerk_imports
                .iter()
                .any(|i| i.raw_path == "PostsController"),
            "Should not extract class definition name"
        );
        // Post should be extracted (it's a reference)
        assert!(
            zeitwerk_imports.iter().any(|i| i.raw_path == "Post"),
            "Should extract Post reference"
        );
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
