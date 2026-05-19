use std::path::PathBuf;
use vrm_diff_engine::temporal::{temporal_diff, FrameDiff, TemporalDiffResult};

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

fn make_solid_color(w: u32, h: u32, rgb: [u8; 3]) -> image::RgbImage {
    image::RgbImage::from_fn(w, h, |_, _| image::Rgb(rgb))
}

/// Write `n` solid-color PNGs to a temp dir, return their paths.
/// Each frame gets a slightly different color so SSIM <1 if compared
/// against a different sequence.
fn write_solid_sequence(dir: &std::path::Path, prefix: &str, n: usize, rgb: [u8; 3]) -> Vec<PathBuf> {
    (0..n)
        .map(|i| {
            let p = dir.join(format!("{prefix}_{i:04}.png"));
            make_solid_color(64, 64, rgb).save(&p).unwrap();
            p
        })
        .collect()
}

fn as_utf8(paths: &[PathBuf]) -> Vec<&camino::Utf8Path> {
    paths
        .iter()
        .map(|p| camino::Utf8Path::from_path(p).unwrap())
        .collect()
}

#[test]
fn temporal_diff_identical_sequences_pass() {
    let dir = tempfile::tempdir().unwrap();
    let cand = write_solid_sequence(dir.path(), "cand", 5, [128, 64, 200]);
    let refr = write_solid_sequence(dir.path(), "refr", 5, [128, 64, 200]);

    let result = temporal_diff(&as_utf8(&cand), &as_utf8(&refr), 0.90).unwrap();

    assert_eq!(result.frame_count, 5);
    assert_eq!(result.frame_count_compared, 5);
    assert!(result.frame_count_match);
    assert!(result.mean_ssim > 0.99, "mean = {}", result.mean_ssim);
    assert!(result.min_ssim > 0.99, "min = {}", result.min_ssim);
    assert!(result.passed);
    assert_eq!(result.per_frame.len(), 5);
    // identity_match is false because BLAKE3 short-circuit lands in Task 3
    assert!(result.per_frame.iter().all(|f| !f.identity_match));
}

#[test]
fn temporal_diff_single_bad_frame_min_relaxation() {
    // 5 identical frames + 1 black frame at index 3 against a white-reference frame.
    let dir = tempfile::tempdir().unwrap();
    let cand_paths: Vec<PathBuf> = (0..6)
        .map(|i| {
            let p = dir.path().join(format!("cand_{i:04}.png"));
            let rgb = if i == 3 { [0u8, 0, 0] } else { [128, 64, 200] };
            make_solid_color(64, 64, rgb).save(&p).unwrap();
            p
        })
        .collect();
    let refr_paths: Vec<PathBuf> = (0..6)
        .map(|i| {
            let p = dir.path().join(format!("refr_{i:04}.png"));
            let rgb = if i == 3 { [255u8, 255, 255] } else { [128, 64, 200] };
            make_solid_color(64, 64, rgb).save(&p).unwrap();
            p
        })
        .collect();

    let result = temporal_diff(&as_utf8(&cand_paths), &as_utf8(&refr_paths), 0.50).unwrap();

    assert_eq!(result.worst_frame_index, 3);
    assert!(result.frame_count_match);
    // Mean: 5 frames at ~1.0 + 1 frame near 0 = ~0.83. Above 0.50 threshold.
    assert!(result.mean_ssim > 0.50, "mean = {}", result.mean_ssim);
    // Min: bad frame ≈ 0. Threshold 0.50 ⇒ min must be ≥ 0.45 to pass.
    // 0 < 0.45 so min relaxation fails.
    assert!(result.min_ssim < 0.5, "min = {}", result.min_ssim);
    assert!(!result.passed, "single bad frame should fail min-relaxation");
}

#[test]
fn temporal_diff_gradual_drift() {
    // 6 frames where candidate vs reference diverge monotonically.
    // Use color distance to drive predictable SSIM degradation.
    let dir = tempfile::tempdir().unwrap();
    let cand: Vec<PathBuf> = (0..6)
        .map(|i| {
            let p = dir.path().join(format!("cand_{i:04}.png"));
            make_solid_color(64, 64, [128, 128, 128]).save(&p).unwrap();
            p
        })
        .collect();
    let refr: Vec<PathBuf> = (0..6)
        .map(|i| {
            let p = dir.path().join(format!("refr_{i:04}.png"));
            // Reference drifts away from candidate over frames
            let shift = (i as u8) * 20;
            make_solid_color(64, 64, [128 + shift, 128, 128]).save(&p).unwrap();
            p
        })
        .collect();

    let result = temporal_diff(&as_utf8(&cand), &as_utf8(&refr), 0.50).unwrap();

    assert!(result.frame_count_match);
    // Frame 0 should be identical (no drift); frame 5 should be most different
    assert_eq!(result.worst_frame_index, 5);
    // Per-frame SSIM should be monotonically (weakly) decreasing
    for win in result.per_frame.windows(2) {
        assert!(
            win[0].ssim >= win[1].ssim - 1e-6,
            "SSIM should be monotonically non-increasing across drift: {:?}",
            result.per_frame
        );
    }
}

#[test]
fn temporal_diff_length_mismatch_fails_regardless_of_ssim() {
    let dir = tempfile::tempdir().unwrap();
    let cand = write_solid_sequence(dir.path(), "cand", 5, [100, 100, 100]);
    let refr = write_solid_sequence(dir.path(), "refr", 8, [100, 100, 100]);

    let result = temporal_diff(&as_utf8(&cand), &as_utf8(&refr), 0.90).unwrap();

    assert!(!result.frame_count_match);
    assert_eq!(result.frame_count, 5);
    assert_eq!(result.frame_count_compared, 5); // min(5, 8)
    // All 5 compared frames are identical so mean/min SSIM ≈ 1.0
    assert!(result.mean_ssim > 0.99);
    assert!(result.min_ssim > 0.99);
    // But length mismatch trumps SSIM
    assert!(!result.passed, "length mismatch must fail regardless of SSIM");
}
