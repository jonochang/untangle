use assert_cmd::Command;
use predicates::prelude::*;

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
    assert_eq!(json["schema_version"], 2);
    assert!(json["report"]["summary_delta"].is_object());
    assert!(json["report"]["verdict"].is_string());
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
