use assert_cmd::cargo::cargo_bin_cmd;
use cucumber::{given, then, when, World};

#[derive(Debug, Default, World)]
struct CliWorld {
    output: Option<std::process::Output>,
}

#[given("the quality fixtures")]
fn quality_fixtures(_world: &mut CliWorld) {}

#[when("I run the overall quality report")]
fn run_overall_quality(world: &mut CliWorld) {
    let mut cmd = cargo_bin_cmd!("untangle");
    let output = cmd
        .args([
            "quality",
            ".",
            "--lang",
            "rust",
            "--metric",
            "overall",
            "--coverage",
            "tests/fixtures/quality/lcov.info",
            "--format",
            "text",
            "--quiet",
        ])
        .output()
        .expect("run untangle quality");
    world.output = Some(output);
}

#[then("the output includes the untangle hotspots section")]
fn output_includes_hotspots(world: &mut CliWorld) {
    let output = world.output.as_ref().expect("output available");
    assert!(output.status.success(), "command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Untangle Hotspots"),
        "expected hotspots section in output"
    );
}

fn main() {
    futures::executor::block_on(CliWorld::run("tests/features/quality"));
}
