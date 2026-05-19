//! Temporal diff for `render_sequence` outputs. Per-frame SSIM with
//! aggregation (mean / p95 / min), worst-frame tracking, and BLAKE3
//! identity short-circuit. See `rfcs/0004-render-sequence-op.md` and
//! `docs/methodology.md` ("Sequence captures") for the contract.

use serde::{Deserialize, Serialize};

/// Per-frame diff record. `identity_match` is true when both renders
/// produced byte-identical PNGs (BLAKE3 short-circuit hit); SSIM is set
/// to 1.0 in that case without computing SSIM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrameDiff {
    pub index: u32,
    pub ssim: f64,
    pub identity_match: bool,
}

/// Aggregated diff result across a sequence pair. Pass criteria from
/// `docs/methodology.md`:
///   `passed = mean_ssim >= threshold AND
///            min_ssim >= threshold - 0.05 AND
///            frame_count_match`
///
/// `frame_count_compared` is the number of frames actually diffed —
/// when the two sequences differ in length, this is `min(candidate, ref)`.
/// `worst_frame_index` indexes into `per_frame` (which is in capture
/// order, sorted by `index`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalDiffResult {
    pub frame_count: u32,
    pub frame_count_compared: u32,
    pub per_frame: Vec<FrameDiff>,
    pub mean_ssim: f64,
    pub p95_ssim: f64,
    pub min_ssim: f64,
    pub worst_frame_index: u32,
    pub frame_count_match: bool,
    pub temporal_ssim_threshold: f64,
    pub passed: bool,
}
