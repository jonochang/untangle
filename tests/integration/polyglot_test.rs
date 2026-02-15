use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn analyze_polyglot_auto_detect() {
    // When --lang is omitted on a mixed Go/Python/Ruby fixture,
    // all three languages should be detected and reported.
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "tests/fixtures/polyglot",
            "--format",
            "json",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"languages\""))
        .stdout(predicate::str::contains("\"go\""))
        .stdout(predicate::str::contains("\"python\""))
        .stdout(predicate::str::contains("\"ruby\""));
}

#[test]
fn analyze_polyglot_explicit_single_lang() {
    // When --lang go is specified, only Go files should be analyzed.
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "tests/fixtures/polyglot",
            "--lang",
            "go",
            "--format",
            "json",
            "--quiet",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["metadata"]["language"], "go");
    // languages array should not be present for single-language
    assert!(json["metadata"]["languages"].is_null());
}

#[test]
fn analyze_polyglot_text_output() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "tests/fixtures/polyglot",
            "--format",
            "text",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Languages:"))
        .stdout(predicate::str::contains("go"))
        .stdout(predicate::str::contains("python"))
        .stdout(predicate::str::contains("ruby"));
}

#[test]
fn analyze_polyglot_dot_output() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "tests/fixtures/polyglot",
            "--format",
            "dot",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph dependencies"))
        // DOT output should have language-colored nodes
        .stdout(predicate::str::contains("fillcolor="));
}

#[test]
fn graph_polyglot_auto_detect() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "graph",
            "tests/fixtures/polyglot",
            "--format",
            "dot",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph dependencies"));
}
