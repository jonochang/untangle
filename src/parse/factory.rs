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
    rust_crate_name: &Option<String>,
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
        Language::Ruby => Box::new(RubyFrontend::with_load_paths(config.ruby_load_paths())),
        Language::Rust => Box::new(match rust_crate_name {
            Some(name) => RustFrontend::with_crate_name(name.clone()),
            None => RustFrontend::new(),
        }),
    }
}

/// Compute the source module path for Go (package-level) vs other languages (file-level).
pub fn source_module_path(
    file_path: &std::path::Path,
    root: &std::path::Path,
    lang: Language,
) -> PathBuf {
    let relative = file_path.strip_prefix(root).unwrap_or(file_path);
    if lang == Language::Go {
        relative.parent().unwrap_or(relative).to_path_buf()
    } else {
        relative.to_path_buf()
    }
}
