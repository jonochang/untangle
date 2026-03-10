use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn service_graph_json_output() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "service-graph",
            "tests/fixtures/monorepo",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["kind"], "service_graph");
    assert_eq!(json["schema_version"], 2);
    assert_eq!(json["services"].as_array().unwrap().len(), 3);
}

#[test]
fn service_graph_detects_cross_service_edges() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "service-graph",
            "tests/fixtures/monorepo",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let edges = json["cross_service_edges"].as_array().unwrap();
    assert!(edges.iter().any(|edge| edge["kind"] == "graphql_query"));
    assert!(edges.iter().any(|edge| edge["kind"] == "rest_call"));
}

#[test]
fn service_graph_dot_output() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "service-graph",
            "tests/fixtures/monorepo",
            "--format",
            "dot",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph service_dependencies"))
        .stdout(predicate::str::contains("user-api"));
}
