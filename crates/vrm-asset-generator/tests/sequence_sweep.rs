//! Integration test for the emit-sequence-sweep subcommand.
//! Runs the binary, asserts every emitted triplet is well-formed and that
//! each plan validates (no animation + render_sequence collision).

use camino::Utf8PathBuf;
use vrm_test_plan::{SequenceFormat, TestPlan};

#[test]
fn emit_sequence_sweep_produces_valid_triplets() {
    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args(["emit-sequence-sweep", "--output-dir", out.as_str()])
        .status()
        .expect("asset-generator must be runnable");
    assert!(status.success(), "emit-sequence-sweep exited non-zero");

    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();

    // Each asset = 3 files (.vrm + .meta.json + .test.yaml). Sweep count is
    // determined by spring_bone_basic_sweep() — assert "more than a handful"
    // rather than pinning to a specific number that may evolve.
    let yamls: Vec<_> = entries
        .iter()
        .filter(|f| f.ends_with(".test.yaml"))
        .collect();
    assert!(
        yamls.len() >= 18,
        "expected at least 18 sequence test.yaml files, got {}",
        yamls.len()
    );
    assert_eq!(
        entries.len(),
        yamls.len() * 3,
        "expected {}x3 = {} files (vrm + meta.json + test.yaml per variant), got {}",
        yamls.len(),
        yamls.len() * 3,
        entries.len()
    );

    for yaml_name in &yamls {
        let yaml_path = out.join(yaml_name);
        let raw = std::fs::read_to_string(yaml_path.as_std_path()).unwrap();
        let plan: TestPlan = serde_yml::from_str(&raw)
            .unwrap_or_else(|e| panic!("{yaml_name} failed to parse: {e}"));

        assert!(
            plan.id.starts_with("swing_seq_"),
            "{yaml_name}: id should start with swing_seq_, got {}",
            plan.id
        );
        assert!(
            plan.render_sequence.is_some(),
            "{yaml_name}: render_sequence missing"
        );
        assert!(
            plan.animation.is_none(),
            "{yaml_name}: animation must be absent (mutually exclusive with render_sequence)"
        );
        assert!(
            plan.validate().is_ok(),
            "{yaml_name}: validator rejected: {:?}",
            plan.validate()
        );

        let seq = plan.render_sequence.unwrap();
        assert_eq!(seq.frame_count, 60, "{yaml_name}: frame_count");
        assert!(
            (seq.frame_hz - 30.0).abs() < 1e-6,
            "{yaml_name}: frame_hz {} != 30.0",
            seq.frame_hz
        );
        assert!(
            (seq.physics_dt_seconds - 1.0 / 60.0).abs() < 1e-6,
            "{yaml_name}: physics_dt_seconds {} != 1/60",
            seq.physics_dt_seconds
        );
        assert!(matches!(seq.output_format, SequenceFormat::PngSequence));
        let anim = seq.animate_root_transform.expect("translation required");
        assert_eq!(anim.translation_start, [0.0, 0.0, 0.0]);
        assert_eq!(anim.translation_end, [0.15, 0.0, 0.0]);
    }
}
