use assert_cmd::Command;

#[test]
fn exit_code_0_on_pass() {
    Command::cargo_bin("untangle")
        .unwrap()
        .current_dir("tests/fixtures/go/diff_repo")
        .args([
            "diff", "--base", "HEAD~1", "--head", "HEAD", "--lang", "go", "--quiet",
        ])
        .assert()
        .code(0);
}

#[test]
fn exit_code_0_when_fail_on_has_no_structural_match() {
    Command::cargo_bin("untangle")
        .unwrap()
        .current_dir("tests/fixtures/go/diff_repo")
        .args([
            "diff",
            "--base",
            "HEAD~1",
            "--head",
            "HEAD",
            "--lang",
            "go",
            "--fail-on",
            "new-edge",
            "--quiet",
        ])
        .assert()
        .code(0);
}

#[test]
fn exit_code_1_on_bad_path() {
    // miette wraps errors with exit code 1
    Command::cargo_bin("untangle")
        .unwrap()
        .args(["analyze", "report", "/nonexistent/path", "--lang", "go"])
        .assert()
        .failure();
}

#[test]
fn analyze_exit_code_0_clean_project() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args([
            "analyze",
            "report",
            "tests/fixtures/go/simple_module",
            "--lang",
            "go",
            "--quiet",
        ])
        .assert()
        .code(0);
}
