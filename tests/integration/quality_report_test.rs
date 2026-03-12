use assert_cmd::Command;

#[test]
fn quality_report_json_includes_unified_sections() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "report",
            "tests/fixtures/quality_report",
            "--lang",
            "python",
            "--coverage",
            "tests/fixtures/quality_report/lcov.info",
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
    assert_eq!(json["kind"], "quality.report");
    assert_eq!(json["schema_version"], 3);

    let report = &json["report"];
    assert!(report["structural"]["summary"].is_object());
    assert!(report["structural"]["hotspots"].as_array().unwrap().len() >= 2);
    assert_eq!(report["functions"]["metadata"]["metric"], "crap");
    assert!(report["functions"]["results"]
        .to_string()
        .contains("\"handle\""));
    assert!(report["architecture"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|node| node["id"] == "api"));
    assert!(!report["architecture"]["feedback_edges"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(report["architecture"]["dot"]
        .as_str()
        .unwrap()
        .contains("digraph architecture"));
    assert!(!report["priorities"].as_array().unwrap().is_empty());
}

#[test]
fn quality_report_without_coverage_falls_back_to_complexity() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "report",
            "tests/fixtures/quality_report",
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
    assert_eq!(json["kind"], "quality.report");
    assert_eq!(
        json["report"]["functions"]["metadata"]["metric"],
        "complexity"
    );
    assert_eq!(
        json["report"]["functions"]["metadata"]["coverage_file"],
        serde_json::Value::Null
    );
}

#[test]
fn quality_report_text_includes_priority_locations_and_evidence() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "report",
            "tests/fixtures/quality_report",
            "--lang",
            "python",
            "--coverage",
            "tests/fixtures/quality_report/lcov.info",
            "--format",
            "text",
            "--quiet",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("1. [architecture] Break architecture feedback from api to service"));
    assert!(text.contains("2. [function] Reduce crap score in handle"));
    assert!(text.contains("3. [structural] Reduce module fan-out in src.api.handler"));
    assert!(text.contains("Location: src/api/handler.py::handle"));
    assert!(text.contains("Evidence:"));
    assert!(text.contains("Lines 5-17 in src/api/handler.py."));
    assert!(text.contains("Components: api, service"));
    assert!(text.contains("Architecture"));
    assert!(text.contains("Components: 3  Edges: 4  Feedback: 1  Layers: 3"));
    assert!(text.contains("Feedback edges:"));
    assert!(text.contains("api -> service"));
}
