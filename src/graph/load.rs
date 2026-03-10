use crate::analysis_context::build_analysis_context;
use crate::config::ResolvedConfig;
use crate::errors::Result;
use crate::graph::builder::{GraphBuilder, ResolvedImport};
use crate::graph::ir::DepGraph;
use crate::parse::common::{ImportConfidence, SourceLocation};
use crate::parse::factory;
use crate::parse::go::GoFrontend;
use crate::parse::ParseFrontend;
use crate::walk::{self, Language};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn load_dependency_graph(
    scan_root: &Path,
    project_root: &Path,
    config: &ResolvedConfig,
) -> Result<DepGraph> {
    let context = build_analysis_context(scan_root, project_root, config)?;

    let resolvers: HashMap<Language, Box<dyn ParseFrontend>> = context
        .langs
        .iter()
        .filter(|&&lang| lang != Language::Go)
        .map(|&lang| {
            let frontend = factory::create_frontend(
                lang,
                config,
                &context.go_module_path,
                &context.rust_crate_name,
            );
            (lang, frontend)
        })
        .collect();

    let go_resolvers: HashMap<PathBuf, Box<dyn ParseFrontend>> = context
        .go_modules
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
    let fallback_go_resolver: Box<dyn ParseFrontend> = factory::create_frontend(
        Language::Go,
        config,
        &context.go_module_path,
        &context.rust_crate_name,
    );

    let go_files_by_module: HashMap<PathBuf, Vec<PathBuf>> =
        if context.langs.contains(&Language::Go) {
            let mut by_module: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
            if let Some(go_files) = context.files_by_lang.get(&Language::Go) {
                for file in go_files {
                    let mod_root = walk::find_go_module_root(file, &context.go_modules)
                        .map(|(root, _)| root.to_path_buf())
                        .unwrap_or_else(|| context.project_root.clone());
                    by_module.entry(mod_root).or_default().push(file.clone());
                }
            }
            by_module
        } else {
            HashMap::new()
        };

    let mut builder = GraphBuilder::new();

    for (lang, file_path) in &context.all_files {
        let source = match std::fs::read(file_path) {
            Ok(source) => source,
            Err(_) => continue,
        };

        let file_go_module = if *lang == Language::Go {
            walk::find_go_module_root(file_path, &context.go_modules)
                .map(|(_, module_path)| module_path.to_string())
                .or_else(|| context.go_module_path.clone())
        } else {
            context.go_module_path.clone()
        };

        let frontend =
            factory::create_frontend(*lang, config, &file_go_module, &context.rust_crate_name);
        let imports = frontend.extract_imports(&source, file_path);
        let source_module = factory::source_module_path(file_path, &context.project_root, *lang);

        let (resolver, lang_files): (&dyn ParseFrontend, Vec<PathBuf>) = if *lang == Language::Go {
            let mod_root = walk::find_go_module_root(file_path, &context.go_modules)
                .map(|(root, _)| root.to_path_buf())
                .unwrap_or_else(|| context.project_root.clone());
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
            let files = context.files_by_lang.get(lang).cloned().unwrap_or_default();
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

            if let Some(target) = resolver.resolve(raw, &context.project_root, &lang_files) {
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
