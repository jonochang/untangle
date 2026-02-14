pub mod dot;
pub mod json;
pub mod sarif;
pub mod text;

use clap::ValueEnum;
use serde::Serialize;

#[derive(Debug, Default, Clone, Copy, ValueEnum, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Json,
    Text,
    Dot,
    Sarif,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(OutputFormat::Json),
            "text" => Ok(OutputFormat::Text),
            "dot" => Ok(OutputFormat::Dot),
            "sarif" => Ok(OutputFormat::Sarif),
            _ => Err(format!("unknown output format: {s}")),
        }
    }
}
