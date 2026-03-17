use crate::config::ResolvedSpecsQualityConfig;
use crate::errors::{Result, UntangleError};
use crate::walk::{self, Language};
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct SpecQualityRunConfig {
    pub root: PathBuf,
    pub lang: Option<Language>,
    pub top: Option<usize>,
    pub quiet: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub defaults: ResolvedSpecsQualityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecQualityReport {
    pub metadata: SpecQualityMetadata,
    pub summary: SpecQualitySummary,
    pub files: Vec<SpecFileReport>,
    pub worst_cases: Vec<SpecCaseRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comparison: Option<SpecComparisonSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecQualityMetadata {
    pub root: PathBuf,
    pub languages: Vec<String>,
    pub files_parsed: usize,
    pub cases: usize,
    pub timestamp: String,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecQualitySummary {
    pub file_count: usize,
    pub case_count: usize,
    pub avg_score: f64,
    pub max_score: f64,
    pub zero_assertion_cases: usize,
    pub low_assertion_cases: usize,
    pub mocking_heavy_cases: usize,
    pub branching_cases: usize,
    pub harmful_duplication_score: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecFileReport {
    pub path: PathBuf,
    pub language: Language,
    pub summary: SpecFileSummary,
    pub guidance: SpecGuidance,
    pub cases: Vec<SpecCaseReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comparison: Option<SpecFileComparison>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecFileSummary {
    pub case_count: usize,
    pub avg_score: f64,
    pub max_score: f64,
    pub zero_assertion_cases: usize,
    pub low_assertion_cases: usize,
    pub mocking_heavy_cases: usize,
    pub branching_cases: usize,
    pub harmful_duplication_score: usize,
    pub case_matrix_candidates: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecCaseReport {
    pub name: String,
    pub path: PathBuf,
    pub language: Language,
    pub context_path: Vec<String>,
    pub start_line: usize,
    pub end_line: usize,
    pub line_count: usize,
    pub assertion_count: usize,
    pub branch_count: usize,
    pub setup_depth: usize,
    pub mock_count: usize,
    pub helper_calls: usize,
    pub table_driven: bool,
    pub smells: Vec<String>,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecCaseRef {
    pub path: PathBuf,
    pub name: String,
    pub start_line: usize,
    pub score: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecPressure {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecRemediationMode {
    Stable,
    Local,
    Split,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecActionability {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecGuidance {
    pub pressure: SpecPressure,
    pub remediation_mode: SpecRemediationMode,
    pub ai_actionability: SpecActionability,
    pub ai_guidance: String,
    pub why: Vec<SpecGuidanceLine>,
    #[serde(rename = "where")]
    pub where_: Vec<SpecGuidanceHotspot>,
    pub how: Vec<SpecGuidanceAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecGuidanceLine {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecGuidanceHotspot {
    pub title: String,
    pub summary: String,
    pub location: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecGuidanceAction {
    pub confidence: u8,
    pub label: String,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpecComparisonVerdict {
    Improved,
    Worse,
    Mixed,
    Unchanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecFileComparison {
    pub verdict: SpecComparisonVerdict,
    pub score_delta: f64,
    pub max_score_delta: f64,
    pub harmful_duplication_delta: isize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecComparisonSummary {
    pub verdict: SpecComparisonVerdict,
    pub improved_files: usize,
    pub worse_files: usize,
    pub mixed_files: usize,
    pub unchanged_files: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct SpecQualityEnvelope {
    kind: String,
    schema_version: u32,
    report: SpecQualityReport,
}

#[derive(Debug, Clone)]
struct DiscoveredCase {
    name: String,
    context_path: Vec<String>,
    start_line: usize,
    end_line: usize,
}

#[derive(Debug, Clone)]
struct FileAnalysis {
    path: PathBuf,
    language: Language,
    source: String,
    cases: Vec<DiscoveredCase>,
    helper_names: Vec<String>,
}

pub fn run(config: SpecQualityRunConfig) -> Result<SpecQualityReport> {
    let start = Instant::now();
    let root = config
        .root
        .canonicalize()
        .map_err(|_| UntangleError::NoFiles { path: config.root.clone() })?;
    let analyses = discover_and_extract(
        &root,
        config.lang,
        &config.include,
        &merged_excludes(&config.exclude, &config.ignore_patterns),
    )?;
    let mut report = build_report(&root, analyses, &config.defaults, config.top);
    report.metadata.elapsed_ms = start.elapsed().as_millis() as u64;
    Ok(report)
}

pub fn write_baseline(report: &SpecQualityReport, path: Option<&Path>) -> Result<PathBuf> {
    let target = path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| report.metadata.root.join("target").join("untangle").join("specs.json"));
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(&target)?;
    write_json(&mut file, report)?;
    Ok(target)
}

pub fn attach_comparison(report: &mut SpecQualityReport, baseline_path: &Path) -> Result<()> {
    let baseline = load_baseline(baseline_path)?;
    compare_reports(report, &baseline.report);
    Ok(())
}

pub fn write_json<W: Write>(writer: &mut W, report: &SpecQualityReport) -> Result<()> {
    serde_json::to_writer_pretty(
        writer,
        &SpecQualityEnvelope {
            kind: "quality.specs".to_string(),
            schema_version: 1,
            report: report.clone(),
        },
    )?;
    Ok(())
}

pub fn write_text<W: Write>(writer: &mut W, report: &SpecQualityReport) -> Result<()> {
    writeln!(writer, "Untangle Spec Quality Report")?;
    writeln!(writer, "===========================")?;
    writeln!(writer)?;
    writeln!(writer, "Root: {}", report.metadata.root.display())?;
    writeln!(writer, "Files: {}", report.metadata.files_parsed)?;
    writeln!(writer, "Cases: {}", report.metadata.cases)?;
    writeln!(writer)?;
    if let Some(comparison) = &report.comparison {
        writeln!(writer, "Comparison")?;
        writeln!(writer, "----------")?;
        writeln!(writer, "Verdict: {}", comparison_label(comparison.verdict))?;
        writeln!(
            writer,
            "Files: improved={} worse={} mixed={} unchanged={}",
            comparison.improved_files,
            comparison.worse_files,
            comparison.mixed_files,
            comparison.unchanged_files
        )?;
        writeln!(writer)?;
    }

    for file in &report.files {
        writeln!(writer, "{}", file.path.display())?;
        writeln!(
            writer,
            "  refactor-pressure: {} ({:.1})",
            pressure_label(file.guidance.pressure),
            file.summary.max_score
        )?;
        writeln!(
            writer,
            "  remediation-mode: {}",
            remediation_label(file.guidance.remediation_mode)
        )?;
        writeln!(
            writer,
            "  ai-actionability: {}",
            actionability_label(file.guidance.ai_actionability)
        )?;
        writeln!(writer, "  ai-guidance: {}", file.guidance.ai_guidance)?;
        writeln!(writer, "  why:")?;
        for line in &file.guidance.why {
            writeln!(writer, "    {}: {}", line.label, line.value)?;
        }
        if let Some(comparison) = &file.comparison {
            writeln!(writer, "  comparison: {}", comparison_label(comparison.verdict))?;
            writeln!(writer, "    score-delta: {:.1}", comparison.score_delta)?;
            writeln!(writer, "    max-score-delta: {:.1}", comparison.max_score_delta)?;
            writeln!(
                writer,
                "    harmful-duplication-delta: {}",
                comparison.harmful_duplication_delta
            )?;
        }
        if !file.guidance.where_.is_empty() {
            writeln!(writer, "  where:")?;
            for hotspot in &file.guidance.where_ {
                writeln!(writer, "    {} -> {}", hotspot.location, hotspot.title)?;
                writeln!(writer, "      {}", hotspot.summary)?;
            }
        }
        if !file.guidance.how.is_empty() {
            writeln!(writer, "  how:")?;
            for action in &file.guidance.how {
                writeln!(writer, "    {}: {}", action.label, action.text)?;
            }
        }
        if !file.cases.is_empty() {
            writeln!(writer, "  worst-examples:")?;
            for case in file.cases.iter().take(3) {
                writeln!(
                    writer,
                    "    {}:{} {} -> score {:.1} [{}]",
                    case.path.display(),
                    case.start_line,
                    qualified_case_name(case),
                    case.score,
                    if case.smells.is_empty() {
                        "none".to_string()
                    } else {
                        case.smells.join(", ")
                    }
                )?;
            }
        }
        writeln!(writer)?;
    }

    if !report.worst_cases.is_empty() {
        writeln!(writer, "Worst Examples")?;
        writeln!(writer, "--------------")?;
        for (idx, case) in report.worst_cases.iter().enumerate() {
            writeln!(
                writer,
                "{}. {}:{} {} ({:.1})",
                idx + 1,
                case.path.display(),
                case.start_line,
                case.name,
                case.score
            )?;
        }
    }

    Ok(())
}

fn discover_and_extract(
    root: &Path,
    lang: Option<Language>,
    include: &[String],
    exclude: &[String],
) -> Result<Vec<FileAnalysis>> {
    let include_set = build_globset(include)?;
    let exclude_set = build_globset(exclude)?;
    let langs = match lang {
        Some(lang) => vec![lang],
        None => walk::detect_languages(root),
    };
    if langs.is_empty() {
        return Err(UntangleError::NoFiles {
            path: root.to_path_buf(),
        });
    }

    let mut analyses = Vec::new();
    let walker = WalkBuilder::new(root).hidden(false).git_ignore(true).build();
    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(language) = walk::language_for_file(path) else {
            continue;
        };
        if !langs.contains(&language) {
            continue;
        }
        let relative = path.strip_prefix(root).unwrap_or(path);
        if exclude_set.is_match(relative) || exclude_set.is_match(path) {
            continue;
        }
        if !include.is_empty() && !include_set.is_match(relative) && !include_set.is_match(path) {
            continue;
        }
        if !is_test_like_path(relative, language) {
            continue;
        }
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let cases = discover_cases(language, &source);
        if cases.is_empty() {
            continue;
        }
        let helper_names = discover_helper_names(language, &source, &cases);
        analyses.push(FileAnalysis {
            path: relative.to_path_buf(),
            language,
            source,
            cases,
            helper_names,
        });
    }

    analyses.sort_by(|a, b| a.path.cmp(&b.path));
    if analyses.is_empty() {
        return Err(UntangleError::NoFiles {
            path: root.to_path_buf(),
        });
    }
    Ok(analyses)
}

fn build_report(
    root: &Path,
    analyses: Vec<FileAnalysis>,
    defaults: &ResolvedSpecsQualityConfig,
    top: Option<usize>,
) -> SpecQualityReport {
    let mut file_reports = Vec::new();
    let mut languages = HashSet::new();
    let mut all_cases = Vec::new();

    for analysis in analyses {
        languages.insert(analysis.language.to_string());
        let mut cases = analysis
            .cases
            .iter()
            .map(|case| score_case(&analysis, case))
            .collect::<Vec<_>>();
        cases.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let summary = summarize_file(&cases);
        let guidance = build_guidance(&analysis.path, &summary, &cases, defaults);
        all_cases.extend(cases.iter().map(|case| SpecCaseRef {
            path: case.path.clone(),
            name: qualified_case_name(case),
            start_line: case.start_line,
            score: case.score,
        }));
        file_reports.push(SpecFileReport {
            path: analysis.path,
            language: analysis.language,
            summary,
            guidance,
            cases,
            comparison: None,
        });
    }

    file_reports.sort_by(|a, b| a.path.cmp(&b.path));
    all_cases.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let top_limit = top.or(defaults.top).unwrap_or(10);
    all_cases.truncate(top_limit);

    let summary = summarize_report(&file_reports);
    let mut language_list = languages.into_iter().collect::<Vec<_>>();
    language_list.sort();

    SpecQualityReport {
        metadata: SpecQualityMetadata {
            root: root.to_path_buf(),
            languages: language_list,
            files_parsed: file_reports.len(),
            cases: file_reports.iter().map(|file| file.cases.len()).sum(),
            timestamp: chrono_now(),
            elapsed_ms: 0,
        },
        summary,
        files: file_reports,
        worst_cases: all_cases,
        comparison: None,
    }
}

fn merged_excludes(exclude: &[String], ignore_patterns: &[String]) -> Vec<String> {
    let mut merged = exclude.to_vec();
    merged.extend(ignore_patterns.iter().cloned());
    merged
}

fn build_globset(patterns: &[String]) -> Result<globset::GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern)?);
    }
    Ok(builder.build()?)
}

fn is_test_like_path(relative: &Path, language: Language) -> bool {
    let text = relative.to_string_lossy();
    let file_name = relative.file_name().and_then(|name| name.to_str()).unwrap_or("");
    match language {
        Language::Python => {
            text.contains("/tests/")
                || text.starts_with("tests/")
                || file_name.starts_with("test_")
                || file_name.ends_with("_test.py")
        }
        Language::Ruby => {
            text.contains("/spec/")
                || text.starts_with("spec/")
                || text.contains("/test/")
                || text.starts_with("test/")
                || file_name.ends_with("_spec.rb")
                || file_name.starts_with("test_")
        }
        Language::Go => file_name.ends_with("_test.go"),
        Language::Rust => text.contains("/tests/") || text.starts_with("tests/"),
    }
}

fn discover_cases(language: Language, source: &str) -> Vec<DiscoveredCase> {
    match language {
        Language::Python => discover_python_cases(source),
        Language::Ruby => discover_ruby_cases(source),
        Language::Go => discover_go_cases(source),
        Language::Rust => discover_rust_cases(source),
    }
}

fn discover_helper_names(language: Language, source: &str, cases: &[DiscoveredCase]) -> Vec<String> {
    let case_names: HashSet<&str> = cases.iter().map(|case| case.name.as_str()).collect();
    let mut names = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        let name = match language {
            Language::Python if trimmed.starts_with("def ") => extract_identifier_after(trimmed, "def "),
            Language::Ruby if trimmed.starts_with("def ") => extract_identifier_after(trimmed, "def "),
            Language::Go if trimmed.starts_with("func ") => extract_go_function_name(trimmed),
            Language::Rust if trimmed.starts_with("fn ") || trimmed.contains(" fn ") => {
                extract_identifier_after(trimmed.split("fn ").nth(1).unwrap_or(""), "")
            }
            _ => None,
        };
        if let Some(name) = name {
            let cleaned = name.trim_end_matches(|c: char| c == '(' || c == '{').to_string();
            if !case_names.contains(cleaned.as_str()) {
                names.push(cleaned);
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

fn discover_python_cases(source: &str) -> Vec<DiscoveredCase> {
    let lines: Vec<&str> = source.lines().collect();
    let mut cases = Vec::new();
    let mut contexts: Vec<(usize, String)> = Vec::new();
    let mut current = 0usize;
    while current < lines.len() {
        let line = lines[current];
        let indent = indentation(line);
        while contexts.last().map(|(lvl, _)| *lvl >= indent).unwrap_or(false) {
            contexts.pop();
        }
        let trimmed = line.trim();
        if trimmed.starts_with("class ") {
            if let Some(name) = extract_identifier_after(trimmed, "class ") {
                contexts.push((indent, name.trim_end_matches(':').to_string()));
            }
        } else if trimmed.starts_with("def ") {
            let name = extract_identifier_after(trimmed, "def ").unwrap_or_default();
            let test_like = name.starts_with("test_")
                || contexts.iter().any(|(_, ctx)| ctx.starts_with("Test"))
                || source.contains("unittest.TestCase");
            if test_like {
                let end = find_python_block_end(&lines, current + 1, indent);
                cases.push(DiscoveredCase {
                    name,
                    context_path: contexts.iter().map(|(_, ctx)| ctx.clone()).collect(),
                    start_line: current + 1,
                    end_line: end,
                });
                current = end;
                continue;
            }
        }
        current += 1;
    }
    cases
}

fn find_python_block_end(lines: &[&str], start: usize, indent: usize) -> usize {
    let mut end = lines.len();
    for (idx, line) in lines.iter().enumerate().skip(start) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if indentation(line) <= indent {
            end = idx;
            break;
        }
    }
    end
}

fn discover_go_cases(source: &str) -> Vec<DiscoveredCase> {
    let lines: Vec<&str> = source.lines().collect();
    let mut cases = Vec::new();
    let mut idx = 0usize;
    while idx < lines.len() {
        let trimmed = lines[idx].trim();
        if trimmed.starts_with("func Test") {
            let name = extract_go_function_name(trimmed).unwrap_or_default();
            let end = find_brace_block_end(&lines, idx);
            cases.push(DiscoveredCase {
                name,
                context_path: Vec::new(),
                start_line: idx + 1,
                end_line: end,
            });
            idx = end;
            continue;
        }
        idx += 1;
    }
    cases
}

fn discover_rust_cases(source: &str) -> Vec<DiscoveredCase> {
    let lines: Vec<&str> = source.lines().collect();
    let mut cases = Vec::new();
    let mut idx = 0usize;
    while idx < lines.len() {
        let trimmed = lines[idx].trim();
        if trimmed.starts_with("#[test]") {
            let mut fn_idx = idx + 1;
            while fn_idx < lines.len() && !lines[fn_idx].contains("fn ") {
                fn_idx += 1;
            }
            if fn_idx < lines.len() {
                let name = extract_identifier_after(lines[fn_idx].trim().split("fn ").nth(1).unwrap_or(""), "")
                    .unwrap_or_default();
                let end = find_brace_block_end(&lines, fn_idx);
                cases.push(DiscoveredCase {
                    name,
                    context_path: Vec::new(),
                    start_line: fn_idx + 1,
                    end_line: end,
                });
                idx = end;
                continue;
            }
        }
        idx += 1;
    }
    cases
}

fn discover_ruby_cases(source: &str) -> Vec<DiscoveredCase> {
    let lines: Vec<&str> = source.lines().collect();
    let mut cases = Vec::new();
    let mut context_stack: Vec<(usize, String)> = Vec::new();
    let mut do_balance = 0usize;
    let mut idx = 0usize;
    while idx < lines.len() {
        let trimmed = lines[idx].trim();
        if trimmed.starts_with("describe ") || trimmed.starts_with("context ") {
            let name = extract_quoted_name(trimmed).unwrap_or_else(|| trimmed.to_string());
            context_stack.push((do_balance, name));
        } else if trimmed.starts_with("it ") || trimmed.starts_with("specify ") {
            let name = extract_quoted_name(trimmed).unwrap_or_else(|| "example".to_string());
            let end = find_ruby_block_end(&lines, idx);
            cases.push(DiscoveredCase {
                name,
                context_path: context_stack.iter().map(|(_, ctx)| ctx.clone()).collect(),
                start_line: idx + 1,
                end_line: end,
            });
            idx = end;
            continue;
        } else if trimmed.starts_with("def test_") {
            let name = extract_identifier_after(trimmed, "def ").unwrap_or_default();
            let end = find_ruby_def_end(&lines, idx);
            cases.push(DiscoveredCase {
                name,
                context_path: Vec::new(),
                start_line: idx + 1,
                end_line: end,
            });
            idx = end;
            continue;
        }

        do_balance += count_token(trimmed, " do");
        if trimmed == "end" || trimmed.ends_with(" end") {
            while let Some((level, _)) = context_stack.last() {
                if *level >= do_balance.saturating_sub(1) {
                    context_stack.pop();
                } else {
                    break;
                }
            }
            do_balance = do_balance.saturating_sub(1);
        }
        idx += 1;
    }
    cases
}

fn find_ruby_block_end(lines: &[&str], start_idx: usize) -> usize {
    let mut balance = 0isize;
    for (idx, line) in lines.iter().enumerate().skip(start_idx) {
        let trimmed = line.trim();
        if idx == start_idx {
            balance += 1;
        }
        balance += count_token(trimmed, " do") as isize;
        if trimmed == "end" || trimmed.ends_with(" end") {
            balance -= 1;
            if balance <= 0 {
                return idx + 1;
            }
        }
    }
    lines.len()
}

fn find_ruby_def_end(lines: &[&str], start_idx: usize) -> usize {
    let mut balance = 0isize;
    for (idx, line) in lines.iter().enumerate().skip(start_idx) {
        let trimmed = line.trim();
        if idx == start_idx {
            balance += 1;
        }
        if trimmed.starts_with("def ") {
            balance += 1;
        }
        if trimmed == "end" || trimmed.ends_with(" end") {
            balance -= 1;
            if balance <= 0 {
                return idx + 1;
            }
        }
    }
    lines.len()
}

fn find_brace_block_end(lines: &[&str], start_idx: usize) -> usize {
    let mut balance = 0isize;
    let mut started = false;
    for (idx, line) in lines.iter().enumerate().skip(start_idx) {
        for ch in line.chars() {
            if ch == '{' {
                balance += 1;
                started = true;
            } else if ch == '}' {
                balance -= 1;
            }
        }
        if started && balance <= 0 {
            return idx + 1;
        }
    }
    lines.len()
}

fn score_case(analysis: &FileAnalysis, case: &DiscoveredCase) -> SpecCaseReport {
    let lines: Vec<&str> = analysis.source.lines().collect();
    let start = case.start_line.saturating_sub(1).min(lines.len());
    let end = case.end_line.min(lines.len());
    let snippet = lines[start..end].join("\n");
    let line_count = case.end_line.saturating_sub(case.start_line) + 1;
    let assertion_count = count_assertions(analysis.language, &snippet);
    let branch_count = count_branches(analysis.language, &snippet);
    let setup_depth = count_setup_depth(analysis.language, &snippet);
    let mock_count = count_mocks(analysis.language, &snippet);
    let helper_calls = count_helper_calls(&snippet, &analysis.helper_names, &case.name);
    let table_driven = is_table_driven(analysis.language, &snippet);

    let mut smells = Vec::new();
    if assertion_count == 0 {
        smells.push("zero-assertion".to_string());
    }
    if assertion_count <= 1 && line_count > 8 {
        smells.push("low-assertion".to_string());
    }
    if branch_count >= 3 {
        smells.push("logic-heavy".to_string());
    }
    if line_count > 20 {
        smells.push("large-example".to_string());
    }
    if mock_count >= 3 {
        smells.push("high-mocking".to_string());
    }
    if helper_calls > 0 {
        smells.push("helper-indirection".to_string());
    }
    if !table_driven && branch_count <= 1 && line_count <= 12 && helper_calls == 0 {
        smells.push("table-driven-candidate".to_string());
    }

    let mut score = 1.0 + branch_count as f64 + setup_depth as f64 + helper_calls as f64;
    score += mock_count as f64 * 1.5;
    if assertion_count == 0 {
        score += 10.0;
    }
    if assertion_count <= 1 && line_count > 8 {
        score += 6.0;
    }
    if branch_count >= 3 {
        score += 4.0;
    }
    if line_count > 20 {
        score += 4.0;
    }
    if mock_count >= 3 {
        score += 4.0;
    }
    if helper_calls > 0 {
        score += 2.0;
    }

    SpecCaseReport {
        name: case.name.clone(),
        path: analysis.path.clone(),
        language: analysis.language,
        context_path: case.context_path.clone(),
        start_line: case.start_line,
        end_line: case.end_line,
        line_count,
        assertion_count,
        branch_count,
        setup_depth,
        mock_count,
        helper_calls,
        table_driven,
        smells,
        score: (score * 10.0).round() / 10.0,
    }
}

fn summarize_file(cases: &[SpecCaseReport]) -> SpecFileSummary {
    if cases.is_empty() {
        return SpecFileSummary {
            case_count: 0,
            avg_score: 0.0,
            max_score: 0.0,
            zero_assertion_cases: 0,
            low_assertion_cases: 0,
            mocking_heavy_cases: 0,
            branching_cases: 0,
            harmful_duplication_score: 0,
            case_matrix_candidates: 0,
        };
    }
    let total = cases.iter().map(|case| case.score).sum::<f64>();
    let max_score = cases.iter().map(|case| case.score).fold(0.0, f64::max);
    let signatures = repeated_signatures(cases);
    SpecFileSummary {
        case_count: cases.len(),
        avg_score: ((total / cases.len() as f64) * 10.0).round() / 10.0,
        max_score: (max_score * 10.0).round() / 10.0,
        zero_assertion_cases: cases.iter().filter(|case| case.assertion_count == 0).count(),
        low_assertion_cases: cases.iter().filter(|case| case.assertion_count <= 1).count(),
        mocking_heavy_cases: cases.iter().filter(|case| case.mock_count >= 3).count(),
        branching_cases: cases.iter().filter(|case| case.branch_count > 0).count(),
        harmful_duplication_score: signatures,
        case_matrix_candidates: cases
            .iter()
            .filter(|case| case.smells.iter().any(|smell| smell == "table-driven-candidate"))
            .count(),
    }
}

fn repeated_signatures(cases: &[SpecCaseReport]) -> usize {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for case in cases {
        let mut parts = vec![
            case.assertion_count.to_string(),
            case.branch_count.to_string(),
            case.mock_count.to_string(),
        ];
        if let Some(first_smell) = case.smells.first() {
            parts.push(first_smell.clone());
        }
        *counts.entry(parts.join(":")).or_insert(0) += 1;
    }
    counts.values().filter(|count| **count > 1).map(|count| count - 1).sum()
}

fn build_guidance(
    path: &Path,
    summary: &SpecFileSummary,
    cases: &[SpecCaseReport],
    defaults: &ResolvedSpecsQualityConfig,
) -> SpecGuidance {
    let severe_cases = cases.iter().filter(|case| case.score >= defaults.split_min_score).count();
    let pressure = if summary.max_score >= defaults.split_min_score || severe_cases >= 2 {
        SpecPressure::High
    } else if summary.max_score <= defaults.stable_max_score
        && summary.zero_assertion_cases == 0
        && summary.mocking_heavy_cases == 0
    {
        SpecPressure::Low
    } else {
        SpecPressure::Medium
    };
    let remediation_mode = match pressure {
        SpecPressure::Low => SpecRemediationMode::Stable,
        SpecPressure::Medium => SpecRemediationMode::Local,
        SpecPressure::High => SpecRemediationMode::Split,
    };
    let ai_actionability = match remediation_mode {
        SpecRemediationMode::Stable => SpecActionability::Low,
        SpecRemediationMode::Local => SpecActionability::Medium,
        SpecRemediationMode::Split => SpecActionability::High,
    };
    let ai_guidance = match remediation_mode {
        SpecRemediationMode::Stable => {
            "No immediate test refactor is recommended; the file is structurally stable enough."
        }
        SpecRemediationMode::Local => {
            "Target the highest-pressure tests directly before attempting broader cleanup."
        }
        SpecRemediationMode::Split => {
            "Structural pressure is spread across several tests; split the file by concern before local cleanup."
        }
    }
    .to_string();

    let why = vec![
        SpecGuidanceLine {
            label: "avg_score".to_string(),
            value: format!("{:.1}", summary.avg_score),
        },
        SpecGuidanceLine {
            label: "max_score".to_string(),
            value: format!("{:.1}", summary.max_score),
        },
        SpecGuidanceLine {
            label: "zero_assertion_cases".to_string(),
            value: summary.zero_assertion_cases.to_string(),
        },
        SpecGuidanceLine {
            label: "mocking_heavy_cases".to_string(),
            value: summary.mocking_heavy_cases.to_string(),
        },
        SpecGuidanceLine {
            label: "harmful_duplication_score".to_string(),
            value: summary.harmful_duplication_score.to_string(),
        },
    ];

    let where_ = cases
        .iter()
        .take(3)
        .map(|case| SpecGuidanceHotspot {
            title: format!("Refactor {}", qualified_case_name(case)),
            summary: format!(
                "score={:.1} assertions={} branches={} mocks={}",
                case.score, case.assertion_count, case.branch_count, case.mock_count
            ),
            location: format!("{}:{}", path.display(), case.start_line),
        })
        .collect();

    let mut how = Vec::new();
    if summary.zero_assertion_cases > 0 {
        how.push(SpecGuidanceAction {
            confidence: 3,
            label: "HIGH".to_string(),
            text: "Strengthen assertions before doing structural cleanup.".to_string(),
        });
    }
    if summary.mocking_heavy_cases > 0 {
        how.push(SpecGuidanceAction {
            confidence: 2,
            label: "MEDIUM".to_string(),
            text: "Reduce mocking and move more coverage toward behavior-level checks."
                .to_string(),
        });
    }
    if summary.harmful_duplication_score > 0 {
        how.push(SpecGuidanceAction {
            confidence: 2,
            label: "MEDIUM".to_string(),
            text: "Extract repeated setup only where harmful duplication is dominating."
                .to_string(),
        });
    }
    if summary.case_matrix_candidates > 0 {
        how.push(SpecGuidanceAction {
            confidence: 2,
            label: "MEDIUM".to_string(),
            text: "Convert repeated low-complexity cases into table-driven tests."
                .to_string(),
        });
    }
    if how.is_empty() {
        how.push(SpecGuidanceAction {
            confidence: 3,
            label: "HIGH".to_string(),
            text: "No refactor is recommended right now.".to_string(),
        });
    }

    SpecGuidance {
        pressure,
        remediation_mode,
        ai_actionability,
        ai_guidance,
        why,
        where_,
        how,
    }
}

fn summarize_report(files: &[SpecFileReport]) -> SpecQualitySummary {
    let case_count: usize = files.iter().map(|file| file.summary.case_count).sum();
    let file_count = files.len();
    let avg_score = if file_count == 0 {
        0.0
    } else {
        files.iter().map(|file| file.summary.avg_score).sum::<f64>() / file_count as f64
    };
    SpecQualitySummary {
        file_count,
        case_count,
        avg_score: (avg_score * 10.0).round() / 10.0,
        max_score: files.iter().map(|file| file.summary.max_score).fold(0.0, f64::max),
        zero_assertion_cases: files.iter().map(|file| file.summary.zero_assertion_cases).sum(),
        low_assertion_cases: files.iter().map(|file| file.summary.low_assertion_cases).sum(),
        mocking_heavy_cases: files.iter().map(|file| file.summary.mocking_heavy_cases).sum(),
        branching_cases: files.iter().map(|file| file.summary.branching_cases).sum(),
        harmful_duplication_score: files
            .iter()
            .map(|file| file.summary.harmful_duplication_score)
            .sum(),
    }
}

fn compare_reports(current: &mut SpecQualityReport, baseline: &SpecQualityReport) {
    let baseline_by_path: HashMap<&PathBuf, &SpecFileReport> =
        baseline.files.iter().map(|file| (&file.path, file)).collect();
    let mut improved = 0usize;
    let mut worse = 0usize;
    let mut mixed = 0usize;
    let mut unchanged = 0usize;

    for file in &mut current.files {
        if let Some(previous) = baseline_by_path.get(&file.path) {
            let comparison = compare_file(&file.summary, &previous.summary);
            match comparison.verdict {
                SpecComparisonVerdict::Improved => improved += 1,
                SpecComparisonVerdict::Worse => worse += 1,
                SpecComparisonVerdict::Mixed => mixed += 1,
                SpecComparisonVerdict::Unchanged => unchanged += 1,
            }
            file.comparison = Some(comparison);
        }
    }
    let verdict = if worse > 0 && improved == 0 && mixed == 0 {
        SpecComparisonVerdict::Worse
    } else if improved > 0 && worse == 0 && mixed == 0 {
        SpecComparisonVerdict::Improved
    } else if mixed > 0 || (improved > 0 && worse > 0) {
        SpecComparisonVerdict::Mixed
    } else {
        SpecComparisonVerdict::Unchanged
    };
    current.comparison = Some(SpecComparisonSummary {
        verdict,
        improved_files: improved,
        worse_files: worse,
        mixed_files: mixed,
        unchanged_files: unchanged,
    });
}

fn compare_file(current: &SpecFileSummary, previous: &SpecFileSummary) -> SpecFileComparison {
    let score_delta = current.avg_score - previous.avg_score;
    let max_score_delta = current.max_score - previous.max_score;
    let harmful_duplication_delta =
        current.harmful_duplication_score as isize - previous.harmful_duplication_score as isize;
    let verdict = if score_delta <= -2.0 && max_score_delta <= 0.0 && harmful_duplication_delta <= 0 {
        SpecComparisonVerdict::Improved
    } else if score_delta >= 2.0 || max_score_delta > 0.0 || harmful_duplication_delta > 0 {
        if score_delta < 0.0 {
            SpecComparisonVerdict::Mixed
        } else {
            SpecComparisonVerdict::Worse
        }
    } else {
        SpecComparisonVerdict::Unchanged
    };
    SpecFileComparison {
        verdict,
        score_delta: (score_delta * 10.0).round() / 10.0,
        max_score_delta: (max_score_delta * 10.0).round() / 10.0,
        harmful_duplication_delta,
    }
}

fn load_baseline(path: &Path) -> Result<SpecQualityEnvelope> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn count_assertions(language: Language, snippet: &str) -> usize {
    let needles = match language {
        Language::Python => vec!["assert ", "self.assert", "pytest.raises"],
        Language::Ruby => vec!["expect(", "assert_", "assert ", "refute_", "raise_error"],
        Language::Go => vec!["t.Fatal", "t.Fatalf", "t.Error", "t.Errorf", "require.", "assert."],
        Language::Rust => vec!["assert!", "assert_eq!", "assert_ne!", "matches!"],
    };
    needles.into_iter().map(|needle| count_token(snippet, needle)).sum()
}

fn count_branches(language: Language, snippet: &str) -> usize {
    let needles = match language {
        Language::Python => vec!["if ", "elif ", "for ", "while ", " except", " and ", " or "],
        Language::Ruby => vec!["if ", "elsif ", "unless ", "while ", "until ", " when ", " and ", " or "],
        Language::Go => vec!["if ", "for ", "case ", "&&", "||"],
        Language::Rust => vec!["if ", "while ", "for ", "match ", "&&", "||"],
    };
    needles.into_iter().map(|needle| count_token(snippet, needle)).sum()
}

fn count_setup_depth(language: Language, snippet: &str) -> usize {
    let needles = match language {
        Language::Python => vec!["with ", "fixture", "setup", "arrange"],
        Language::Ruby => vec!["before ", "let(", "let!(", "subject", "setup"],
        Language::Go => vec!["t.Run(", "setup", "defer "],
        Language::Rust => vec!["let ", "setup", "arrange", "mod fixtures"],
    };
    needles.into_iter().map(|needle| count_token(snippet, needle)).sum()
}

fn count_mocks(language: Language, snippet: &str) -> usize {
    let needles = match language {
        Language::Python => vec!["mock.", "Mock(", "MagicMock(", "patch(", "monkeypatch", "stub"],
        Language::Ruby => vec!["double(", "allow(", "receive(", "stub(", "mock("],
        Language::Go => vec!["gomock", "mock.", "fake", "stub"],
        Language::Rust => vec!["mockall", "mock_", "double", "stub"],
    };
    needles.into_iter().map(|needle| count_token(snippet, needle)).sum()
}

fn count_helper_calls(snippet: &str, helper_names: &[String], current_name: &str) -> usize {
    helper_names
        .iter()
        .filter(|name| name.as_str() != current_name)
        .map(|name| count_token(snippet, &format!("{name}(")))
        .sum()
}

fn is_table_driven(language: Language, snippet: &str) -> bool {
    match language {
        Language::Python => snippet.contains("parametrize") || snippet.contains("subTest("),
        Language::Ruby => snippet.contains("shared_examples") || snippet.contains("each do |"),
        Language::Go => snippet.contains("[]struct") || snippet.contains("t.Run("),
        Language::Rust => snippet.contains("for case in") || snippet.contains("cases.iter()"),
    }
}

fn count_token(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

fn indentation(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count()
}

fn extract_identifier_after(text: &str, prefix: &str) -> Option<String> {
    let remainder = text.strip_prefix(prefix).unwrap_or(text);
    let ident: String = remainder
        .chars()
        .take_while(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == ':' || *ch == '.')
        .collect();
    if ident.is_empty() { None } else { Some(ident) }
}

fn extract_go_function_name(text: &str) -> Option<String> {
    if let Some(rest) = text.strip_prefix("func ") {
        if rest.starts_with('(') {
            let after_receiver = rest.split(')').nth(1)?.trim_start();
            return extract_identifier_after(after_receiver, "");
        }
        return extract_identifier_after(rest, "");
    }
    None
}

fn extract_quoted_name(text: &str) -> Option<String> {
    let quote = if text.contains('"') { '"' } else if text.contains('\'') { '\'' } else { return None };
    let mut parts = text.split(quote);
    parts.next()?;
    parts.next().map(|s| s.to_string())
}

fn qualified_case_name(case: &SpecCaseReport) -> String {
    if case.context_path.is_empty() {
        case.name.clone()
    } else {
        format!("{} / {}", case.context_path.join(" / "), case.name)
    }
}

fn pressure_label(pressure: SpecPressure) -> &'static str {
    match pressure {
        SpecPressure::Low => "low",
        SpecPressure::Medium => "medium",
        SpecPressure::High => "high",
    }
}

fn remediation_label(mode: SpecRemediationMode) -> &'static str {
    match mode {
        SpecRemediationMode::Stable => "stable",
        SpecRemediationMode::Local => "local",
        SpecRemediationMode::Split => "split",
    }
}

fn actionability_label(actionability: SpecActionability) -> &'static str {
    match actionability {
        SpecActionability::Low => "low",
        SpecActionability::Medium => "medium",
        SpecActionability::High => "high",
    }
}

fn comparison_label(verdict: SpecComparisonVerdict) -> &'static str {
    match verdict {
        SpecComparisonVerdict::Improved => "improved",
        SpecComparisonVerdict::Worse => "worse",
        SpecComparisonVerdict::Mixed => "mixed",
        SpecComparisonVerdict::Unchanged => "unchanged",
    }
}

fn chrono_now() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let days = (secs / 86400) as i64;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}
