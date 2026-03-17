use assert_cmd::Command;

#[test]
fn analyze_report_rust_workspace_root_resolves_internal_and_cross_crate_edges() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "report",
            "tests/fixtures/rust/workspace_simple",
            "--lang",
            "rust",
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
    let node_count = json["metadata"]["node_count"].as_u64().unwrap_or(0);
    let edge_count = json["metadata"]["edge_count"].as_u64().unwrap_or(0);
    let unresolved = json["metadata"]["unresolved_imports"]
        .as_u64()
        .unwrap_or(u64::MAX);

    assert!(
        node_count >= 3,
        "expected workspace nodes, got {node_count}"
    );
    assert!(
        edge_count >= 2,
        "expected workspace edges, got {edge_count}"
    );
    assert!(
        unresolved <= 1,
        "expected low unresolved imports, got {unresolved}"
    );

    let hotspots = json["hotspots"].as_array().unwrap();
    let has_cross_crate = hotspots.iter().any(|hotspot| {
        hotspot["node"] == "b.src.lib"
            && hotspot["fanout_edges"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|edge| edge["to"] == "a.src.foo")
    });
    let has_internal = hotspots.iter().any(|hotspot| {
        hotspot["node"] == "a.src.foo"
            && hotspot["fanout_edges"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|edge| edge["to"] == "a.src.nested")
    });

    assert!(has_cross_crate, "missing cross-crate edge");
    assert!(has_internal, "missing crate-local edge");
}
