use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn config_show_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("untangle").unwrap();
    cmd.args(["config", "show", tmp.path().to_str().unwrap()]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Loaded config files: (none)"))
        .stdout(predicate::str::contains("Resolved settings:"))
        .stdout(predicate::str::contains(
            "analyze.report.format: json <- default",
        ))
        .stdout(predicate::str::contains("defaults.quiet: false <- default"))
        .stdout(predicate::str::contains(
            "rules.high_fanout.min_fanout: 5 <- default",
        ));
}

#[test]
fn config_show_with_project_config() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join(".untangle.toml"),
        r#"
[analyze.report]
format = "text"

[defaults]
quiet = true

[rules.high_fanout]
min_fanout = 10
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("untangle").unwrap();
    cmd.args(["config", "show", tmp.path().to_str().unwrap()]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(".untangle.toml"))
        .stdout(predicate::str::contains(
            "analyze.report.format: text <- project config",
        ))
        .stdout(predicate::str::contains(
            "defaults.quiet: true <- project config",
        ))
        .stdout(predicate::str::contains(
            "rules.high_fanout.min_fanout: 10 <- project config",
        ));
}

#[test]
fn config_explain_high_fanout() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join(".untangle.toml"),
        r#"
[rules.high_fanout]
min_fanout = 20
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("untangle").unwrap();
    cmd.args([
        "config",
        "explain",
        "high_fanout",
        tmp.path().to_str().unwrap(),
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Rule: high_fanout"))
        .stdout(predicate::str::contains(
            "rules.high_fanout.enabled: true <- default",
        ))
        .stdout(predicate::str::contains(
            "rules.high_fanout.min_fanout: 20 <- project config",
        ));
}

#[test]
fn config_explain_unknown_category() {
    let tmp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("untangle").unwrap();
    cmd.args([
        "config",
        "explain",
        "nonexistent",
        tmp.path().to_str().unwrap(),
    ]);
    cmd.assert().success().stdout(predicate::str::contains(
        "Unknown rule category: nonexistent",
    ));
}

#[test]
fn config_explain_architecture_policy() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join(".untangle.toml"),
        r#"
[analyze.architecture]
level = 2
check_format = "json"
fail_on_violations = false

[analyze.architecture.allowed_dependencies]
api = ["db"]
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("untangle").unwrap();
    cmd.args([
        "config",
        "explain",
        "architecture_policy",
        tmp.path().to_str().unwrap(),
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Rule: architecture_policy"))
        .stdout(predicate::str::contains(
            "analyze.architecture.level: 2 <- project config",
        ))
        .stdout(predicate::str::contains(
            "analyze.architecture.check_format: json <- project config",
        ))
        .stdout(predicate::str::contains(
            "analyze.architecture.allowed_dependencies: {\"api\": [\"db\"]} <- project config",
        ));
}

#[test]
fn analyze_report_respects_config_insights() {
    let go_fixture = fixture_path("go/simple_module");
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("project");
    std::fs::create_dir_all(&dest).unwrap();
    copy_dir_recursive(&go_fixture, &dest);
    std::fs::write(
        dest.join(".untangle.toml"),
        r#"
[analyze.report]
insights = "off"
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("untangle").unwrap();
    cmd.args([
        "analyze",
        "report",
        dest.to_str().unwrap(),
        "--lang",
        "go",
        "--quiet",
    ]);
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(json.get("insights").is_none());
}

#[test]
fn cli_flag_overrides_config_format() {
    let go_fixture = fixture_path("go/simple_module");
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("project");
    std::fs::create_dir_all(&dest).unwrap();
    copy_dir_recursive(&go_fixture, &dest);

    std::fs::write(
        dest.join(".untangle.toml"),
        r#"
[analyze.report]
format = "text"
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("untangle").unwrap();
    cmd.args([
        "analyze",
        "report",
        dest.to_str().unwrap(),
        "--lang",
        "go",
        "--format",
        "json",
        "--quiet",
    ]);
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let _: serde_json::Value = serde_json::from_str(&stdout).unwrap();
}

#[test]
fn insights_flag_overrides_config() {
    let go_fixture = fixture_path("go/simple_module");
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("project");
    std::fs::create_dir_all(&dest).unwrap();
    copy_dir_recursive(&go_fixture, &dest);

    std::fs::write(
        dest.join(".untangle.toml"),
        r#"
[analyze.report]
insights = "on"
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("untangle").unwrap();
    cmd.args([
        "analyze",
        "report",
        dest.to_str().unwrap(),
        "--lang",
        "go",
        "--quiet",
        "--insights",
        "off",
    ]);
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(json.get("insights").is_none());
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) {
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
