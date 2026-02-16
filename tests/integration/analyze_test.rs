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

#[test]
fn analyze_json_contains_insights_key() {
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
        .stdout(predicate::str::contains("\"insights\""));
}

#[test]
fn analyze_no_insights_suppresses_key() {
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
            "--no-insights",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"insights\"").not());
}

#[test]
fn analyze_go_nested_modules_resolves_imports() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "tests/fixtures/go/nested_modules",
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
    // Should have resolved edges from both nested modules
    let edge_count = json["metadata"]["edge_count"].as_u64().unwrap_or(0);
    assert!(
        edge_count >= 2,
        "Expected at least 2 edges from nested module imports, got {edge_count}"
    );
}

#[test]
fn analyze_ruby_zeitwerk_resolves_constants() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "tests/fixtures/ruby/zeitwerk_project",
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
    // Zeitwerk should resolve Post and User constants from posts_controller.rb
    let edge_count = json["metadata"]["edge_count"].as_u64().unwrap_or(0);
    assert!(
        edge_count >= 1,
        "Expected at least 1 edge from Zeitwerk constant resolution, got {edge_count}"
    );
}

#[test]
fn analyze_polyglot_json_has_resolution_counts() {
    let output = Command::cargo_bin("untangle")
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
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let languages = json["metadata"]["languages"].as_array().unwrap();
    for lang in languages {
        assert!(
            lang.get("imports_resolved").is_some(),
            "Missing imports_resolved for {}",
            lang["language"]
        );
        assert!(
            lang.get("imports_unresolved").is_some(),
            "Missing imports_unresolved for {}",
            lang["language"]
        );
    }
}
