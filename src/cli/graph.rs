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

    // Read go.mod and Cargo.toml if relevant languages are present
    let go_module_path = if langs.contains(&Language::Go) {
        GoFrontend::read_go_mod(&root)
    } else {
        None
    };
    let rust_crate_name = if langs.contains(&Language::Rust) {
        RustFrontend::read_cargo_toml(&root)
    } else {
        None
    };

    // Create resolvers per language
    let resolvers: HashMap<Language, Box<dyn ParseFrontend>> = langs
        .iter()
        .map(|&lang| {
            let frontend =
                factory::create_frontend(lang, &config, &go_module_path, &rust_crate_name);
            (lang, frontend)
        })
        .collect();

    let mut builder = GraphBuilder::new();

    for (lang, file_path) in &all_files {
        let source = match std::fs::read(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let frontend = factory::create_frontend(*lang, &config, &go_module_path, &rust_crate_name);
        let imports = frontend.extract_imports(&source, file_path);
        let source_module = factory::source_module_path(file_path, &root, *lang);

        let lang_files = files_by_lang.get(lang).map(|v| v.as_slice()).unwrap_or(&[]);
        let resolver = resolvers.get(lang).unwrap();

        for raw in &imports {
            if raw.confidence == ImportConfidence::External
                || raw.confidence == ImportConfidence::Dynamic
                || raw.confidence == ImportConfidence::Unresolvable
            {
                continue;
            }

            if let Some(target) = resolver.resolve(raw, &root, lang_files) {
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
