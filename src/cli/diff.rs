use crate::cli::common::{RuntimeArgs, TargetArgs};
use crate::config::resolve::{resolve_config, CliOverrides};
use crate::config::ResolvedArchitectureConfig;
use crate::errors::{Result, UntangleError};
use crate::formats::DiffFormat;
use crate::graph::diff::{analyze_repo_diff, DiffAnalysisRequest, FailCondition, Verdict};
use crate::walk::Language;
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct DiffArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[command(flatten)]
    pub runtime: RuntimeArgs,

    /// Base git ref
    #[arg(long)]
    pub base: String,

    /// Head git ref
    #[arg(long)]
    pub head: String,

    /// Output format
    #[arg(long)]
    pub format: Option<DiffFormat>,

    /// Fail-on conditions (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub fail_on: Vec<String>,
}

impl DiffArgs {
    fn to_cli_overrides(&self) -> CliOverrides {
        CliOverrides {
            lang: self.target.lang,
            quiet: self.runtime.quiet,
            include_tests: self.target.include_tests,
            include: self.target.include.clone(),
            exclude: self.target.exclude.clone(),
            fail_on: self.fail_on.clone(),
            ..Default::default()
        }
    }
}

pub fn run(args: &DiffArgs) -> Result<()> {
    let path = args
        .target
        .path
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    let root = path
        .canonicalize()
        .map_err(|_| UntangleError::NoFiles { path: path.clone() })?;
    let config = resolve_config(&root, &args.to_cli_overrides())?;
    let repo = crate::git::open_repo(&root)?;
    let langs = determine_languages(&root, config.lang)?;

    let mut exclude = config.exclude.clone();
    exclude.extend(config.ignore_patterns.iter().cloned());
    let conditions: Vec<FailCondition> = config
        .fail_on
        .iter()
        .filter_map(|condition| FailCondition::parse(condition))
        .collect();
    let has_architecture_policy = has_architecture_policy(&config.analyze_architecture);
    ensure_architecture_policy_available(
        &conditions,
        &config.analyze_architecture,
        has_architecture_policy,
    )?;

    let result = analyze_repo_diff(DiffAnalysisRequest {
        repo: &repo,
        root: &root,
        base_ref: &args.base,
        head_ref: &args.head,
        langs: &langs,
        include: &config.include,
        exclude: &exclude,
        include_tests: config.include_tests,
        go_exclude_stdlib: config.go.exclude_stdlib,
        ruby_load_paths: &config.ruby_load_paths(),
        ruby_zeitwerk: config.ruby.zeitwerk,
        conditions: &conditions,
        architecture_config: has_architecture_policy.then_some(&config.analyze_architecture),
    })?;

    let mut stdout = std::io::stdout();
    match args.format.unwrap_or(config.diff.format) {
        DiffFormat::Json => crate::output::json::write_diff_json(&mut stdout, &result)?,
        DiffFormat::Text => crate::output::text::write_diff_text(&mut stdout, &result)?,
    }

    if result.verdict == Verdict::Fail {
        std::process::exit(1);
    }

    Ok(())
}

fn ensure_architecture_policy_available(
    conditions: &[FailCondition],
    _config: &ResolvedArchitectureConfig,
    has_architecture_policy: bool,
) -> Result<()> {
    let needs_architecture_policy = conditions.iter().any(|condition| {
        matches!(
            condition,
            FailCondition::NewArchitectureViolation
                | FailCondition::NewArchitectureCycle
                | FailCondition::ArchitectureCycleGrowth
        )
    });
    if needs_architecture_policy && !has_architecture_policy {
        return Err(UntangleError::Config(
            "Architecture diff policies require [analyze.architecture] policy configuration"
                .to_string(),
        ));
    }

    Ok(())
}

fn has_architecture_policy(config: &ResolvedArchitectureConfig) -> bool {
    !config.allowed_dependencies.is_empty()
        || !config.forbidden_dependencies.is_empty()
        || !config.exceptions.is_empty()
        || !config.ignored_components.is_empty()
}

fn determine_languages(
    root: &std::path::Path,
    configured: Option<Language>,
) -> Result<Vec<Language>> {
    match configured {
        Some(lang) => Ok(vec![lang]),
        None => {
            let detected = crate::walk::detect_languages(root);
            if detected.is_empty() {
                Err(UntangleError::NoFiles {
                    path: root.to_path_buf(),
                })
            } else {
                Ok(detected)
            }
        }
    }
}
