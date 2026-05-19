use vrm_diff_engine::temporal::{FrameDiff, TemporalDiffResult};

#[test]
fn temporal_diff_result_roundtrip() {
    let r = TemporalDiffResult {
        frame_count: 3,
        frame_count_compared: 3,
        per_frame: vec![
            FrameDiff { index: 0, ssim: 1.0, identity_match: true },
            FrameDiff { index: 1, ssim: 0.97, identity_match: false },
            FrameDiff { index: 2, ssim: 0.95, identity_match: false },
        ],
        mean_ssim: 0.973,
        p95_ssim: 0.95,
        min_ssim: 0.95,
        worst_frame_index: 2,
        frame_count_match: true,
        temporal_ssim_threshold: 0.90,
        passed: true,
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: TemporalDiffResult = serde_json::from_str(&s).unwrap();
    assert_eq!(back.per_frame, r.per_frame);
    assert_eq!(back.frame_count, 3);
    assert_eq!(back.worst_frame_index, 2);
    assert!(back.passed);
    assert!(back.frame_count_match);
    // Verify identity_match survives serialization
    assert!(back.per_frame[0].identity_match);
    assert!(!back.per_frame[1].identity_match);
}

#[test]
fn frame_diff_roundtrip() {
    let f = FrameDiff { index: 42, ssim: 0.876, identity_match: false };
    let s = serde_json::to_string(&f).unwrap();
    let back: FrameDiff = serde_json::from_str(&s).unwrap();
    assert_eq!(back, f);
}
