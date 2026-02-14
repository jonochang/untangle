use miette::Diagnostic;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum UntangleError {
    #[error("No parseable files found in {path}")]
    #[diagnostic(code(untangle::no_files))]
    NoFiles { path: PathBuf },

    #[error("Could not resolve ref: {reference}")]
    #[diagnostic(code(untangle::bad_ref))]
    BadRef { reference: String },

    #[error("Parse error in {file}: {message}")]
    #[diagnostic(code(untangle::parse_error))]
    ParseError { file: PathBuf, message: String },

    #[error("Configuration error: {0}")]
    #[diagnostic(code(untangle::config))]
    Config(String),

    #[error(transparent)]
    #[diagnostic(code(untangle::io))]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    #[diagnostic(code(untangle::git))]
    Git(#[from] git2::Error),

    #[error(transparent)]
    #[diagnostic(code(untangle::json))]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    #[diagnostic(code(untangle::glob))]
    Glob(#[from] globset::Error),
}

pub type Result<T> = std::result::Result<T, UntangleError>;
