//! Tests for `render_sequence` per-frame spring-position capture (CCD Phase 1).
//!
//! When `capture_positions = true` every `SequenceFrame.spring_positions` must
//! be `Some(non-empty)` and must match the shape returned by `dump_bone_positions`.
//! When `capture_positions = false` every frame's `spring_positions` must be `None`.
//!
//! The mock has no real physics engine; positions are a static deterministic
//! synthetic chain identical across all frames.  Two identical runs must produce
//! identical positions (determinism contract).

use camino::Utf8PathBuf;
use vrm_asset_generator::params::MToonParams;
use vrm_mock_renderer::handlers;
use vrm_mock_renderer::session::{Session, SessionRegistry};
use vrm_ops::tools as ops;

/// Spin up an isolated mock session backed by a synthetic stub asset.
/// Duplicated from the other render_sequence test files; integration tests
/// cannot share helpers across files without a common module.
fn fresh_session() -> (SessionRegistry, String) {
    let dir = tempfile::tempdir().unwrap();
    let id = "anchor";
    let vrm_path = dir.path().join(format!("{id}.vrm"));
    let meta_path = dir.path().join(format!("{id}.meta.json"));

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
    drop(dir);
    (reg, session_id)
}

fn base_params(session_id: &str, output_dir: &str, capture: bool) -> ops::RenderSequenceParams {
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
        animate_root_transform: Some(ops::RootTransformAnimation {
            translation_start: [0.0, 0.0, 0.0],
            translation_end: [0.25, 0.0, 0.0],
        }),
        apply_vrma: None,
        capture_positions: capture,
    }
}

#[test]
fn render_sequence_includes_positions_when_capture_requested() {
    let (mut reg, session_id) = fresh_session();
    let out = tempfile::tempdir().unwrap();

    let params = base_params(&session_id, out.path().to_str().unwrap(), true);
    let result = handlers::render_sequence(&mut reg, params).unwrap();

    assert_eq!(result.frames.len(), 3, "expected 3 frames");

    for frame in &result.frames {
        let sp = frame
            .spring_positions
            .as_ref()
            .unwrap_or_else(|| panic!("frame {} has no spring_positions", frame.index));
        assert!(
            !sp.is_empty(),
            "frame {} spring_positions must be non-empty",
            frame.index
        );
        // Mock's synthetic chain: 1 chain named "mock_hair" with 4 joints.
        assert_eq!(sp.len(), 1, "mock returns exactly 1 synthetic spring chain");
        assert_eq!(
            sp[0].name, "mock_hair",
            "unexpected synthetic chain name in frame {}",
            frame.index
        );
        assert_eq!(
            sp[0].joint_positions.len(),
            4,
            "mock chain has 4 joints (frame {})",
            frame.index
        );
    }
}

#[test]
fn render_sequence_omits_positions_when_not_requested() {
    let (mut reg, session_id) = fresh_session();
    let out = tempfile::tempdir().unwrap();

    let params = base_params(&session_id, out.path().to_str().unwrap(), false);
    let result = handlers::render_sequence(&mut reg, params).unwrap();

    assert_eq!(result.frames.len(), 3);
    for frame in &result.frames {
        assert!(
            frame.spring_positions.is_none(),
            "frame {} should have no spring_positions when capture_positions=false",
            frame.index
        );
    }
}

#[test]
fn render_sequence_positions_are_deterministic() {
    // Two identical runs (same params, different output dirs) must produce
    // byte-identical spring_positions on every frame.
    let (mut reg, session_id) = fresh_session();
    let out_a = tempfile::tempdir().unwrap();
    let out_b = tempfile::tempdir().unwrap();

    let result_a = handlers::render_sequence(
        &mut reg,
        base_params(&session_id, out_a.path().to_str().unwrap(), true),
    )
    .unwrap();
    let result_b = handlers::render_sequence(
        &mut reg,
        base_params(&session_id, out_b.path().to_str().unwrap(), true),
    )
    .unwrap();

    assert_eq!(result_a.frames.len(), result_b.frames.len());
    for (fa, fb) in result_a.frames.iter().zip(result_b.frames.iter()) {
        assert_eq!(
            fa.spring_positions, fb.spring_positions,
            "spring_positions differ between runs at frame {}",
            fa.index
        );
    }
}

#[test]
fn render_sequence_positions_match_dump_bone_positions_shape() {
    // The per-frame positions must have the same chain count and joint count
    // as what dump_bone_positions reports for the same session.
    let (mut reg, session_id) = fresh_session();
    let out = tempfile::tempdir().unwrap();

    let dump_result = handlers::dump_bone_positions(
        &mut reg,
        ops::DumpBonePositionsParams {
            session_id: session_id.clone(),
            spring_index: None,
        },
    )
    .unwrap();

    let seq_result = handlers::render_sequence(
        &mut reg,
        base_params(&session_id, out.path().to_str().unwrap(), true),
    )
    .unwrap();

    for frame in &seq_result.frames {
        let sp = frame.spring_positions.as_ref().expect("positions present");
        assert_eq!(
            sp.len(),
            dump_result.springs.len(),
            "frame {} chain count must match dump_bone_positions",
            frame.index
        );
        for (chain_idx, (seq_chain, dump_chain)) in
            sp.iter().zip(dump_result.springs.iter()).enumerate()
        {
            assert_eq!(
                seq_chain.joint_positions.len(),
                dump_chain.joint_positions.len(),
                "frame {} chain {} joint count mismatch",
                frame.index,
                chain_idx
            );
            assert_eq!(
                seq_chain.name, dump_chain.name,
                "frame {} chain {} name mismatch",
                frame.index, chain_idx
            );
        }
    }
}
