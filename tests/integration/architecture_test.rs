use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn architecture_json_output() {
    let output = Command::cargo_bin("untangle")
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
    assert_eq!(node_ids, vec!["api", "db", "utils"]);

    let edge = json["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|edge| edge["from"] == "api" && edge["to"] == "db")
        .unwrap();
    assert_eq!(edge["count"], 1);
}

#[test]
fn architecture_level_two_expands_children() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
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
fn architecture_dot_output() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
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
