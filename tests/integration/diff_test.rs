use assert_cmd::Command;
use predicates::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        if path.is_dir() {
            std::fs::create_dir_all(&dest_path).unwrap();
            copy_dir_recursive(&path, &dest_path);
        } else {
            std::fs::copy(&path, &dest_path).unwrap();
        }
    }
}

fn git(dir: &Path, args: &[&str]) {
    let status = ProcessCommand::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {:?} failed", args);
}

#[test]
fn diff_go_shows_changes() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .current_dir("tests/fixtures/go/diff_repo")
        .args([
            "diff", "--base", "HEAD~1", "--head", "HEAD", "--lang", "go", "--format", "json",
            "--quiet",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["kind"], "diff.report");
    assert_eq!(json["schema_version"], 3);
    assert!(json["report"]["summary_delta"].is_object());
    assert!(json["report"]["verdict"].is_string());
    assert!(json["report"]["comparison"]["verdict"].is_string());
}

#[test]
fn diff_go_pass_without_fail_on() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .current_dir("tests/fixtures/go/diff_repo")
        .args([
            "diff", "--base", "HEAD~1", "--head", "HEAD", "--lang", "go", "--quiet",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["kind"], "diff.report");
    assert_eq!(json["report"]["verdict"], "pass");
}

#[test]
fn diff_text_output_includes_summary_sections() {
    Command::cargo_bin("untangle")
        .unwrap()
        .current_dir("tests/fixtures/go/diff_repo")
        .args([
            "diff", "--base", "HEAD", "--head", "HEAD", "--lang", "go", "--format", "text",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Untangle Diff Report"))
        .stdout(predicate::str::contains("Comparison: unchanged"))
        .stdout(predicate::str::contains("Summary"))
        .stdout(predicate::str::contains("Verdict: Pass"))
        .stdout(predicate::str::contains("Completed in"));
}

#[test]
fn diff_go_fail_on_new_edge_still_passes_without_structural_changes() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .current_dir("tests/fixtures/go/diff_repo")
        .args([
            "diff",
            "--base",
            "HEAD~1",
            "--head",
            "HEAD",
            "--lang",
            "go",
            "--fail-on",
            "new-edge",
            "--quiet",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["report"]["verdict"], "pass");
    assert!(json["report"]["reasons"].as_array().unwrap().is_empty());
}

#[test]
fn diff_rejects_sarif_output() {
    Command::cargo_bin("untangle")
        .unwrap()
        .current_dir("tests/fixtures/go/diff_repo")
        .args([
            "diff", "--base", "HEAD~1", "--head", "HEAD", "--lang", "go", "--format", "sarif",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

#[test]
fn diff_can_fail_on_new_architecture_violation() {
    let src = fixture_path("python/simple_project");
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("repo");
    std::fs::create_dir_all(&dest).unwrap();
    copy_dir_recursive(&src, &dest);
    std::fs::write(
        dest.join(".untangle.toml"),
        r#"
[analyze.architecture]
level = 1

[analyze.architecture.allowed_dependencies]
api = ["utils"]
db = []
utils = []
"#,
    )
    .unwrap();

    std::fs::write(
        dest.join("src/api/handler.py"),
        r#"from src.utils import logging

def handle():
    logging.info("handled")
"#,
    )
    .unwrap();

    git(&dest, &["init"]);
    git(&dest, &["config", "user.email", "test@example.com"]);
    git(&dest, &["config", "user.name", "Test User"]);
    git(&dest, &["add", "."]);
    git(&dest, &["commit", "-m", "base"]);

    std::fs::write(
        dest.join("src/api/handler.py"),
        r#"from src.db import connection
from src.utils import logging

def handle():
    connection.query()
    logging.info("handled")
"#,
    )
    .unwrap();
    git(&dest, &["add", "."]);
    git(&dest, &["commit", "-m", "head"]);

    let output = Command::cargo_bin("untangle")
        .unwrap()
        .current_dir(&dest)
        .args([
            "diff",
            "--base",
            "HEAD~1",
            "--head",
            "HEAD",
            "--lang",
            "python",
            "--format",
            "json",
            "--fail-on",
            "new-architecture-violation",
            "--quiet",
        ])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["report"]["verdict"], "fail");
    assert_eq!(json["report"]["comparison"]["verdict"], "worse");
    assert_eq!(json["report"]["reasons"][0], "new-architecture-violation");
    assert_eq!(
        json["report"]["architecture_policy_delta"]["new_violations"][0]["from"],
        "api"
    );
    assert_eq!(
        json["report"]["architecture_policy_delta"]["new_violations"][0]["to"],
        "db"
    );
}
