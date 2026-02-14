use crate::errors::{Result, UntangleError};
use crate::graph::builder::{GraphBuilder, ResolvedImport};
use crate::output::OutputFormat;
use crate::parse::common::{ImportConfidence, SourceLocation};
use crate::parse::{
    go::GoFrontend, python::PythonFrontend, ruby::RubyFrontend, rust::RustFrontend, ParseFrontend,
};
use crate::walk::{self, Language};
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct GraphArgs {
    /// Path to analyze
    pub path: PathBuf,

    /// Language to analyze
    #[arg(long, value_parser = parse_language)]
    pub lang: Option<Language>,

    /// Output format (json or dot)
    #[arg(long, default_value = "dot")]
    pub format: OutputFormat,

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

    let lang = match args.lang {
        Some(l) => l,
        None => walk::detect_language(&root)
            .ok_or_else(|| UntangleError::NoFiles { path: root.clone() })?,
    };

    let files = walk::discover_files(
        &root,
        lang,
        &args.include,
        &args.exclude,
        args.include_tests,
    )?;

    if files.is_empty() {
        return Err(UntangleError::NoFiles { path: root });
    }

    let frontend: Box<dyn ParseFrontend> = match lang {
        Language::Go => {
            let module_path = GoFrontend::read_go_mod(&root);
            Box::new(match module_path {
                Some(mp) => GoFrontend::with_module_path(mp),
                None => GoFrontend::new(),
            })
        }
        Language::Python => Box::new(PythonFrontend::new()),
        Language::Ruby => Box::new(RubyFrontend::new()),
        Language::Rust => {
            let crate_name = RustFrontend::read_cargo_toml(&root);
            Box::new(match crate_name {
                Some(name) => RustFrontend::with_crate_name(name),
                None => RustFrontend::new(),
            })
        }
    };

    let mut builder = GraphBuilder::new();
    let project_files = files.clone();

    for file_path in &files {
        let source = match std::fs::read(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let imports = frontend.extract_imports(&source, file_path);
        let source_module = file_path
            .strip_prefix(&root)
            .unwrap_or(file_path)
            .to_path_buf();

        for raw in &imports {
            if raw.confidence == ImportConfidence::External
                || raw.confidence == ImportConfidence::Dynamic
                || raw.confidence == ImportConfidence::Unresolvable
            {
                continue;
            }

            if let Some(target) = frontend.resolve(raw, &root, &project_files) {
                builder.add_import(&ResolvedImport {
                    source_module: source_module.clone(),
                    target_module: target,
                    location: SourceLocation {
                        file: source_module.clone(),
                        line: raw.line,
                        column: raw.column,
                    },
                });
            }
        }
    }

    let graph = builder.build();
    let mut stdout = std::io::stdout();

    match args.format {
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
