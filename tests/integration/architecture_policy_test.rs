use assert_cmd::Command;
use predicates::prelude::*;
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
fn architecture_check_reports_allowlist_violation_in_json() {
    let src = fixture_path("python/simple_project");
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("project");
    std::fs::create_dir_all(&dest).unwrap();
    copy_dir_recursive(&src, &dest);
    std::fs::write(
        dest.join(".untangle.toml"),
        r#"
[analyze.architecture]
level = 1
check_format = "json"
fail_on_violations = true
fail_on_cycles = true

[analyze.architecture.allowed_dependencies]
api = ["utils"]
db = []
utils = []
"#,
    )
    .unwrap();

    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "architecture-check",
            dest.to_str().unwrap(),
            "--lang",
            "python",
            "--format",
            "json",
            "--quiet",
        ])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["kind"], "analyze.architecture.check");
    assert_eq!(json["schema_version"], 2);
    assert_eq!(json["summary"]["verdict"], "fail");
    assert_eq!(json["summary"]["violation_count"], 1);
    assert_eq!(json["violations"][0]["from"], "api");
    assert_eq!(json["violations"][0]["to"], "db");
    assert_eq!(json["violations"][0]["kind"], "allowlist");
}

#[test]
fn architecture_init_writes_starter_policy_and_requires_force_to_replace() {
    let src = fixture_path("python/simple_project");
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("project");
    std::fs::create_dir_all(&dest).unwrap();
    copy_dir_recursive(&src, &dest);

    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "architecture-init",
            dest.to_str().unwrap(),
            "--lang",
            "python",
            "--level",
            "1",
            "--quiet",
        ])
        .assert()
        .success();

    let config = std::fs::read_to_string(dest.join(".untangle.toml")).unwrap();
    assert!(config.contains("[analyze.architecture.allowed_dependencies]"));
    assert!(config.contains("api = [\"db\", \"utils\"]"));
    assert!(config.contains("db = []"));
    assert!(config.contains("utils = []"));

    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "architecture-init",
            dest.to_str().unwrap(),
            "--lang",
            "python",
            "--level",
            "1",
            "--quiet",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Architecture policy already exists",
        ));
}
