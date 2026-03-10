use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn analyze_report_go_simple_module_json() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "report",
            "tests/fixtures/go/simple_module",
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
    assert_eq!(json["kind"], "analyze.report");
    assert_eq!(json["schema_version"], 2);
    assert_eq!(json["metadata"]["language"], "go");
    assert!(json["metadata"]["node_count"].is_number());
    assert!(json["metadata"]["edge_count"].is_number());
}

#[test]
fn analyze_report_text_output() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "report",
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
fn analyze_report_sarif_output() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "report",
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
fn analyze_report_auto_detect_go() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "report",
            "tests/fixtures/go/simple_module",
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
}

#[test]
fn analyze_report_nonexistent_path_returns_failure() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args(["analyze", "report", "tests/fixtures/nonexistent", "--lang", "go"])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn analyze_report_with_top_flag() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "report",
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
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(json["hotspots"].as_array().unwrap().len() <= 2);
}

#[test]
fn analyze_report_json_contains_insights_by_default() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "report",
            "tests/fixtures/go/simple_module",
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
    assert!(json.get("insights").is_some());
}

#[test]
fn analyze_report_insights_off_suppresses_key() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "report",
            "tests/fixtures/go/simple_module",
            "--lang",
            "go",
            "--format",
            "json",
            "--quiet",
            "--insights",
            "off",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(json.get("insights").is_none());
}

#[test]
fn analyze_graph_go_dot_output() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
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
fn analyze_graph_go_json_output() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
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
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["kind"], "analyze.graph");
    assert_eq!(json["schema_version"], 2);
    assert!(json["nodes"].is_array());
    assert!(json["edges"].is_array());
}

#[test]
fn analyze_report_go_nested_modules_resolves_imports() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "report",
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
    let edge_count = json["metadata"]["edge_count"].as_u64().unwrap_or(0);
    assert!(edge_count >= 2);
}

#[test]
fn analyze_report_ruby_zeitwerk_resolves_constants() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "report",
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
    let edge_count = json["metadata"]["edge_count"].as_u64().unwrap_or(0);
    assert!(edge_count >= 1);
}

#[test]
fn analyze_report_polyglot_json_has_resolution_counts() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "report",
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
        assert!(lang.get("imports_resolved").is_some());
        assert!(lang.get("imports_unresolved").is_some());
    }
}
