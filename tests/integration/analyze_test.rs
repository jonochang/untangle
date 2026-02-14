use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn analyze_go_simple_module_json() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "tests/fixtures/go/simple_module",
            "--lang",
            "go",
            "--format",
            "json",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"node_count\""))
        .stdout(predicate::str::contains("\"edge_count\""))
        .stdout(predicate::str::contains("\"language\": \"go\""));
}

#[test]
fn analyze_go_simple_module_text() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "tests/fixtures/go/simple_module",
            "--lang",
            "go",
            "--format",
            "text",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Untangle Analysis Report"))
        .stdout(predicate::str::contains("Fan-out:"));
}

#[test]
fn analyze_go_simple_module_dot() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "tests/fixtures/go/simple_module",
            "--lang",
            "go",
            "--format",
            "dot",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph dependencies"));
}

#[test]
fn analyze_go_simple_module_sarif() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "tests/fixtures/go/simple_module",
            "--lang",
            "go",
            "--format",
            "sarif",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("sarif-schema-2.1.0"));
}

#[test]
fn analyze_python_simple_project_json() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "tests/fixtures/python/simple_project",
            "--lang",
            "python",
            "--format",
            "json",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"language\": \"python\""));
}

#[test]
fn analyze_ruby_require_relative_json() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "tests/fixtures/ruby/require_relative",
            "--lang",
            "ruby",
            "--format",
            "json",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"language\": \"ruby\""));
}

#[test]
fn analyze_auto_detect_go() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "tests/fixtures/go/simple_module",
            "--format",
            "json",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"language\": \"go\""));
}

#[test]
fn analyze_nonexistent_path_returns_exit_2() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args(["analyze", "tests/fixtures/nonexistent", "--lang", "go"])
        .assert()
        .failure()
        .code(1); // miette wraps with exit code 1
}

#[test]
fn analyze_with_top_flag() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "tests/fixtures/go/simple_module",
            "--lang",
            "go",
            "--format",
            "json",
            "--top",
            "2",
            "--quiet",
        ])
        .assert()
        .success();
}

#[test]
fn graph_go_dot_output() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "graph",
            "tests/fixtures/go/simple_module",
            "--lang",
            "go",
            "--format",
            "dot",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph dependencies"))
        .stdout(predicate::str::contains("->"));
}

#[test]
fn graph_go_json_output() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "graph",
            "tests/fixtures/go/simple_module",
            "--lang",
            "go",
            "--format",
            "json",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"nodes\""))
        .stdout(predicate::str::contains("\"edges\""));
}

#[test]
fn analyze_rust_simple_crate_json() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "tests/fixtures/rust/simple_crate",
            "--lang",
            "rust",
            "--format",
            "json",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"language\": \"rust\""))
        .stdout(predicate::str::contains("\"node_count\""))
        .stdout(predicate::str::contains("\"edge_count\""));
}
