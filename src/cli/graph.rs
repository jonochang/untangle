use crate::config::resolve::{resolve_config, CliOverrides};
use crate::errors::{Result, UntangleError};
use crate::graph::builder::{GraphBuilder, ResolvedImport};
use crate::output::OutputFormat;
use crate::parse::common::{ImportConfidence, SourceLocation};
use crate::parse::factory;
use crate::parse::go::GoFrontend;
use crate::parse::rust::RustFrontend;
use crate::parse::ParseFrontend;
use crate::walk::{self, Language};
use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct GraphArgs {
    /// Path to analyze
    pub path: PathBuf,

    /// Language to analyze
    #[arg(long, value_parser = parse_language)]
    pub lang: Option<Language>,

    /// Output format (json or dot)
    #[arg(long)]
    pub format: Option<OutputFormat>,

    /// Include test files
    #[arg(long)]
    pub include_tests: bool,

    /// Include glob patterns
    #[arg(long)]
    pub include: Vec<String>,

    /// Exclude glob patterns
    #[arg(long)]
    pub exclude: Vec<String>,

    /// Suppress progress output
    #[arg(long)]
    pub quiet: bool,
}

impl GraphArgs {
    fn to_cli_overrides(&self) -> CliOverrides {
        CliOverrides {
            lang: self.lang,
            format: self.format,
            quiet: self.quiet,
            include_tests: self.include_tests,
            include: self.include.clone(),
            exclude: self.exclude.clone(),
            ..Default::default()
        }
    }
}

fn parse_language(s: &str) -> std::result::Result<Language, String> {
    s.parse()
}

pub fn run(args: &GraphArgs) -> Result<()> {
    let root = args
        .path
        .canonicalize()
        .map_err(|_| UntangleError::NoFiles {
            path: args.path.clone(),
        })?;

    // Resolve config
    let overrides = args.to_cli_overrides();
    let config = resolve_config(&root, &overrides)?;

    let format: OutputFormat = config.format.parse().unwrap_or(OutputFormat::Dot);

    // Merge ignore_patterns into exclude list
    let mut exclude = config.exclude.clone();
    exclude.extend(config.ignore_patterns.iter().cloned());

    // Determine languages and discover files
    let (langs, files_by_lang): (Vec<Language>, HashMap<Language, Vec<PathBuf>>) = match config.lang
    {
        Some(l) => {
            let files =
                walk::discover_files(&root, l, &config.include, &exclude, config.include_tests)?;
            let mut map = HashMap::new();
            if !files.is_empty() {
                map.insert(l, files);
            }
            (vec![l], map)
        }
        None => {
            let map =
                walk::discover_files_multi(&root, &config.include, &exclude, config.include_tests)?;
            let mut langs: Vec<Language> = map.keys().copied().collect();
            langs.sort_by(|a, b| {
                map.get(b)
                    .map(|v| v.len())
                    .unwrap_or(0)
                    .cmp(&map.get(a).map(|v| v.len()).unwrap_or(0))
            });
            (langs, map)
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

    // Discover Go modules and read Cargo.toml if relevant languages are present
    let go_modules = if langs.contains(&Language::Go) {
        walk::discover_go_modules(&root)
    } else {
        std::collections::HashMap::new()
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

    // Create resolvers per language (non-Go)
    let resolvers: HashMap<Language, Box<dyn ParseFrontend>> = langs
        .iter()
        .filter(|&&lang| lang != Language::Go)
        .map(|&lang| {
            let frontend =
                factory::create_frontend(lang, &config, &go_module_path, &rust_crate_name);
            (lang, frontend)
        })
        .collect();

    // Per-module Go resolvers
    let go_resolvers: HashMap<PathBuf, Box<dyn ParseFrontend>> = go_modules
        .iter()
        .map(|(mod_root, mod_path)| {
            let fe = GoFrontend::with_module_path(mod_path.clone())
                .with_exclude_stdlib(config.go.exclude_stdlib);
            (mod_root.clone(), Box::new(fe) as Box<dyn ParseFrontend>)
        })
        .collect();
    let fallback_go_resolver: Box<dyn ParseFrontend> =
        factory::create_frontend(Language::Go, &config, &go_module_path, &rust_crate_name);

    // Partition Go files by module root
    let go_files_by_module: HashMap<PathBuf, Vec<PathBuf>> = if langs.contains(&Language::Go) {
        let mut by_module: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        if let Some(go_files) = files_by_lang.get(&Language::Go) {
            for f in go_files {
                let mod_root = walk::find_go_module_root(f, &go_modules)
                    .map(|(r, _)| r.to_path_buf())
                    .unwrap_or_else(|| root.clone());
                by_module.entry(mod_root).or_default().push(f.clone());
            }
        }
        by_module
    } else {
        HashMap::new()
    };

    let mut builder = GraphBuilder::new();

    for (lang, file_path) in &all_files {
        let source = match std::fs::read(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // For Go files, use per-module frontend
        let file_go_module = if *lang == Language::Go {
            walk::find_go_module_root(file_path, &go_modules)
                .map(|(_, mp)| mp.to_string())
                .or_else(|| go_module_path.clone())
        } else {
            go_module_path.clone()
        };

        let frontend = factory::create_frontend(*lang, &config, &file_go_module, &rust_crate_name);
        let imports = frontend.extract_imports(&source, file_path);
        let source_module = factory::source_module_path(file_path, &root, *lang);

        // Get appropriate resolver and file list
        let (resolver, lang_files): (&dyn ParseFrontend, Vec<PathBuf>) = if *lang == Language::Go {
            let mod_root = walk::find_go_module_root(file_path, &go_modules)
                .map(|(r, _)| r.to_path_buf())
                .unwrap_or_else(|| root.clone());
            let resolver = go_resolvers
                .get(&mod_root)
                .map(|r| r.as_ref())
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
            if raw.confidence == ImportConfidence::External
                || raw.confidence == ImportConfidence::Dynamic
                || raw.confidence == ImportConfidence::Unresolvable
            {
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

    let graph = builder.build();
    let mut stdout = std::io::stdout();

    match format {
        OutputFormat::Dot => {
            crate::output::dot::write_dot(&mut stdout, &graph)?;
        }
        OutputFormat::Json => {
            // Serialize graph as JSON
            let nodes: Vec<_> = graph.node_indices().map(|i| &graph[i]).collect();
            let edges: Vec<_> = graph
                .edge_indices()
                .map(|e| {
                    let (s, t) = graph.edge_endpoints(e).unwrap();
                    serde_json::json!({
                        "from": graph[s].name,
                        "to": graph[t].name,
                        "source_locations": graph[e].source_locations,
                    })
                })
                .collect();
            let output = serde_json::json!({
                "nodes": nodes,
                "edges": edges,
            });
            serde_json::to_writer_pretty(&mut stdout, &output)?;
        }
        _ => {
            crate::output::dot::write_dot(&mut stdout, &graph)?;
        }
    }

    Ok(())
}
