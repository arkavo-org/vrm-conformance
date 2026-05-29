//! End-to-end: `emit-material-name-classification-sweep` emits the 6-variant
//! material-name render-classification corpus (the VMK `Vita_clothing`
//! z-fighting reproducer). Each variant is one MToon material under a
//! heuristic-tripping or control name; conformant renderers must produce
//! identical output at a fixed `doubleSided`.
//! Spec: docs/superpowers/specs/2026-05-28-material-name-classification-reproducer.md

use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args(args)
        .output()
        .expect("run vrm-asset-generator")
}

#[test]
fn emit_material_name_classification_sweep_produces_six_triplets() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("matname");
    let out_str = out.to_str().unwrap();

    let status = run(&[
        "emit-material-name-classification-sweep",
        "--output-dir",
        out_str,
    ]);
    assert!(
        status.status.success(),
        "emit-material-name-classification-sweep should exit 0; stderr: {}",
        String::from_utf8_lossy(&status.stderr)
    );

    let vrm_count = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "vrm"))
        .count();
    assert_eq!(vrm_count, 6, "expected 6 .vrm assets");
}

#[test]
fn clothing_variant_material_name_trips_heuristic_but_is_single_sided() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("matname");
    let out_str = out.to_str().unwrap();
    run(&[
        "emit-material-name-classification-sweep",
        "--output-dir",
        out_str,
    ]);

    // The clothing single-sided variant's emitted glTF material must (a) carry
    // a name containing "cloth" so a name-heuristic renderer trips on it, and
    // (b) declare doubleSided = false so a conformant renderer single-sides it.
    let vrm = out.join("matname_clothing_singlesided.vrm");
    let bytes = std::fs::read(&vrm).expect("clothing variant .vrm exists");
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("matname_clothing_singlesided"),
        "material name must be present in the glTF JSON"
    );
    // The generator always emits an explicit `"doubleSided": <bool>` on the
    // material (emit.rs:228/328). This single-sided variant must emit `false`,
    // and must NOT emit `true` — that's the declared-spec value a conformant
    // renderer honors and a name-heuristic renderer (VMK) overrides.
    assert!(
        !text.contains("\"doubleSided\":true") && !text.contains("\"doubleSided\": true"),
        "single-sided clothing variant must NOT declare doubleSided=true"
    );
}
