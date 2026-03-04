use assert_cmd::cargo::cargo_bin_cmd;
use cucumber::{given, then, when, World};
use std::path::Path;
use tempfile::TempDir;

#[derive(Debug, Default, World)]
struct CliWorld {
    output: Option<std::process::Output>,
    temp_dir: Option<TempDir>,
}

fn run_cmd(args: &[&str], cwd: Option<&Path>) -> std::process::Output {
    let mut cmd = cargo_bin_cmd!("untangle");
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.args(args).output().expect("run untangle command")
}

#[given("the analyze fixtures")]
fn analyze_fixtures(_world: &mut CliWorld) {}

#[given("the quality fixtures")]
fn quality_fixtures(_world: &mut CliWorld) {}

#[given("the diff fixtures")]
fn diff_fixtures(_world: &mut CliWorld) {}

#[given("an empty temp project")]
fn empty_temp_project(world: &mut CliWorld) {
    world.temp_dir = Some(TempDir::new().expect("temp dir"));
}

#[when("I run analyze in text format")]
fn run_analyze_text(world: &mut CliWorld) {
    let output = run_cmd(
        &[
            "analyze",
            "tests/fixtures/go/simple_module",
            "--lang",
            "go",
            "--format",
            "text",
            "--quiet",
        ],
        None,
    );
    world.output = Some(output);
}

#[when("I run analyze with top 5")]
fn run_analyze_top(world: &mut CliWorld) {
    let output = run_cmd(
        &[
            "analyze",
            "tests/fixtures/go/simple_module",
            "--lang",
            "go",
            "--format",
            "text",
            "--top",
            "5",
            "--quiet",
        ],
        None,
    );
    world.output = Some(output);
}

#[when("I run analyze in json format")]
fn run_analyze_json(world: &mut CliWorld) {
    let output = run_cmd(
        &[
            "analyze",
            "tests/fixtures/go/simple_module",
            "--lang",
            "go",
            "--format",
            "json",
            "--quiet",
        ],
        None,
    );
    world.output = Some(output);
}

#[when("I run graph in dot format")]
fn run_graph_dot(world: &mut CliWorld) {
    let output = run_cmd(
        &[
            "graph",
            "tests/fixtures/go/simple_module",
            "--lang",
            "go",
            "--format",
            "dot",
            "--quiet",
        ],
        None,
    );
    world.output = Some(output);
}

#[when("I run diff with identical refs")]
fn run_diff_same_refs(world: &mut CliWorld) {
    let cwd = Path::new("tests/fixtures/go/diff_repo");
    let output = run_cmd(
        &[
            "diff", "--base", "HEAD", "--head", "HEAD", "--lang", "go", "--format", "text",
            "--quiet",
        ],
        Some(cwd),
    );
    world.output = Some(output);
}

#[when("I run the overall quality report")]
fn run_overall_quality(world: &mut CliWorld) {
    let output = run_cmd(
        &[
            "quality",
            ".",
            "--lang",
            "rust",
            "--metric",
            "overall",
            "--coverage",
            "lcov.info",
            "--format",
            "text",
            "--quiet",
        ],
        None,
    );
    world.output = Some(output);
}

#[when("I run the crap quality report")]
fn run_crap_quality(world: &mut CliWorld) {
    let output = run_cmd(
        &[
            "quality",
            "tests/fixtures/quality",
            "--lang",
            "rust",
            "--metric",
            "crap",
            "--coverage",
            "tests/fixtures/quality/lcov.info",
            "--format",
            "text",
            "--quiet",
        ],
        None,
    );
    world.output = Some(output);
}

#[when("I run the quality report with min cc 2")]
fn run_quality_min_cc(world: &mut CliWorld) {
    let output = run_cmd(
        &[
            "quality",
            "tests/fixtures/quality",
            "--lang",
            "python",
            "--metric",
            "crap",
            "--coverage",
            "tests/fixtures/quality/lcov.info",
            "--format",
            "text",
            "--min-cc",
            "2",
            "--quiet",
        ],
        None,
    );
    world.output = Some(output);
}

#[when("I run config show")]
fn run_config_show(world: &mut CliWorld) {
    let temp_dir = world.temp_dir.as_ref().expect("temp dir");
    let path = temp_dir.path().to_string_lossy().to_string();
    let output = run_cmd(&["config", "show", "--path", &path], None);
    world.output = Some(output);
}

#[then("the analyze report includes summary")]
fn analyze_includes_summary(world: &mut CliWorld) {
    let output = world.output.as_ref().expect("output available");
    assert!(output.status.success(), "command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Untangle Analysis Report"));
    assert!(stdout.contains("Summary"));
}

#[then("the analyze report includes hotspots")]
fn analyze_includes_top(world: &mut CliWorld) {
    let output = world.output.as_ref().expect("output available");
    assert!(output.status.success(), "command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hotspots"));
}

#[then("the analyze output is json")]
fn analyze_output_json(world: &mut CliWorld) {
    let output = world.output.as_ref().expect("output available");
    assert!(output.status.success(), "command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"summary\""));
    assert!(stdout.contains("\"metadata\""));
}

#[then("the graph output is dot")]
fn graph_output_dot(world: &mut CliWorld) {
    let output = world.output.as_ref().expect("output available");
    assert!(output.status.success(), "command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim_start().starts_with("digraph"));
}

#[then("the diff verdict is pass")]
fn diff_verdict_pass(world: &mut CliWorld) {
    let output = world.output.as_ref().expect("output available");
    assert!(output.status.success(), "command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Verdict: Pass"));
}

#[then("the output includes the untangle hotspots section")]
fn output_includes_hotspots(world: &mut CliWorld) {
    let output = world.output.as_ref().expect("output available");
    assert!(output.status.success(), "command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Untangle Hotspots"));
}

#[then("the output includes the crap report table")]
fn output_includes_crap_summary(world: &mut CliWorld) {
    let output = world.output.as_ref().expect("output available");
    assert!(output.status.success(), "command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Metric:    crap"));
    assert!(stdout.contains("Function"));
}

#[then("the output excludes low cc functions")]
fn output_excludes_low_cc(world: &mut CliWorld) {
    let output = world.output.as_ref().expect("output available");
    assert!(output.status.success(), "command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("foo"));
    assert!(!stdout.contains("bar"));
}

#[then("the config output shows defaults")]
fn config_shows_defaults(world: &mut CliWorld) {
    let output = world.output.as_ref().expect("output available");
    assert!(output.status.success(), "command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Loaded config files: (none)"));
    assert!(stdout.contains("rules.high_fanout.min_fanout: 5 <- default"));
}

fn main() {
    futures::executor::block_on(CliWorld::run("tests/features"));
}
