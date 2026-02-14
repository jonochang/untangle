use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn diff_go_shows_changes() {
    Command::cargo_bin("untangle")
        .unwrap()
        .current_dir("tests/fixtures/go/diff_repo")
        .args([
            "diff", "--base", "HEAD~1", "--head", "HEAD", "--lang", "go", "--format", "json",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"verdict\""))
        .stdout(predicate::str::contains("\"summary_delta\""));
}

#[test]
fn diff_go_pass_without_fail_on() {
    Command::cargo_bin("untangle")
        .unwrap()
        .current_dir("tests/fixtures/go/diff_repo")
        .args([
            "diff", "--base", "HEAD~1", "--head", "HEAD", "--lang", "go", "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"verdict\": \"pass\""));
}

#[test]
fn diff_go_fail_on_new_edge() {
    Command::cargo_bin("untangle")
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
        .code(1)
        .stdout(predicate::str::contains("\"verdict\": \"fail\""))
        .stdout(predicate::str::contains("new-edge"));
}
