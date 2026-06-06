//! Fast test: the runner writes <id>_<renderer>_colliders.json when a sequence
//! result carries per-frame synthetic colliders. Uses a stub result, no adapter.

use vrm_ops::tools::{ColorSpace, RenderSequenceResult, SequenceCollider, SequenceFrame};
use vrm_runner::execute::{persist_colliders_json_for_test, FrameCollidersEntry};

#[test]
fn writes_colliders_json_when_frames_carry_synthetic_colliders() {
    let dir = tempfile::tempdir().unwrap();
    let out = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let result = RenderSequenceResult {
        frames: vec![SequenceFrame {
            index: 0,
            timestamp_seconds: 0.0,
            path: "0.png".into(),
            blake3: "blake3:0".into(),
            spring_positions: None,
            synthetic_colliders: Some(vec![SequenceCollider::Sphere {
                center: [0.0, 1.0, 0.0],
                radius: 0.05,
            }]),
        }],
        duration_seconds: 0.0,
        actual_color_space: ColorSpace::Linear,
        frame_hz_achieved: 30.0,
        muxed_path: None,
    };
    persist_colliders_json_for_test(&result, &out, "synthcoll", "vmk").unwrap();
    let path = out.join("synthcoll_vmk_colliders.json");
    assert!(path.exists());
    let entries: Vec<FrameCollidersEntry> =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].frame_index, 0);
    assert_eq!(entries[0].colliders.len(), 1);
}
