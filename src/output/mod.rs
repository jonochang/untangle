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
