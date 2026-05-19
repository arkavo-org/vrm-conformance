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
    // identity_match may be true (BLAKE3 short-circuit) for byte-identical PNGs —
    // either path (ssim≈1.0 or identity_match=true) satisfies "identical inputs pass".
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
fn temporal_diff_identical_files_short_circuit_to_identity_match() {
    // Point both candidate and reference at the SAME file paths.
    // BLAKE3 of each pair must match → identity_match=true, ssim=1.0 exactly.
    let dir = tempfile::tempdir().unwrap();
    let frames = write_solid_sequence(dir.path(), "shared", 3, [50, 150, 100]);

    // Use the same paths for both sides. The function shouldn't even
    // call ssim_pngs for these — identity_match should short-circuit.
    let cand = as_utf8(&frames);
    let refr = as_utf8(&frames);
    let result = temporal_diff(&cand, &refr, 0.90).unwrap();

    assert_eq!(result.frame_count, 3);
    assert!(result.frame_count_match);
    assert!(result.passed);

    // Every frame must be flagged as identity_match
    for (i, fd) in result.per_frame.iter().enumerate() {
        assert!(fd.identity_match, "frame {i} should be identity_match");
        // SSIM should be exactly 1.0 (not 0.9999...) since we skipped SSIM
        assert_eq!(fd.ssim, 1.0, "frame {i} ssim should be exactly 1.0");
    }
}

#[test]
fn temporal_diff_identical_content_different_files_short_circuit() {
    // Two distinct files with byte-identical content (same RGB).
    // BLAKE3 should still match because content is identical.
    let dir = tempfile::tempdir().unwrap();
    let cand = write_solid_sequence(dir.path(), "cand", 2, [200, 100, 50]);
    let refr = write_solid_sequence(dir.path(), "refr", 2, [200, 100, 50]);

    // Sanity check: bytes match
    for (a, b) in cand.iter().zip(refr.iter()) {
        assert_eq!(std::fs::read(a).unwrap(), std::fs::read(b).unwrap(),
                   "test setup: solid-color PNGs of same color should be byte-identical");
    }

    let result = temporal_diff(&as_utf8(&cand), &as_utf8(&refr), 0.90).unwrap();
    for fd in &result.per_frame {
        assert!(fd.identity_match, "byte-identical PNGs should short-circuit");
        assert_eq!(fd.ssim, 1.0);
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

#[test]
fn temporal_diff_empty_sequences() {
    // Both sequences empty. frame_count_match=true (both 0), but no
    // signal to assert pass — design choice: passed=false (no frames
    // compared means we can't assert conformance).
    let result = temporal_diff(&[], &[], 0.90).unwrap();

    assert_eq!(result.frame_count, 0);
    assert_eq!(result.frame_count_compared, 0);
    assert!(result.frame_count_match, "0 == 0");
    assert!(result.per_frame.is_empty());
    assert_eq!(result.mean_ssim, 0.0);
    assert_eq!(result.min_ssim, 0.0);
    assert_eq!(result.worst_frame_index, 0);
    // mean_ssim 0.0 < threshold 0.90 ⇒ passed=false.
    // This is the right default: an empty sequence cannot conform.
    assert!(!result.passed);
}

#[test]
fn temporal_diff_single_frame_sequence() {
    // Single-frame each, identical. mean=p95=min=ssim.
    let dir = tempfile::tempdir().unwrap();
    let cand = write_solid_sequence(dir.path(), "cand", 1, [80, 80, 80]);
    let refr = write_solid_sequence(dir.path(), "refr", 1, [80, 80, 80]);

    let result = temporal_diff(&as_utf8(&cand), &as_utf8(&refr), 0.90).unwrap();

    assert_eq!(result.frame_count, 1);
    assert_eq!(result.frame_count_compared, 1);
    assert_eq!(result.per_frame.len(), 1);
    // mean == p95 == min for a 1-element distribution
    assert_eq!(result.mean_ssim, result.p95_ssim);
    assert_eq!(result.mean_ssim, result.min_ssim);
    assert!(result.passed);
}

#[test]
fn temporal_diff_one_empty_one_nonempty_fails_length_mismatch() {
    // Asymmetric empty: candidate has 0, reference has 3.
    // frame_count_match=false → passed=false regardless.
    let dir = tempfile::tempdir().unwrap();
    let refr = write_solid_sequence(dir.path(), "refr", 3, [80, 80, 80]);

    let result = temporal_diff(&[], &as_utf8(&refr), 0.90).unwrap();

    assert_eq!(result.frame_count, 0);
    assert_eq!(result.frame_count_compared, 0);
    assert!(!result.frame_count_match);
    assert!(!result.passed);
}
