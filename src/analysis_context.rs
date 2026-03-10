use crate::config::ResolvedConfig;
use crate::errors::{Result, UntangleError};
use crate::parse::go::GoFrontend;
use crate::parse::rust::RustFrontend;
use crate::walk::{self, Language};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct AnalysisContext {
    pub scan_root: PathBuf,
    pub project_root: PathBuf,
    pub langs: Vec<Language>,
    pub files_by_lang: HashMap<Language, Vec<PathBuf>>,
    pub all_files: Vec<(Language, PathBuf)>,
    pub go_modules: HashMap<PathBuf, String>,
    pub go_module_path: Option<String>,
    pub rust_crate_name: Option<String>,
}

pub fn canonicalize_root(path: &Path) -> Result<PathBuf> {
    path.canonicalize().map_err(|_| UntangleError::NoFiles {
        path: path.to_path_buf(),
    })
}

pub fn resolve_project_root(scan_root: &Path, lang: Option<Language>) -> PathBuf {
    match lang {
        Some(Language::Rust) => {
            find_manifest_root(scan_root, "Cargo.toml").unwrap_or_else(|| scan_root.to_path_buf())
        }
        Some(Language::Go) => {
            find_manifest_root(scan_root, "go.mod").unwrap_or_else(|| scan_root.to_path_buf())
        }
        _ => scan_root.to_path_buf(),
    }
}

pub fn build_analysis_context(
    scan_root: &Path,
    project_root: &Path,
    config: &ResolvedConfig,
) -> Result<AnalysisContext> {
    let mut exclude = config.exclude.clone();
    exclude.extend(config.ignore_patterns.iter().cloned());

    let (langs, files_by_lang): (Vec<Language>, HashMap<Language, Vec<PathBuf>>) = match config.lang
    {
        Some(language) => {
            let files = walk::discover_files(
                scan_root,
                language,
                &config.include,
                &exclude,
                config.include_tests,
            )?;
            let mut files_by_lang = HashMap::new();
            if !files.is_empty() {
                files_by_lang.insert(language, files);
            }
            (vec![language], files_by_lang)
        }
        None => {
            let files_by_lang = walk::discover_files_multi(
                scan_root,
                &config.include,
                &exclude,
                config.include_tests,
            )?;
            let mut langs: Vec<Language> = files_by_lang.keys().copied().collect();
            langs.sort_by(|a, b| {
                files_by_lang
                    .get(b)
                    .map(|v| v.len())
                    .unwrap_or(0)
                    .cmp(&files_by_lang.get(a).map(|v| v.len()).unwrap_or(0))
            });
            (langs, files_by_lang)
        }
    };

    let project_root = if config.lang.is_none() && langs.len() == 1 {
        resolve_project_root(project_root, langs.first().copied())
    } else {
        project_root.to_path_buf()
    };

    let all_files: Vec<(Language, PathBuf)> = langs
        .iter()
        .flat_map(|&lang| {
            files_by_lang
                .get(&lang)
                .into_iter()
                .flat_map(move |files| files.iter().cloned().map(move |file| (lang, file)))
        })
        .collect();

    if all_files.is_empty() {
        return Err(UntangleError::NoFiles {
            path: scan_root.to_path_buf(),
        });
    }

    let go_modules = if langs.contains(&Language::Go) {
        walk::discover_go_modules(&project_root)
    } else {
        HashMap::new()
    };

    let go_module_path = go_modules.get(&project_root).cloned().or_else(|| {
        if langs.contains(&Language::Go) {
            GoFrontend::read_go_mod(&project_root)
        } else {
            None
        }
    });

    let rust_crate_name = if langs.contains(&Language::Rust) {
        let rust_root = find_manifest_root(scan_root, "Cargo.toml").unwrap_or_else(|| {
            find_manifest_root(&project_root, "Cargo.toml").unwrap_or_else(|| project_root.clone())
        });
        RustFrontend::read_cargo_toml(&rust_root)
    } else {
        None
    };

    Ok(AnalysisContext {
        scan_root: scan_root.to_path_buf(),
        project_root,
        langs,
        files_by_lang,
        all_files,
        go_modules,
        go_module_path,
        rust_crate_name,
    })
}

fn find_manifest_root(start: &Path, manifest: &str) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        if dir.join(manifest).exists() {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }

    None
}
