use assert_cmd::Command;
use std::path::{Path, PathBuf};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        if path.is_dir() {
            std::fs::create_dir_all(&dest_path).unwrap();
            copy_dir_recursive(&path, &dest_path);
        } else {
            std::fs::copy(&path, &dest_path).unwrap();
        }
    }
}

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
    assert_eq!(json["schema_version"], 4);

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
    for result in json["report"]["functions"]["results"].as_array().unwrap() {
        assert_eq!(result["coverage_pct"], serde_json::Value::Null);
    }
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

#[test]
fn quality_report_text_uses_na_coverage_without_coverage_file() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "report",
            "tests/fixtures/quality_report",
            "--lang",
            "python",
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
    assert!(text.contains("cov=N/A"));
    assert!(text.contains("with N/A coverage."));
}

#[test]
fn quality_report_json_includes_architecture_component_metrics_and_policy_summary() {
    let src = fixture_path("quality_report");
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("project");
    std::fs::create_dir_all(&dest).unwrap();
    copy_dir_recursive(&src, &dest);
    std::fs::write(
        dest.join(".untangle.toml"),
        r#"
[analyze.architecture]
level = 1

[analyze.architecture.allowed_dependencies]
api = ["db"]
service = ["api", "db"]
db = []
"#,
    )
    .unwrap();

    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "report",
            dest.to_str().unwrap(),
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
    let architecture = &json["report"]["architecture"];
    assert!(architecture["component_metrics"].as_array().unwrap().len() >= 3);
    assert!(architecture["cycles"].as_array().unwrap().len() >= 1);
    assert_eq!(architecture["policy"]["verdict"], "fail");
    assert_eq!(architecture["policy"]["violation_count"], 1);
    assert_eq!(architecture["policy"]["top_violations"][0]["from"], "api");
    assert_eq!(architecture["policy"]["top_violations"][0]["to"], "service");
}
