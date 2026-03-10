use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn quality_functions_rust_json() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "functions",
            "tests/fixtures/quality",
            "--lang",
            "rust",
            "--metric",
            "crap",
            "--coverage",
            "tests/fixtures/quality/lcov.info",
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
    assert_eq!(json["kind"], "quality.functions");
    assert_eq!(json["schema_version"], 2);
    assert_eq!(json["report"]["metadata"]["metric"], "crap");
    assert!(json["report"]["results"].to_string().contains("\"simple\""));
}

#[test]
fn quality_functions_min_cc_filters() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "functions",
            "tests/fixtures/quality",
            "--lang",
            "python",
            "--metric",
            "crap",
            "--coverage",
            "tests/fixtures/quality/lcov.info",
            "--format",
            "json",
            "--min-cc",
            "2",
            "--quiet",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let report_str = json["report"]["results"].to_string();
    assert!(report_str.contains("\"foo\""));
    assert!(!report_str.contains("\"bar\""));
}

#[test]
fn quality_project_rust_json() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "project",
            "tests/fixtures/quality",
            "--lang",
            "rust",
            "--coverage",
            "tests/fixtures/quality/lcov.info",
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
    assert_eq!(json["kind"], "quality.project");
    assert_eq!(json["schema_version"], 2);
    assert_eq!(json["report"]["metadata"]["metric"], "overall");
    assert!(json["report"]["overall"]["untangle"].is_object());
    assert!(json["report"]["overall"]["crap"].is_object());
}

#[test]
fn quality_functions_rejects_overall_metric() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "functions",
            "tests/fixtures/quality",
            "--lang",
            "rust",
            "--metric",
            "overall",
            "--coverage",
            "tests/fixtures/quality/lcov.info",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("supports only function-level"));
}

#[test]
fn quality_functions_complexity_works_without_coverage() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "quality",
            "functions",
            "tests/fixtures/quality",
            "--lang",
            "rust",
            "--metric",
            "complexity",
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
    assert_eq!(json["kind"], "quality.functions");
    assert_eq!(json["report"]["metadata"]["metric"], "complexity");
    assert_eq!(
        json["report"]["metadata"]["coverage_file"],
        serde_json::Value::Null
    );
    assert!(json["report"]["results"].to_string().contains("\"simple\""));
}
