use serde::Serialize;
use std::path::PathBuf;

/// Raw import extracted from a single source file.
#[derive(Debug, Clone)]
pub struct RawImport {
    /// The import path as written in source
    pub raw_path: String,
    /// The source file containing the import
    pub source_file: PathBuf,
    /// Line number of the import statement (1-indexed)
    pub line: usize,
    /// Column (optional, for SARIF precision)
    pub column: Option<usize>,
    /// Classification of the import
    pub kind: ImportKind,
    /// Parser confidence
    pub confidence: ImportConfidence,
}

#[derive(Debug, Clone)]
pub enum ImportKind {
    /// `import foo` / `require "foo"` / `import "foo"`
    Direct,
    /// `from foo import bar` (Python)
    FromImport { module: String, names: Vec<String> },
    /// `from . import foo` (Python)
    RelativeImport {
        level: usize,
        module: Option<String>,
        names: Vec<String>,
    },
    /// `require_relative "./foo"` (Ruby)
    RequireRelative,
    /// `autoload :Foo, "path"` (Ruby)
    Autoload { constant: String },
    /// Ruby constant reference resolved via Zeitwerk convention (CamelCase → snake_case)
    ZeitwerkConstant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportConfidence {
    /// Fully resolved to a project-internal target
    Resolved,
    /// Likely external (third-party / stdlib)
    External,
    /// Contains dynamic component — unresolvable
    Dynamic,
    /// String interpolation or metaprogramming
    Unresolvable,
}

/// Source location for edge provenance
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
pub struct SourceLocation {
    pub file: PathBuf,
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
}
