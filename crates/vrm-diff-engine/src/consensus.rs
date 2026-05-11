//! N-way cross-renderer consensus over rendered PNGs.
//!
//! Pairwise SSIM (`ssim_pngs`) answers "does renderer X match reference Y?"
//! With a single reference renderer per test plan, that means the test is
//! anchored to whichever renderer was declared canonical — fine when one
//! renderer is the spec ground truth (e.g., glTF-Render-Fidelity's reference
//! pipeline) but limiting when multiple renderers each have legitimate
//! claims to correctness.
//!
//! Consensus diff lifts that anchor. Given N rendered PNGs from N different
//! renderers of the same test plan, we compute the full N×N pairwise SSIM
//! matrix, then for each renderer count how many *other* renderers it
//! agrees with within the threshold. A renderer that agrees with every
//! peer passes; a renderer that disagrees with one or more peers is an
//! outlier. The test plan passes overall iff there are no outliers.
//!
//! ## Generalization
//!
//! - **N = 2**: agreement_count == 1 iff the pair matches. Passes iff
//!   both match each other — semantically equivalent to pairwise SSIM.
//! - **N = 3**: each renderer must match both peers. Outliers are flagged
//!   individually so cross-renderer regressions surface against a real
//!   majority instead of a single reference.
//! - **N ≥ 4**: same pattern. The full matrix is preserved in the result
//!   so downstream tools (the comparison site, a dashboard, an
//!   investigator) can inspect specific pair scores.

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use crate::ssim::{ssim_pngs, SsimError};

/// One renderer's contribution to a consensus diff: the path to its
/// rendered PNG plus the name it's known by in the manifest. Names matter
/// because the result reports outliers by name.
#[derive(Debug, Clone)]
pub struct RendererRender<'a> {
    pub name: String,
    pub png_path: &'a Utf8Path,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResult {
    pub test_id: String,
    pub threshold: f32,
    /// Renderer names in the order they appear in `ssim_matrix`.
    pub renderers: Vec<String>,
    /// Symmetric N×N pairwise SSIM. Diagonal is 1.0. `ssim_matrix[i][j]`
    /// is SSIM between `renderers[i]` and `renderers[j]`.
    pub ssim_matrix: Vec<Vec<f32>>,
    /// `agreement_count[i]` = number of *other* renderers (j != i) for
    /// which `ssim_matrix[i][j] >= threshold`. Max possible = N - 1.
    pub agreement_count: Vec<u32>,
    /// Renderer names whose `agreement_count` is below `n - 1`. Empty
    /// iff every renderer agrees with every peer.
    pub outliers: Vec<String>,
    /// True iff `outliers.is_empty()`. Convenience for CI gates.
    pub consensus_passed: bool,
}

impl ConsensusResult {
    pub fn overall_passed(&self) -> bool {
        self.consensus_passed
    }
}

/// Compute the N-way consensus diff. Loads each PNG, computes pairwise
/// SSIM for every unordered pair (the diagonal is 1.0; the matrix is
/// symmetric), then identifies outliers per the `threshold`.
///
/// Errors if fewer than 2 renderers are passed (consensus is meaningless
/// at N < 2) or if any pair fails to load / has mismatched dimensions.
pub fn consensus_diff(
    test_id: &str,
    renderers: &[RendererRender<'_>],
    threshold: f32,
) -> Result<ConsensusResult, SsimError> {
    if renderers.len() < 2 {
        return Err(SsimError::Compute(format!(
            "consensus needs at least 2 renderers, got {}",
            renderers.len()
        )));
    }

    let n = renderers.len();
    let mut matrix: Vec<Vec<f32>> = vec![vec![1.0_f32; n]; n];

    for i in 0..n {
        for j in (i + 1)..n {
            let s = ssim_pngs(renderers[i].png_path, renderers[j].png_path)? as f32;
            matrix[i][j] = s;
            matrix[j][i] = s;
        }
    }

    let agreement_count: Vec<u32> = (0..n)
        .map(|i| {
            (0..n)
                .filter(|&j| j != i && matrix[i][j] >= threshold)
                .count() as u32
        })
        .collect();

    let max_agreement = (n - 1) as u32;
    let outliers: Vec<String> = renderers
        .iter()
        .zip(agreement_count.iter())
        .filter_map(|(r, &count)| {
            if count < max_agreement {
                Some(r.name.clone())
            } else {
                None
            }
        })
        .collect();

    Ok(ConsensusResult {
        test_id: test_id.into(),
        threshold,
        renderers: renderers.iter().map(|r| r.name.clone()).collect(),
        ssim_matrix: matrix,
        agreement_count,
        outliers: outliers.clone(),
        consensus_passed: outliers.is_empty(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use image::{Rgb, RgbImage};

    fn write_solid_png(path: &Utf8PathBuf, w: u32, h: u32, rgb: [u8; 3]) {
        let img = RgbImage::from_fn(w, h, |_, _| Rgb(rgb));
        img.save(path.as_std_path()).expect("write png");
    }

    fn make_path(dir: &std::path::Path, name: &str) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(dir.join(name)).unwrap()
    }

    #[test]
    fn three_identical_images_all_pass() {
        let dir = tempfile::tempdir().unwrap();
        let p_a = make_path(dir.path(), "a.png");
        let p_b = make_path(dir.path(), "b.png");
        let p_c = make_path(dir.path(), "c.png");
        for p in [&p_a, &p_b, &p_c] {
            write_solid_png(p, 32, 32, [120, 120, 120]);
        }

        let result = consensus_diff(
            "test",
            &[
                RendererRender { name: "a".into(), png_path: &p_a },
                RendererRender { name: "b".into(), png_path: &p_b },
                RendererRender { name: "c".into(), png_path: &p_c },
            ],
            0.99,
        )
        .unwrap();

        assert!(result.consensus_passed);
        assert_eq!(result.outliers, Vec::<String>::new());
        assert_eq!(result.agreement_count, vec![2, 2, 2]);
        // Diagonal should be 1.0; off-diagonal should be ~1.0 since identical.
        for i in 0..3 {
            assert!((result.ssim_matrix[i][i] - 1.0).abs() < 1e-6);
            for j in 0..3 {
                if i != j {
                    assert!(
                        result.ssim_matrix[i][j] > 0.99,
                        "expected near-1 SSIM between identical images, got {} at [{i}][{j}]",
                        result.ssim_matrix[i][j]
                    );
                }
            }
        }
    }

    #[test]
    fn two_matching_plus_one_outlier_flags_the_outlier() {
        let dir = tempfile::tempdir().unwrap();
        let p_a = make_path(dir.path(), "a.png");
        let p_b = make_path(dir.path(), "b.png");
        let p_c = make_path(dir.path(), "c.png");
        write_solid_png(&p_a, 64, 64, [40, 200, 40]);
        write_solid_png(&p_b, 64, 64, [40, 200, 40]); // matches a
        write_solid_png(&p_c, 64, 64, [255, 0, 0]); // diverges

        let result = consensus_diff(
            "test",
            &[
                RendererRender { name: "a".into(), png_path: &p_a },
                RendererRender { name: "b".into(), png_path: &p_b },
                RendererRender { name: "outlier".into(), png_path: &p_c },
            ],
            0.99,
        )
        .unwrap();

        assert!(!result.consensus_passed);
        // All three are outliers in the unanimity-required model: a and b
        // each agree with one peer only (the matching one), not with the
        // diverging c. With N=3 and threshold strict, partial agreement
        // doesn't clear the bar.
        assert!(result.outliers.contains(&"outlier".to_string()));
        // The outlier renderer agrees with 0 peers; a and b agree with 1.
        let outlier_idx = result
            .renderers
            .iter()
            .position(|n| n == "outlier")
            .unwrap();
        assert_eq!(result.agreement_count[outlier_idx], 0);
    }

    #[test]
    fn n_equals_2_reduces_to_pairwise_ssim() {
        let dir = tempfile::tempdir().unwrap();
        let p_a = make_path(dir.path(), "a.png");
        let p_b = make_path(dir.path(), "b.png");
        write_solid_png(&p_a, 32, 32, [100, 100, 100]);
        write_solid_png(&p_b, 32, 32, [100, 100, 100]);

        let result = consensus_diff(
            "pair",
            &[
                RendererRender { name: "a".into(), png_path: &p_a },
                RendererRender { name: "b".into(), png_path: &p_b },
            ],
            0.99,
        )
        .unwrap();

        assert!(result.consensus_passed);
        assert_eq!(result.agreement_count, vec![1, 1]);
        assert_eq!(result.ssim_matrix.len(), 2);
        assert_eq!(result.ssim_matrix[0].len(), 2);
    }

    #[test]
    fn single_renderer_rejected_as_invalid_input() {
        let dir = tempfile::tempdir().unwrap();
        let p_a = make_path(dir.path(), "a.png");
        write_solid_png(&p_a, 32, 32, [10, 10, 10]);

        let err = consensus_diff(
            "solo",
            &[RendererRender { name: "a".into(), png_path: &p_a }],
            0.99,
        )
        .unwrap_err();
        assert!(matches!(err, SsimError::Compute(_)));
    }

    #[test]
    fn dimension_mismatch_propagates_as_ssim_error() {
        let dir = tempfile::tempdir().unwrap();
        let p_a = make_path(dir.path(), "a.png");
        let p_b = make_path(dir.path(), "b.png");
        write_solid_png(&p_a, 32, 32, [10, 10, 10]);
        write_solid_png(&p_b, 64, 64, [10, 10, 10]);

        let err = consensus_diff(
            "size-mismatch",
            &[
                RendererRender { name: "a".into(), png_path: &p_a },
                RendererRender { name: "b".into(), png_path: &p_b },
            ],
            0.99,
        )
        .unwrap_err();
        assert!(matches!(err, SsimError::Dimension(..)));
    }
}
