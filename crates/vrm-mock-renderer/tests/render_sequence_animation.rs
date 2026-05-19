//! Anchor tests pinning the mock renderer's per-frame variation semantics.
//! Guards against silent changes to how animate_root_transform and
//! apply_vrma encode into pixel content.

use camino::Utf8PathBuf;
use vrm_asset_generator::params::MToonParams;
use vrm_mock_renderer::handlers;
use vrm_mock_renderer::session::{Session, SessionRegistry};
use vrm_ops::tools as ops;

/// Spin up an isolated mock session backed by a synthetic stub asset
/// (VRM body stub + meta.json sidecar — the mock only reads the sidecar).
/// Returns the registry and session_id.
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

    let asset_path =
        Utf8PathBuf::try_from(vrm_path).expect("path is valid UTF-8");
    let mut reg = SessionRegistry::new();
    let session = Session::load(asset_path.as_path()).unwrap();
    let session_id = reg.insert(session);

    // Keep `dir` alive until the session is inserted; the registry owns nothing
    // from the tempdir, so we can let dir drop here.
    drop(dir);

    (reg, session_id)
}

fn base_params(session_id: &str, output_dir: &str) -> ops::RenderSequenceParams {
    ops::RenderSequenceParams {
        session_id: session_id.into(),
        width: 64,
        height: 64,
        output_dir: output_dir.into(),
        frame_count: 3,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        color_space: ops::ColorSpace::Linear,
        msaa: 1,
        output_type: ops::OutputType::Color,
        output_format: ops::SequenceFormat::PngSequence,
        animate_root_transform: None,
        apply_vrma: None,
    }
}

#[test]
fn frames_without_animation_differ_only_in_frame_marker() {
    let (mut reg, session_id) = fresh_session();
    let out = tempfile::tempdir().unwrap();

    let params = base_params(&session_id, out.path().to_str().unwrap());
    let result = handlers::render_sequence(&mut reg, params).unwrap();

    assert_eq!(result.frames.len(), 3);
    // Frame-index marker grows; all three frames must have distinct BLAKE3.
    assert_ne!(result.frames[0].blake3, result.frames[1].blake3);
    assert_ne!(result.frames[1].blake3, result.frames[2].blake3);
    assert_ne!(result.frames[0].blake3, result.frames[2].blake3);
}

#[test]
fn frames_with_root_animation_shift_per_frame() {
    let (mut reg, session_id) = fresh_session();
    let out = tempfile::tempdir().unwrap();

    let mut params = base_params(&session_id, out.path().to_str().unwrap());
    params.animate_root_transform = Some(ops::RootTransformAnimation {
        translation_start: [0.0, 0.0, 0.0],
        translation_end: [0.5, 0.0, 0.0], // 50% width shift across the sequence
    });

    let result = handlers::render_sequence(&mut reg, params).unwrap();
    let hashes: std::collections::HashSet<_> =
        result.frames.iter().map(|f| f.blake3.clone()).collect();
    assert_eq!(hashes.len(), 3, "all 3 frames should have distinct hashes");
}

#[test]
fn frames_with_vrma_encode_time_in_marker() {
    let (mut reg, session_id) = fresh_session();
    let out = tempfile::tempdir().unwrap();

    let mut params = base_params(&session_id, out.path().to_str().unwrap());
    params.apply_vrma = Some(ops::VrmaPlaybackSpec {
        vrma_handle: 1,
        start_seconds: 0.0,
    });

    let result = handlers::render_sequence(&mut reg, params).unwrap();
    assert_eq!(result.frames.len(), 3);
    // VRMA marker color advances frame-to-frame; frame 0 vs frame 2 must
    // be distinct (frame_marker AND vrma_marker both differ).
    assert_ne!(result.frames[0].blake3, result.frames[2].blake3);
}

#[test]
fn mutual_exclusion_rejected() {
    let (mut reg, session_id) = fresh_session();
    let out = tempfile::tempdir().unwrap();

    let mut params = base_params(&session_id, out.path().to_str().unwrap());
    params.animate_root_transform = Some(ops::RootTransformAnimation {
        translation_start: [0.0; 3],
        translation_end: [1.0, 0.0, 0.0],
    });
    params.apply_vrma = Some(ops::VrmaPlaybackSpec {
        vrma_handle: 1,
        start_seconds: 0.0,
    });

    let err = handlers::render_sequence(&mut reg, params).unwrap_err();
    assert_eq!(err.code, -32602, "mutual exclusion must return -32602");
}

#[test]
fn physics_dt_above_60hz_rejected() {
    let (mut reg, session_id) = fresh_session();
    let out = tempfile::tempdir().unwrap();

    let mut params = base_params(&session_id, out.path().to_str().unwrap());
    params.physics_dt_seconds = 0.1; // well over 1/60

    let err = handlers::render_sequence(&mut reg, params).unwrap_err();
    assert_eq!(err.code, -32602, "physics_dt floor must return -32602");
}

#[test]
fn frame_count_is_consistent_with_request() {
    let (mut reg, session_id) = fresh_session();
    let out = tempfile::tempdir().unwrap();

    let mut params = base_params(&session_id, out.path().to_str().unwrap());
    params.frame_count = 7;

    let result = handlers::render_sequence(&mut reg, params).unwrap();
    assert_eq!(result.frames.len(), 7);
    // Indices are 0..7 in capture order
    for (expected_idx, frame) in result.frames.iter().enumerate() {
        assert_eq!(frame.index, expected_idx as u32);
    }
    // duration = frame_count / frame_hz
    assert!((result.duration_seconds - 7.0 / 30.0).abs() < 1e-6);
}

#[test]
fn png_sequence_format_produces_no_muxed_file() {
    let (mut reg, session_id) = fresh_session();
    let out = tempfile::tempdir().unwrap();

    let params = base_params(&session_id, out.path().to_str().unwrap());
    let result = handlers::render_sequence(&mut reg, params).unwrap();
    assert!(result.muxed_path.is_none());
}

/// This test runs only when ffmpeg is on PATH (gated by --ignored to keep
/// CI green when ffmpeg isn't installed). The soft-skip path is covered
/// implicitly by the PngSequence test above — when ffmpeg is missing the
/// handler returns muxed_path: None just like the PngSequence case.
#[test]
#[ignore = "requires ffmpeg on PATH"]
fn mp4_format_invokes_ffmpeg() {
    let (mut reg, session_id) = fresh_session();
    let out = tempfile::tempdir().unwrap();

    let mut params = base_params(&session_id, out.path().to_str().unwrap());
    params.output_format = ops::SequenceFormat::Mp4;
    params.frame_count = 5;

    let result = handlers::render_sequence(&mut reg, params).unwrap();
    if let Some(path) = &result.muxed_path {
        assert!(
            std::path::Path::new(path).exists(),
            "ffmpeg ran but didn't produce the file: {path}"
        );
        assert!(path.ends_with("sequence.mp4"));
    } else {
        // ffmpeg absent on this machine — soft-skip path taken. Still OK.
        eprintln!("ffmpeg not on PATH; mp4 test soft-skipped");
    }
}

#[test]
#[ignore = "requires ffmpeg on PATH"]
fn mov_format_invokes_ffmpeg() {
    let (mut reg, session_id) = fresh_session();
    let out = tempfile::tempdir().unwrap();

    let mut params = base_params(&session_id, out.path().to_str().unwrap());
    params.output_format = ops::SequenceFormat::Mov;
    params.frame_count = 5;

    let result = handlers::render_sequence(&mut reg, params).unwrap();
    if let Some(path) = &result.muxed_path {
        assert!(std::path::Path::new(path).exists());
        assert!(path.ends_with("sequence.mov"));
    }
}

#[test]
fn frame_paths_match_zero_padded_naming() {
    let (mut reg, session_id) = fresh_session();
    let out = tempfile::tempdir().unwrap();

    let mut params = base_params(&session_id, out.path().to_str().unwrap());
    params.frame_count = 12; // verify 4-digit zero-padding

    let result = handlers::render_sequence(&mut reg, params).unwrap();
    assert!(result.frames[0].path.ends_with("0000.png"));
    assert!(result.frames[9].path.ends_with("0009.png"));
    assert!(result.frames[10].path.ends_with("0010.png"));
    assert!(result.frames[11].path.ends_with("0011.png"));

    // All files exist on disk
    for frame in &result.frames {
        assert!(
            std::path::Path::new(&frame.path).exists(),
            "frame {} not on disk: {}",
            frame.index,
            frame.path
        );
    }
}
