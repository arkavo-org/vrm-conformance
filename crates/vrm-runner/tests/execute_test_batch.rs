//! Contract tests for `vrm-runner execute-test-batch`. Tests use mock
//! shell-script fixtures so they run without Unity installed.

use std::path::PathBuf;
use std::process::Command;

fn runner_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_vrm-runner"))
}

#[test]
fn execute_test_batch_subcommand_is_registered() {
    // The subcommand must parse — clap should accept the flag set even
    // if the implementation is a stub. Failing here means the CLI
    // surface doesn't exist yet.
    let out = Command::new(runner_bin())
        .args(["execute-test-batch", "--help"])
        .output()
        .expect("spawn runner");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "execute-test-batch --help should succeed; stdout={stdout} stderr={stderr}"
    );
    assert!(
        stdout.contains("--plans"),
        "help must mention --plans flag; got: {stdout}"
    );
    assert!(
        stdout.contains("--adapter-bin"),
        "help must mention --adapter-bin flag; got: {stdout}"
    );
    assert!(
        stdout.contains("--output-dir"),
        "help must mention --output-dir flag; got: {stdout}"
    );
}
