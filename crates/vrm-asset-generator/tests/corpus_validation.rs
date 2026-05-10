//! Slow integration test: emits the full MToon sweep into a temp dir
//! and runs every .vrm through the validator. Marked `#[ignore]` so it
//! doesn't run on every `cargo test`; CI runs it explicitly via
//! `cargo test --ignored corpus_validation`.

use camino::Utf8PathBuf;
use vrm_asset_generator::{emit::emit_with_sidecars, sweep::mtoon_basic_sweep};
use vrm_validator_wrap::{validate, ValidatorConfig};

#[test]
#[ignore = "slow; run via cargo test -- --ignored"]
fn full_sweep_validates_clean() {
    // Match the skip-gracefully convention used by vrm-validator-wrap's tests:
    // CI may run from the workspace root while cargo's cwd is the crate dir,
    // so the default `.tools/vrm-validator-cli` relative path may not resolve
    // even when the shim is installed. Skip cleanly and tell the operator how
    // to retry; do NOT panic.
    let cfg = match ValidatorConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "SKIP: validator shim not reachable ({e}). Set VRM_VALIDATOR_BIN to an absolute path \
                 (e.g. VRM_VALIDATOR_BIN=$(git rev-parse --show-toplevel)/.tools/vrm-validator-cli) \
                 or run scripts/install-validator.sh from the workspace root."
            );
            return;
        }
    };

    let dir = tempfile::tempdir().unwrap();
    let out_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();

    let mut failures = Vec::new();
    for (i, p) in mtoon_basic_sweep().iter().enumerate() {
        let stem = out_dir.join(&p.id);
        if let Err(e) = emit_with_sidecars(p, &stem) {
            failures.push((p.id.clone(), format!("emission failed: {e}")));
            continue;
        }
        let vrm = stem.with_extension("vrm");

        let report = match validate(&cfg, &vrm) {
            Ok(r) => r,
            Err(e) => {
                failures.push((p.id.clone(), format!("validator error: {e}")));
                continue;
            }
        };
        if report.issues.num_errors > 0 {
            let summary = report
                .issues
                .messages
                .iter()
                .filter(|m| m.severity == 0)
                .map(|m| format!("{}: {}", m.code, m.message))
                .collect::<Vec<_>>()
                .join("; ");
            failures.push((p.id.clone(), summary));
        } else {
            eprintln!("[{:3}] {} OK", i, p.id);
        }
    }

    if !failures.is_empty() {
        for (id, msg) in &failures {
            eprintln!("FAIL: {id}: {msg}");
        }
        panic!("{} of ~50 assets failed validation", failures.len());
    }
}
