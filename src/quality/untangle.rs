use crate::analysis_context::build_analysis_context;
use crate::config::ResolvedConfig;
use crate::errors::Result;
use crate::graph::builder::{GraphBuilder, ResolvedImport};
use crate::metrics::scc;
use crate::metrics::summary::Summary;
use crate::parse::common::{ImportConfidence, RawImport, SourceLocation};
use crate::parse::factory;
use crate::parse::go::GoFrontend;
use crate::parse::ParseFrontend;
use crate::quality::{UntangleHotspot, UntangleMetricSummary};
use crate::walk::{self, Language};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

struct FileParseResult {
    source_module: PathBuf,
    imports: Vec<RawImport>,
    language: Language,
    original_file: PathBuf,
}

pub fn compute_untangle_summary(
    root: &Path,
    config: &ResolvedConfig,
    hotspot_limit: usize,
) -> Result<UntangleMetricSummary> {
    let context = build_analysis_context(root, root, config)?;
    let mut parse_results: Vec<FileParseResult> = Vec::new();

    for (lang, files) in &context.files_by_lang {
        for file_path in files {
            let Ok(source) = std::fs::read(file_path) else {
                continue;
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
            let source_module =
                factory::source_module_path(file_path, &context.project_root, *lang);

            parse_results.push(FileParseResult {
                source_module,
                imports,
                language: *lang,
                original_file: file_path.clone(),
            });
        }
    }

    let mut builder = GraphBuilder::new();
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

    let fallback_go_resolver = factory::create_frontend(
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

    let files_by_lang_for_resolve: HashMap<Language, Vec<PathBuf>> = context
        .files_by_lang
        .iter()
        .map(|(&lang, files)| (lang, files.clone()))
        .collect();

    for result in &parse_results {
        let (resolver, lang_files): (&dyn ParseFrontend, Vec<PathBuf>) = if result.language
            == Language::Go
        {
            let mod_root = walk::find_go_module_root(&result.original_file, &context.go_modules)
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
            let resolver = match resolvers.get(&result.language) {
                Some(resolver) => resolver.as_ref(),
                None => continue,
            };
            let files = files_by_lang_for_resolve
                .get(&result.language)
                .cloned()
                .unwrap_or_default();
            (resolver, files)
        };

        for raw in &result.imports {
            if matches!(
                raw.confidence,
                ImportConfidence::External
                    | ImportConfidence::Dynamic
                    | ImportConfidence::Unresolvable
            ) {
                continue;
            }

            if let Some(target_module) = resolver.resolve(raw, &context.project_root, &lang_files) {
                builder.add_import(&ResolvedImport {
                    source_module: result.source_module.clone(),
                    target_module,
                    location: SourceLocation {
                        file: result.source_module.clone(),
                        line: raw.line,
                        column: raw.column,
                    },
                    language: Some(result.language),
                });
            }
        }
    }

    let graph = builder.build();
    let summary = Summary::from_graph(&graph);
    let scc_map = scc::node_scc_map(&graph);

    let mut nodes: Vec<_> = graph
        .node_indices()
        .map(|idx| {
            let fanout = crate::metrics::fanout::fan_out(&graph, idx);
            let fanin = crate::metrics::fanout::fan_in(&graph, idx);
            (idx, fanout, fanin)
        })
        .collect();
    nodes.sort_by(|a, b| b.1.cmp(&a.1));

    let hotspots = nodes
        .iter()
        .take(hotspot_limit.min(nodes.len()))
        .map(|&(idx, fanout, fanin)| UntangleHotspot {
            module: graph[idx].name.clone(),
            path: graph[idx].path.clone(),
            fanout,
            fanin,
            scc: scc_map.get(&idx).copied(),
        })
        .collect();

    let node_count = graph.node_count();
    let edge_count = graph.edge_count();
    let edge_density = if node_count > 1 {
        edge_count as f64 / (node_count as f64 * (node_count as f64 - 1.0))
    } else {
        0.0
    };

    Ok(UntangleMetricSummary {
        nodes: node_count,
        edges: edge_count,
        edge_density: (edge_density * 10000.0).round() / 10000.0,
        files_parsed: parse_results.len(),
        summary,
        hotspots,
    })
}
