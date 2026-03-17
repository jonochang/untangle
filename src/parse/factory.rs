use crate::analysis_context::RustWorkspaceContext;
use crate::config::ResolvedConfig;
use crate::parse::go::GoFrontend;
use crate::parse::python::PythonFrontend;
use crate::parse::ruby::RubyFrontend;
use crate::parse::rust::RustFrontend;
use crate::parse::ParseFrontend;
use crate::walk::Language;
use std::path::PathBuf;

/// Create a ParseFrontend for a given language and config.
pub fn create_frontend(
    lang: Language,
    config: &ResolvedConfig,
    go_module_path: &Option<String>,
    rust_workspace: &Option<RustWorkspaceContext>,
) -> Box<dyn ParseFrontend> {
    match lang {
        Language::Go => {
            let fe = match go_module_path {
                Some(mp) => GoFrontend::with_module_path(mp.clone()),
                None => GoFrontend::new(),
            };
            Box::new(fe.with_exclude_stdlib(config.go.exclude_stdlib))
        }
        Language::Python => Box::new(PythonFrontend::new()),
        Language::Ruby => Box::new(
            RubyFrontend::with_load_paths(config.ruby_load_paths())
                .with_zeitwerk(config.ruby.zeitwerk),
        ),
        Language::Rust => Box::new(match rust_workspace {
            Some(workspace) => RustFrontend::with_workspace(workspace.clone()),
            None => RustFrontend::new(),
        }),
    }
}

/// Compute the source module path for Go (package-level) vs other languages (file-level).
pub fn source_module_path(
    file_path: &std::path::Path,
    root: &std::path::Path,
    lang: Language,
    rust_workspace: Option<&RustWorkspaceContext>,
) -> PathBuf {
    if lang == Language::Rust {
        if let Some(workspace) = rust_workspace {
            if let Some(package) = workspace.find_package_for_file(file_path, root) {
                if let Some(module_id) = package.module_id_for_file(file_path) {
                    return module_id;
                }
            }
        }
    }

    let relative = file_path.strip_prefix(root).unwrap_or(file_path);
    if lang == Language::Go {
        relative.parent().unwrap_or(relative).to_path_buf()
    } else {
        relative.to_path_buf()
    }
}
