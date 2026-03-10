use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn analyze_architecture_json_output() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "architecture",
            "tests/fixtures/python/simple_project",
            "--lang",
            "python",
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
    assert_eq!(json["kind"], "analyze.architecture");
    assert_eq!(json["schema_version"], 2);

    let node_ids: Vec<&str> = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["id"].as_str().unwrap())
        .collect();
    assert_eq!(node_ids, vec!["api", "db", "utils"]);
}

#[test]
fn analyze_architecture_level_two_expands_children() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "architecture",
            "tests/fixtures/ruby/require_relative",
            "--lang",
            "ruby",
            "--format",
            "json",
            "--level",
            "2",
            "--quiet",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let node_ids: Vec<&str> = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["id"].as_str().unwrap())
        .collect();
    assert_eq!(node_ids, vec!["helper", "main", "utils"]);
}

#[test]
fn analyze_architecture_dot_output() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "architecture",
            "tests/fixtures/python/circular",
            "--lang",
            "python",
            "--format",
            "dot",
            "--quiet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph architecture"))
        .stdout(predicate::str::contains("rankdir=TB"))
        .stdout(predicate::str::contains("color=firebrick"));
}

#[test]
fn deprecated_architecture_alias_still_works() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "architecture",
            "tests/fixtures/python/simple_project",
            "--lang",
            "python",
            "--format",
            "json",
            "--quiet",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("deprecated"));
}
