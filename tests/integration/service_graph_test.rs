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

    // Should list all 3 services
    let services = json["services"].as_array().unwrap();
    assert_eq!(services.len(), 3);

    let service_names: Vec<&str> = services
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(service_names.contains(&"user-api"));
    assert!(service_names.contains(&"post-api"));
    assert!(service_names.contains(&"web-frontend"));
}

#[test]
fn service_graph_detects_graphql_dependency() {
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

    // web-frontend should have a graphql_query edge to user-api
    let gql_edge = edges.iter().find(|e| {
        e["from_service"] == "web-frontend"
            && e["to_service"] == "user-api"
            && e["kind"] == "graphql_query"
    });
    assert!(
        gql_edge.is_some(),
        "Expected GraphQL edge from web-frontend to user-api, got edges: {edges:?}"
    );
}

#[test]
fn service_graph_detects_rest_dependency() {
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

    // web-frontend should have a rest_call edge to post-api
    let rest_edge = edges.iter().find(|e| {
        e["from_service"] == "web-frontend"
            && e["to_service"] == "post-api"
            && e["kind"] == "rest_call"
    });
    assert!(
        rest_edge.is_some(),
        "Expected REST edge from web-frontend to post-api, got edges: {edges:?}"
    );
}

#[test]
fn service_graph_text_output() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "service-graph",
            "tests/fixtures/monorepo",
            "--format",
            "text",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Service Graph"))
        .stdout(predicate::str::contains("user-api"))
        .stdout(predicate::str::contains("post-api"))
        .stdout(predicate::str::contains("web-frontend"));
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
        .stdout(predicate::str::contains("user-api"))
        .stdout(predicate::str::contains("post-api"))
        .stdout(predicate::str::contains("web-frontend"));
}

#[test]
fn service_graph_fails_without_services_config() {
    // Running service-graph on a directory without [services] should fail
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "service-graph",
            "tests/fixtures/polyglot",
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No [services] configured"));
}
