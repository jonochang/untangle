use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn quality_crap_rust_json() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "tests/fixtures/quality",
            "--lang",
            "rust",
            "--metric",
            "crap",
            "--coverage",
            "tests/fixtures/quality/lcov.info",
            "--format",
            "json",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"metric\": \"crap\""))
        .stdout(predicate::str::contains("\"function\": \"simple\""));
}

#[test]
fn quality_min_cc_filters() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "tests/fixtures/quality",
            "--lang",
            "python",
            "--metric",
            "crap",
            "--coverage",
            "tests/fixtures/quality/lcov.info",
            "--format",
            "json",
            "--min-cc",
            "2",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"function\": \"foo\""))
        .stdout(predicate::str::contains("\"function\": \"bar\"").not());
}

#[test]
fn quality_overall_rust_json() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "tests/fixtures/quality",
            "--lang",
            "rust",
            "--metric",
            "overall",
            "--coverage",
            "tests/fixtures/quality/lcov.info",
            "--format",
            "json",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"metric\": \"overall\""))
        .stdout(predicate::str::contains("\"untangle\""))
        .stdout(predicate::str::contains("\"crap\""));
}
