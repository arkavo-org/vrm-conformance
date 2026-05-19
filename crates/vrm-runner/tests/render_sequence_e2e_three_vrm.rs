//! End-to-end sequence dispatch test against the three-vrm adapter.
//!
//! Ignored by default because it requires:
//!   - Node.js on PATH
//!   - `npm install` + `npm run build` already run in `adapters/three-vrm/`
//!   - Playwright Chromium installed (`npx playwright install chromium`)
//!
//! Run locally with:
//!   cd adapters/three-vrm && npm run build && cd ../..
//!   cargo test -p vrm-runner --test render_sequence_e2e_three_vrm -- --ignored
//!
//! This test makes three-vrm the second real renderer that drives the
//! render_sequence path end-to-end through the runner. Assertions cover:
//!   - the dispatch returns SequenceStatus::Ok
//!   - real PNG frames land on disk (non-trivial size)
//!   - the runner re-hashes the adapter's placeholder BLAKE3 sentinel
//!     into real, non-zero hashes

use camino::Utf8PathBuf;
use vrm_asset_generator::emit::emit_vrm;
use vrm_asset_generator::params::MToonParams;
use vrm_asset_generator::sidecar::build_default_test_plan;
use vrm_runner::execute::{execute_plan, ExecuteOptions, SequenceStatus};
use vrm_test_plan::{RenderSequenceBlock, SequenceFormat, SequenceRootTransformAnimation};

fn three_vrm_bin() -> (Utf8PathBuf, Vec<String>) {
    let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap().parent().unwrap();
    let dist_main = workspace_root.join("adapters/three-vrm/dist/main.js");
    assert!(
        dist_main.exists(),
        "three-vrm dist/main.js not built — run `cd adapters/three-vrm && npm run build` first"
    );
    let node = std::process::Command::new("node")
        .arg("--version")
        .output()
        .expect("node must be on PATH");
    assert!(node.status.success(), "node --version failed");
    (Utf8PathBuf::from("node"), vec![dist_main.to_string()])
}

#[test]
#[ignore = "requires node + three-vrm dist build + playwright chromium"]
fn three_vrm_render_sequence_with_animate_root_transform_produces_frames() {
    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = asset_dir.clone();

    let id = "seq_e2e_three_vrm";
    let vrm_path = asset_dir.join(format!("{id}.vrm"));

    let mtoon = MToonParams::defaults(id);
    emit_vrm(&mtoon, &vrm_path).expect("emit_vrm must succeed");

    let mut plan = build_default_test_plan(&mtoon, &format!("{id}.vrm"));
    plan.render_sequence = Some(RenderSequenceBlock {
        frame_count: 2,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        output_format: SequenceFormat::PngSequence,
        animate_root_transform: Some(SequenceRootTransformAnimation {
            translation_start: [0.0, 0.0, 0.0],
            translation_end: [0.1, 0.0, 0.0],
        }),
        apply_vrma: None,
        temporal_ssim_threshold: None,
    });
    plan.animation = None;

    let (adapter_bin, adapter_args) = three_vrm_bin();
    let opts = ExecuteOptions {
        adapter_bin,
        adapter_args,
        asset_dir,
        output_dir,
        renderer_name: "three-vrm-seq".into(),
        emit_progress_ndjson: false,
        reference: None,
        reference_positions: None,
        vrma_path: None,
        apply_at_time: 0.0,
        reference_pose_json: None,
    };

    let result = execute_plan(&plan, &opts)
        .expect("execute_plan against three-vrm in sequence mode must succeed");

    let seq = result
        .sequence
        .expect("sequence mode should populate result.sequence");

    assert_eq!(
        seq.status,
        SequenceStatus::Ok,
        "three-vrm should return Ok now; got {:?}, error_message={:?}, phase={:?}",
        seq.status,
        seq.error_message,
        seq.unimplemented_phase
    );

    let seq_result = seq
        .result
        .expect("RenderSequenceResult should be populated");
    assert_eq!(seq_result.frames.len(), 2, "frame_count was 2");

    let zero_sentinel = format!("blake3:{}", "0".repeat(64));

    for (i, frame) in seq_result.frames.iter().enumerate() {
        assert_eq!(frame.index, i as u32);
        assert!(
            std::path::Path::new(&frame.path).exists(),
            "frame {i} not on disk: {}",
            frame.path
        );
        let bytes = std::fs::read(&frame.path).expect("frame PNG readable");
        assert!(
            bytes.len() > 100,
            "frame {i} PNG too small ({} bytes) — render likely empty",
            bytes.len()
        );
        assert!(frame.blake3.starts_with("blake3:"));
        assert_eq!(frame.blake3.len(), "blake3:".len() + 64);
        assert_ne!(
            frame.blake3, zero_sentinel,
            "frame {i} blake3 is still the adapter's sentinel — runner re-hash didn't run"
        );
    }
}
