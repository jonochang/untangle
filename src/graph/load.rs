use crate::config::ResolvedConfig;
use crate::errors::{Result, UntangleError};
use crate::graph::builder::{GraphBuilder, ResolvedImport};
use crate::graph::ir::DepGraph;
use crate::parse::common::{ImportConfidence, SourceLocation};
use crate::parse::factory;
use crate::parse::go::GoFrontend;
use crate::parse::rust::RustFrontend;
use crate::parse::ParseFrontend;
use crate::walk::{self, Language};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn load_dependency_graph(root: &Path, config: &ResolvedConfig) -> Result<DepGraph> {
    let root = root.canonicalize().map_err(|_| UntangleError::NoFiles {
        path: root.to_path_buf(),
    })?;

    let mut exclude = config.exclude.clone();
    exclude.extend(config.ignore_patterns.iter().cloned());

    let (langs, files_by_lang): (Vec<Language>, HashMap<Language, Vec<PathBuf>>) = match config.lang
    {
        Some(language) => {
            let files = walk::discover_files(
                &root,
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
            let files_by_lang =
                walk::discover_files_multi(&root, &config.include, &exclude, config.include_tests)?;
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

    let all_files: Vec<(Language, PathBuf)> = langs
        .iter()
        .flat_map(|&lang| {
            files_by_lang
                .get(&lang)
                .map(|files| files.iter().map(move |f| (lang, f.clone())))
                .into_iter()
                .flatten()
        })
        .collect();

    if all_files.is_empty() {
        return Err(UntangleError::NoFiles { path: root });
    }

    let go_modules = if langs.contains(&Language::Go) {
        walk::discover_go_modules(&root)
    } else {
        HashMap::new()
    };
    let go_module_path = go_modules.get(&root).cloned().or_else(|| {
        if langs.contains(&Language::Go) {
            GoFrontend::read_go_mod(&root)
        } else {
            None
        }
    });
    let rust_crate_name = if langs.contains(&Language::Rust) {
        RustFrontend::read_cargo_toml(&root)
    } else {
        None
    };

    let resolvers: HashMap<Language, Box<dyn ParseFrontend>> = langs
        .iter()
        .filter(|&&lang| lang != Language::Go)
        .map(|&lang| {
            let frontend =
                factory::create_frontend(lang, config, &go_module_path, &rust_crate_name);
            (lang, frontend)
        })
        .collect();

    let go_resolvers: HashMap<PathBuf, Box<dyn ParseFrontend>> = go_modules
        .iter()
        .map(|(mod_root, mod_path)| {
            let frontend = GoFrontend::with_module_path(mod_path.clone())
                .with_exclude_stdlib(config.go.exclude_stdlib);
            (
                mod_root.clone(),
                Box::new(frontend) as Box<dyn ParseFrontend>,
            )
        })
        .collect();
    let fallback_go_resolver: Box<dyn ParseFrontend> =
        factory::create_frontend(Language::Go, config, &go_module_path, &rust_crate_name);

    let go_files_by_module: HashMap<PathBuf, Vec<PathBuf>> = if langs.contains(&Language::Go) {
        let mut by_module: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        if let Some(go_files) = files_by_lang.get(&Language::Go) {
            for file in go_files {
                let mod_root = walk::find_go_module_root(file, &go_modules)
                    .map(|(root, _)| root.to_path_buf())
                    .unwrap_or_else(|| root.clone());
                by_module.entry(mod_root).or_default().push(file.clone());
            }
        }
        by_module
    } else {
        HashMap::new()
    };

    let mut builder = GraphBuilder::new();

    for (lang, file_path) in &all_files {
        let source = match std::fs::read(file_path) {
            Ok(source) => source,
            Err(_) => continue,
        };

        let file_go_module = if *lang == Language::Go {
            walk::find_go_module_root(file_path, &go_modules)
                .map(|(_, module_path)| module_path.to_string())
                .or_else(|| go_module_path.clone())
        } else {
            go_module_path.clone()
        };

        let frontend = factory::create_frontend(*lang, config, &file_go_module, &rust_crate_name);
        let imports = frontend.extract_imports(&source, file_path);
        let source_module = factory::source_module_path(file_path, &root, *lang);

        let (resolver, lang_files): (&dyn ParseFrontend, Vec<PathBuf>) = if *lang == Language::Go {
            let mod_root = walk::find_go_module_root(file_path, &go_modules)
                .map(|(root, _)| root.to_path_buf())
                .unwrap_or_else(|| root.clone());
            let resolver = go_resolvers
                .get(&mod_root)
                .map(|resolver| resolver.as_ref())
                .unwrap_or(fallback_go_resolver.as_ref());
            let files = go_files_by_module
                .get(&mod_root)
                .cloned()
                .unwrap_or_default();
            (resolver, files)
        } else {
            let resolver = resolvers.get(lang).unwrap().as_ref();
            let files = files_by_lang.get(lang).cloned().unwrap_or_default();
            (resolver, files)
        };

        for raw in &imports {
            if matches!(
                raw.confidence,
                ImportConfidence::External
                    | ImportConfidence::Dynamic
                    | ImportConfidence::Unresolvable
            ) {
                continue;
            }

            if let Some(target) = resolver.resolve(raw, &root, &lang_files) {
                builder.add_import(&ResolvedImport {
                    source_module: source_module.clone(),
                    target_module: target,
                    location: SourceLocation {
                        file: source_module.clone(),
                        line: raw.line,
                        column: raw.column,
                    },
                    language: Some(*lang),
                });
            }
        }
    }

    Ok(builder.build())
}
