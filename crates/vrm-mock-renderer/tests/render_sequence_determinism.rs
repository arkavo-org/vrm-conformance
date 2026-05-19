//! Cross-run determinism: render the same sequence twice into two
//! distinct temp dirs, assert byte-identical PNGs file-by-file and
//! identical BLAKE3 hashes in the result. Then verify temporal_diff
//! identifies every frame as identity_match with SSIM 1.0.
//!
//! This is the Phase 3 done-when condition for render_sequence in the
//! mock renderer.

use camino::Utf8PathBuf;
use vrm_asset_generator::params::MToonParams;
use vrm_mock_renderer::handlers;
use vrm_mock_renderer::session::{Session, SessionRegistry};
use vrm_ops::tools as ops;

/// Spin up an isolated mock session backed by a synthetic stub asset
/// (VRM body stub + meta.json sidecar — the mock only reads the sidecar).
/// Duplicated verbatim from render_sequence_animation.rs; integration
/// tests cannot share helpers across files without a common module.
fn fresh_session() -> (SessionRegistry, String) {
    let dir = tempfile::tempdir().unwrap();
    let id = "anchor";
    let vrm_path = dir.path().join(format!("{id}.vrm"));
    let meta_path = dir.path().join(format!("{id}.meta.json"));

    // The mock reads the sidecar for MToonParams; the .vrm body is unused.
    std::fs::write(&vrm_path, b"glTF\x02\x00\x00\x00\x0c\x00\x00\x00").unwrap();
    let meta = serde_json::json!({
        "id": id,
        "license": "CC0-1.0",
        "params": MToonParams::defaults(id),
    });
    std::fs::write(&meta_path, serde_json::to_vec(&meta).unwrap()).unwrap();

    let asset_path = Utf8PathBuf::try_from(vrm_path).expect("path is valid UTF-8");
    let mut reg = SessionRegistry::new();
    let session = Session::load(asset_path.as_path()).unwrap();
    let session_id = reg.insert(session);

    // Keep `dir` alive until the session is inserted; the registry owns nothing
    // from the tempdir, so we can let dir drop here.
    drop(dir);

    (reg, session_id)
}

fn params(session_id: &str, output_dir: &str) -> ops::RenderSequenceParams {
    ops::RenderSequenceParams {
        session_id: session_id.into(),
        width: 64,
        height: 64,
        output_dir: output_dir.into(),
        frame_count: 5,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        color_space: ops::ColorSpace::Linear,
        msaa: 1,
        output_type: ops::OutputType::Color,
        output_format: ops::SequenceFormat::PngSequence,
        animate_root_transform: Some(ops::RootTransformAnimation {
            translation_start: [0.0, 0.0, 0.0],
            translation_end: [0.25, 0.0, 0.0],
        }),
        apply_vrma: None,
    }
}

#[test]
fn two_runs_produce_byte_identical_pngs() {
    let (mut reg, session_id) = fresh_session();

    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();

    let result_a = handlers::render_sequence(
        &mut reg,
        params(&session_id, dir_a.path().to_str().unwrap()),
    )
    .unwrap();
    let result_b = handlers::render_sequence(
        &mut reg,
        params(&session_id, dir_b.path().to_str().unwrap()),
    )
    .unwrap();

    assert_eq!(result_a.frames.len(), result_b.frames.len());

    for (fa, fb) in result_a.frames.iter().zip(result_b.frames.iter()) {
        assert_eq!(fa.blake3, fb.blake3, "frame {} blake3 differs", fa.index);
        let bytes_a = std::fs::read(&fa.path).unwrap();
        let bytes_b = std::fs::read(&fb.path).unwrap();
        assert_eq!(bytes_a, bytes_b, "frame {} png bytes differ", fa.index);
    }
}

#[test]
fn self_diff_via_temporal_diff_returns_ssim_1_for_all_frames() {
    use vrm_diff_engine::temporal::temporal_diff;

    let (mut reg, session_id) = fresh_session();

    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();

    let result_a = handlers::render_sequence(
        &mut reg,
        params(&session_id, dir_a.path().to_str().unwrap()),
    )
    .unwrap();
    let result_b = handlers::render_sequence(
        &mut reg,
        params(&session_id, dir_b.path().to_str().unwrap()),
    )
    .unwrap();

    let paths_a: Vec<camino::Utf8PathBuf> = result_a
        .frames
        .iter()
        .map(|f| camino::Utf8PathBuf::from(&f.path))
        .collect();
    let paths_b: Vec<camino::Utf8PathBuf> = result_b
        .frames
        .iter()
        .map(|f| camino::Utf8PathBuf::from(&f.path))
        .collect();

    let refs_a: Vec<&camino::Utf8Path> = paths_a.iter().map(|p| p.as_path()).collect();
    let refs_b: Vec<&camino::Utf8Path> = paths_b.iter().map(|p| p.as_path()).collect();

    let diff = temporal_diff(&refs_a, &refs_b, 0.95).unwrap();

    assert!(diff.frame_count_match);
    assert_eq!(
        diff.mean_ssim, 1.0,
        "self-diff mean SSIM must be exactly 1.0"
    );
    assert_eq!(diff.min_ssim, 1.0, "self-diff min SSIM must be exactly 1.0");
    assert!(
        diff.per_frame.iter().all(|f| f.identity_match),
        "all frames should be identity_match (BLAKE3 short-circuit): {:?}",
        diff.per_frame
    );
    assert!(diff.passed);
}
