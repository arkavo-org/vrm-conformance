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
    let cfg = ValidatorConfig::from_env().expect("install validator first");

    let dir = tempfile::tempdir().unwrap();
    let out_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();

    let mut failures = Vec::new();
    for (i, p) in mtoon_basic_sweep().iter().enumerate() {
        let stem = out_dir.join(&p.id);
        emit_with_sidecars(p, &stem).expect("emission must succeed");
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
        }
        eprintln!("[{:3}] {} OK", i, p.id);
    }

    if !failures.is_empty() {
        for (id, msg) in &failures {
            eprintln!("FAIL: {id}: {msg}");
        }
        panic!("{} of ~50 assets failed validation", failures.len());
    }
}
