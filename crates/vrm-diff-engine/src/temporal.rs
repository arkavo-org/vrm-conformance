//! Temporal diff for `render_sequence` outputs. Per-frame SSIM with
//! aggregation (mean / p95 / min), worst-frame tracking, and BLAKE3
//! identity short-circuit. See `rfcs/0004-render-sequence-op.md` and
//! `docs/methodology.md` ("Sequence captures") for the contract.

use crate::ssim::{ssim_pngs, SsimError};
use camino::Utf8Path;
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

/// Diff two sequences frame-by-frame. Each sequence is a list of PNG
/// paths in capture order (index 0 = first frame). When the two
/// sequences differ in length, only the common prefix is compared and
/// `frame_count_match` is set to false (a hard failure regardless of
/// SSIM, per docs/methodology.md).
///
/// The `threshold` is the per-test `temporal_ssim_threshold` (RFC-0004
/// default 0.90). Pass formula matches `docs/methodology.md`:
///   `passed = mean_ssim >= threshold AND
///            min_ssim >= threshold - 0.05 AND
///            frame_count_match`
///
/// Per-frame BLAKE3 hashing short-circuits SSIM computation when both
/// PNGs are byte-identical; those frames get `identity_match=true` and
/// `ssim=1.0` exactly.
pub fn temporal_diff(
    candidate_frames: &[&Utf8Path],
    reference_frames: &[&Utf8Path],
    threshold: f64,
) -> Result<TemporalDiffResult, TemporalDiffError> {
    let candidate_count = candidate_frames.len() as u32;
    let reference_count = reference_frames.len() as u32;
    let frame_count_match = candidate_count == reference_count;
    let compared = candidate_count.min(reference_count);

    let mut per_frame = Vec::with_capacity(compared as usize);
    for i in 0..compared as usize {
        let cand_hash = blake3_of_file(candidate_frames[i])?;
        let ref_hash = blake3_of_file(reference_frames[i])?;
        if cand_hash == ref_hash {
            per_frame.push(FrameDiff {
                index: i as u32,
                ssim: 1.0,
                identity_match: true,
            });
            continue;
        }
        let ssim = ssim_pngs(candidate_frames[i], reference_frames[i])?;
        per_frame.push(FrameDiff {
            index: i as u32,
            ssim,
            identity_match: false,
        });
    }

    let (mean_ssim, p95_ssim, min_ssim, worst_frame_index) = aggregate(&per_frame);

    let passed = frame_count_match && mean_ssim >= threshold && min_ssim >= threshold - 0.05;

    Ok(TemporalDiffResult {
        frame_count: candidate_count,
        frame_count_compared: compared,
        per_frame,
        mean_ssim,
        p95_ssim,
        min_ssim,
        worst_frame_index,
        frame_count_match,
        temporal_ssim_threshold: threshold,
        passed,
    })
}

/// Returns (mean, p95, min, worst_frame_index).
///
/// "p95 SSIM" is the 5th percentile (95% of frames are at least this
/// good). This convention matches site display: the headline number
/// shown to a reviewer is "worst-case quality across the run."
fn aggregate(frames: &[FrameDiff]) -> (f64, f64, f64, u32) {
    if frames.is_empty() {
        return (0.0, 0.0, 0.0, 0);
    }
    let sum: f64 = frames.iter().map(|f| f.ssim).sum();
    let mean = sum / frames.len() as f64;

    let mut sorted: Vec<f64> = frames.iter().map(|f| f.ssim).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p95_index = ((sorted.len() as f64) * 0.05).floor() as usize;
    let p95 = sorted[p95_index];
    let min = *sorted.first().unwrap();

    let worst = frames
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            a.ssim
                .partial_cmp(&b.ssim)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i as u32)
        .unwrap_or(0);

    (mean, p95, min, worst)
}

fn blake3_of_file(path: &Utf8Path) -> Result<[u8; 32], TemporalDiffError> {
    let bytes = std::fs::read(path.as_std_path()).map_err(|e| TemporalDiffError::Io {
        path: path.to_string(),
        source: e,
    })?;
    Ok(blake3::hash(&bytes).into())
}

#[derive(Debug, thiserror::Error)]
pub enum TemporalDiffError {
    #[error(transparent)]
    Ssim(#[from] SsimError),

    #[error("io error reading {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
}
