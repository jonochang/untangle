use crate::config::ResolvedConfig;
use crate::errors::{Result, UntangleError};
use crate::parse::go::GoFrontend;
use crate::parse::rust::RustFrontend;
use crate::walk::{self, Language};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct AnalysisContext {
    pub scan_root: PathBuf,
    pub project_root: PathBuf,
    pub langs: Vec<Language>,
    pub files_by_lang: HashMap<Language, Vec<PathBuf>>,
    pub all_files: Vec<(Language, PathBuf)>,
    pub go_modules: HashMap<PathBuf, String>,
    pub go_module_path: Option<String>,
    pub rust_workspace: Option<RustWorkspaceContext>,
}

#[derive(Clone, Debug)]
pub struct RustPackage {
    pub name: String,
    pub normalized_name: String,
    pub manifest_dir: PathBuf,
    pub source_roots: Vec<PathBuf>,
    pub entry_source_root: PathBuf,
}

impl RustPackage {
    pub fn source_root_for_file<'a>(&'a self, file_path: &Path) -> Option<&'a Path> {
        self.source_roots
            .iter()
            .filter(|root| file_path.starts_with(root))
            .max_by_key(|root| root.components().count())
            .map(PathBuf::as_path)
    }

    pub fn module_id_for_file(&self, file_path: &Path) -> Option<PathBuf> {
        let absolute_file = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            self.manifest_dir.join(file_path)
        };
        let relative = absolute_file.strip_prefix(&self.manifest_dir).ok()?;
        Some(PathBuf::from(&self.normalized_name).join(relative))
    }
}

#[derive(Clone, Debug, Default)]
pub struct RustWorkspaceContext {
    pub packages: Vec<RustPackage>,
    pub package_by_name: HashMap<String, RustPackage>,
}

impl RustWorkspaceContext {
    pub fn from_packages(packages: Vec<RustPackage>) -> Self {
        let package_by_name = packages
            .iter()
            .cloned()
            .map(|package| (package.normalized_name.clone(), package))
            .collect();
        Self {
            packages,
            package_by_name,
        }
    }

    pub fn find_package_for_file<'a>(
        &'a self,
        file_path: &Path,
        project_root: &Path,
    ) -> Option<&'a RustPackage> {
        let absolute_file = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            project_root.join(file_path)
        };

        self.packages
            .iter()
            .filter_map(|package| {
                package
                    .source_root_for_file(&absolute_file)
                    .map(|root| (root.components().count(), package))
            })
            .max_by_key(|(depth, _)| *depth)
            .map(|(_, package)| package)
    }

    pub fn files_for_package(
        &self,
        package: &RustPackage,
        project_root: &Path,
        project_files: &[PathBuf],
    ) -> Vec<PathBuf> {
        project_files
            .iter()
            .filter(|file| {
                let absolute_file = if file.is_absolute() {
                    (*file).clone()
                } else {
                    project_root.join(file)
                };
                package.source_root_for_file(&absolute_file).is_some()
            })
            .cloned()
            .collect()
    }
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

    let rust_workspace = if langs.contains(&Language::Rust) {
        let rust_root = find_manifest_root(scan_root, "Cargo.toml").unwrap_or_else(|| {
            find_manifest_root(&project_root, "Cargo.toml").unwrap_or_else(|| project_root.clone())
        });
        build_rust_workspace_context(&rust_root)
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
        rust_workspace,
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

fn build_rust_workspace_context(manifest_root: &Path) -> Option<RustWorkspaceContext> {
    discover_rust_workspace_with_metadata(manifest_root)
        .or_else(|| discover_single_rust_package(manifest_root))
}

fn discover_single_rust_package(manifest_root: &Path) -> Option<RustWorkspaceContext> {
    let crate_name = RustFrontend::read_cargo_toml(manifest_root)?;
    let package = RustPackage {
        normalized_name: normalize_rust_crate_name(&crate_name),
        name: crate_name,
        manifest_dir: manifest_root.to_path_buf(),
        source_roots: vec![manifest_root.join("src")],
        entry_source_root: manifest_root.join("src"),
    };
    Some(RustWorkspaceContext::from_packages(vec![package]))
}

fn discover_rust_workspace_with_metadata(manifest_root: &Path) -> Option<RustWorkspaceContext> {
    let manifest_path = manifest_root.join("Cargo.toml");
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--no-deps")
        .arg("--format-version")
        .arg("1")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .current_dir(manifest_root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let metadata: CargoMetadata = serde_json::from_slice(&output.stdout).ok()?;
    let members: std::collections::HashSet<&str> = metadata
        .workspace_members
        .iter()
        .map(String::as_str)
        .collect();

    let packages: Vec<RustPackage> = metadata
        .packages
        .into_iter()
        .filter(|package| members.contains(package.id.as_str()))
        .filter_map(|package| {
            let manifest_dir = package.manifest_path.parent()?.to_path_buf();
            let mut source_roots: Vec<PathBuf> = package
                .targets
                .iter()
                .filter_map(|target| target.src_path.parent().map(Path::to_path_buf))
                .collect();
            source_roots.sort();
            source_roots.dedup();

            let entry_source_root = package
                .targets
                .iter()
                .find(|target| target.kind.iter().any(|kind| kind == "lib"))
                .and_then(|target| target.src_path.parent().map(Path::to_path_buf))
                .or_else(|| source_roots.first().cloned())
                .unwrap_or_else(|| manifest_dir.join("src"));

            if source_roots.is_empty() {
                source_roots.push(entry_source_root.clone());
            }

            Some(RustPackage {
                normalized_name: normalize_rust_crate_name(&package.name),
                name: package.name,
                manifest_dir,
                source_roots,
                entry_source_root,
            })
        })
        .collect();

    if packages.is_empty() {
        None
    } else {
        Some(RustWorkspaceContext::from_packages(packages))
    }
}

fn normalize_rust_crate_name(crate_name: &str) -> String {
    crate_name.replace('-', "_")
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoMetadataPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadataPackage {
    id: String,
    name: String,
    manifest_path: PathBuf,
    targets: Vec<CargoMetadataTarget>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadataTarget {
    kind: Vec<String>,
    src_path: PathBuf,
}
