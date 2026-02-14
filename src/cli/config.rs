use crate::config::resolve::{resolve_config, CliOverrides};
use crate::errors::Result;
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Show resolved configuration with provenance
    Show {
        /// Working directory (defaults to current directory)
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Explain where a specific rule's thresholds come from
    Explain {
        /// Rule category to explain (e.g., high_fanout, god_module)
        category: String,
        /// Working directory (defaults to current directory)
        #[arg(long)]
        path: Option<PathBuf>,
    },
}

pub fn run(args: &ConfigArgs) -> Result<()> {
    match &args.action {
        ConfigAction::Show { path } => {
            let working_dir = resolve_working_dir(path)?;
            let config = resolve_config(&working_dir, &CliOverrides::default())?;
            let mut stdout = std::io::stdout();
            crate::config::show::render_show(&mut stdout, &config)
                .map_err(crate::errors::UntangleError::Io)?;
        }
        ConfigAction::Explain { category, path } => {
            let working_dir = resolve_working_dir(path)?;
            let config = resolve_config(&working_dir, &CliOverrides::default())?;
            let mut stdout = std::io::stdout();
            crate::config::show::render_explain(&mut stdout, &config, category)
                .map_err(crate::errors::UntangleError::Io)?;
        }
    }
    Ok(())
}

fn resolve_working_dir(path: &Option<PathBuf>) -> Result<PathBuf> {
    let p = path.clone().unwrap_or_else(|| PathBuf::from("."));
    p.canonicalize()
        .map_err(|_| crate::errors::UntangleError::Config(format!("Invalid path: {}", p.display())))
}
