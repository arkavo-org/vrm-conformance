//! End-to-end sequence dispatch test against the VMK Swift adapter.
//!
//! Ignored by default because it requires:
//!   - Xcode 26 + macOS 26 (VMK's platform floor)
//!   - `swift build` to compile the adapter
//!   - A Metal-capable GPU
//!
//! Run locally with:
//!   cargo test -p vrm-runner --test render_sequence_e2e_vmk -- --ignored
//!
//! This test makes vrm-metal-kit the first real renderer that drives the
//! render_sequence path end-to-end through the runner. Assertions cover:
//!   - the dispatch returns SequenceStatus::Ok (not Unimplemented, not Error)
//!   - real PNG frames land on disk (non-trivial size)
//!   - the runner re-hashes the adapter's placeholder BLAKE3 sentinel into
//!     a real, non-zero hash (Phase 5 Task 2 contract)

use camino::Utf8PathBuf;
use vrm_asset_generator::emit::emit_vrm;
use vrm_asset_generator::params::MToonParams;
use vrm_asset_generator::sidecar::build_default_test_plan;
use vrm_runner::execute::{execute_plan, ExecuteOptions, SequenceStatus};
use vrm_test_plan::{RenderSequenceBlock, SequenceFormat, SequenceRootTransformAnimation};

/// Locate or build the VMK adapter binary. The Swift adapter is not part
/// of the cargo workspace, so we drop down to a `swift build` invocation
/// when the binary isn't present.
fn vmk_bin() -> Utf8PathBuf {
    let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap().parent().unwrap();
    let vmk_dir = workspace_root.join("adapters/vrm-metal-kit");
    let bin = vmk_dir.join(".build/debug/vrm-metal-kit-adapter");

    if !bin.exists() {
        let status = std::process::Command::new("swift")
            .arg("build")
            .current_dir(vmk_dir.as_std_path())
            .status()
            .expect("swift build must be available (requires Xcode 26 + macOS 26)");
        assert!(status.success(), "swift build vrm-metal-kit-adapter failed");
    }
    bin
}

#[test]
#[ignore = "requires Xcode 26 + macOS 26 + swift build + Metal GPU"]
fn vmk_render_sequence_with_animate_root_transform_produces_frames() {
    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = asset_dir.clone();

    let id = "seq_e2e_vmk";
    let vrm_path = asset_dir.join(format!("{id}.vrm"));

    // Emit a real .vrm — VMK actually loads the glTF, unlike the mock
    // which only reads the .meta.json sidecar.
    let mtoon = MToonParams::defaults(id);
    emit_vrm(&mtoon, &vrm_path).expect("emit_vrm must succeed");

    // Build a default plan, then inject a 2-frame render_sequence block
    // with animate_root_transform so VMK exercises the per-frame
    // root-interpolation path.
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
        capture_positions: false,
    });
    // Validator rejects animation + render_sequence both set; the default
    // plan doesn't set animation but be explicit for clarity.
    plan.animation = None;

    let renderer_name = "vmk-seq";
    let opts = ExecuteOptions {
        adapter_bin: vmk_bin(),
        adapter_args: Vec::new(),
        asset_dir,
        output_dir,
        renderer_name: renderer_name.into(),
        emit_progress_ndjson: false,
        reference: None,
        reference_positions: None,
        vrma_path: None,
        apply_at_time: 0.0,
        reference_pose_json: None,
    };

    let result =
        execute_plan(&plan, &opts).expect("execute_plan against VMK in sequence mode must succeed");

    let seq = result
        .sequence
        .expect("sequence mode should populate result.sequence");

    assert_eq!(
        seq.status,
        SequenceStatus::Ok,
        "VMK should return Ok now that render_sequence is implemented; got {:?}, error_message={:?}, phase={:?}",
        seq.status, seq.error_message, seq.unimplemented_phase
    );

    let seq_result = seq
        .result
        .expect("RenderSequenceResult should be populated");
    assert_eq!(seq_result.frames.len(), 2, "frame_count was 2");

    // The zero-hash sentinel from the adapter, after the runner's re-hash.
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

        // The runner is supposed to re-hash; the result should not still
        // contain the adapter's zero-sentinel.
        assert!(
            frame.blake3.starts_with("blake3:"),
            "frame {i} blake3 missing prefix: {}",
            frame.blake3
        );
        assert_eq!(
            frame.blake3.len(),
            "blake3:".len() + 64,
            "frame {i} blake3 wrong length"
        );
        assert_ne!(
            frame.blake3, zero_sentinel,
            "frame {i} blake3 is still the adapter's sentinel — runner re-hash didn't run"
        );
    }
}
