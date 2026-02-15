pub mod common;
pub mod factory;
pub mod go;
pub mod graphql;
pub mod graphql_client;
pub mod openapi;
pub mod python;
pub mod resolver;
pub mod rest_client;
pub mod ruby;
pub mod rust;

pub use common::RawImport;

use std::path::Path;

/// Parser frontend trait — each language implements this.
pub trait ParseFrontend {
    /// Return the tree-sitter Language for this frontend.
    fn language(&self) -> tree_sitter::Language;

    /// Extract raw imports from a single file's source bytes.
    fn extract_imports(&self, source: &[u8], file_path: &Path) -> Vec<RawImport>;

    /// Resolve a raw import to a canonical project-internal module path.
    /// Returns None if the import is external/unresolvable.
    fn resolve(
        &self,
        raw: &RawImport,
        project_root: &Path,
        project_files: &[std::path::PathBuf],
    ) -> Option<std::path::PathBuf>;
}
