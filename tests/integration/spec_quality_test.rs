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
fn quality_specs_json_reports_cross_language_cases() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "specs",
            "tests/fixtures/spec_quality",
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
    assert_eq!(json["kind"], "quality.specs");
    assert_eq!(json["schema_version"], 1);
    let report = &json["report"];
    let languages = report["metadata"]["languages"].as_array().unwrap();
    assert!(languages.iter().any(|lang| lang == "python"));
    assert!(languages.iter().any(|lang| lang == "ruby"));
    assert!(languages.iter().any(|lang| lang == "go"));
    assert!(languages.iter().any(|lang| lang == "rust"));
    assert!(report["files"].as_array().unwrap().len() >= 4);
    assert!(report["files"][0]["guidance"].is_object());
    assert!(report["worst_cases"].as_array().unwrap().len() >= 1);
}

#[test]
fn quality_specs_text_includes_guidance_and_locations() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "specs",
            "tests/fixtures/spec_quality",
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
    assert!(text.contains("Untangle Spec Quality Report"));
    assert!(text.contains("remediation-mode:"));
    assert!(text.contains("ai-guidance:"));
    assert!(text.contains("worst-examples:"));
    assert!(text.contains("tests/test_api.py") || text.contains("python/tests/test_api.py"));
}

#[test]
fn quality_specs_can_write_and_compare_baseline() {
    let src = fixture_path("spec_quality");
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("project");
    std::fs::create_dir_all(&dest).unwrap();
    copy_dir_recursive(&src, &dest);

    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "specs",
            dest.to_str().unwrap(),
            "--format",
            "json",
            "--write-baseline",
            "--quiet",
        ])
        .assert()
        .success();

    let baseline = dest.join("target/untangle/specs.json");
    assert!(baseline.exists());

    std::fs::write(
        dest.join("python/tests/test_api.py"),
        r#"from unittest import TestCase

class TestApi(TestCase):
    def test_handles_error_paths(self):
        if True:
            if True:
                if True:
                    assert True
"#,
    )
    .unwrap();

    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "specs",
            dest.to_str().unwrap(),
            "--format",
            "json",
            "--compare",
            baseline.to_str().unwrap(),
            "--quiet",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(json["report"]["comparison"].is_object());
    assert_ne!(json["report"]["comparison"]["verdict"], "unchanged");
}
