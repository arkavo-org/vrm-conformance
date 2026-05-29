//! Cross-variant SSIM: assert two renders of the SAME renderer DIFFER.
//!
//! The inverse of the normal conformance diff (which passes when SSIM is
//! high). Used by the doubleSided back-face-culling spec test: the
//! doubleSided=false render (culled → all-background) and the doubleSided=true
//! render (surface shown) of a conformant renderer MUST diverge. Pass iff
//! their SSIM is at or below `max_ssim`.

use crate::ssim::{ssim_pngs, SsimError};
use camino::Utf8Path;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CrossVariantResult {
    pub ssim: f64,
    pub max_ssim: f64,
    pub passed: bool,
}

/// Compare two renders and pass iff they visibly DIFFER (ssim <= max_ssim).
pub fn cross_variant_diff(
    false_png: &Utf8Path,
    true_png: &Utf8Path,
    max_ssim: f64,
) -> Result<CrossVariantResult, SsimError> {
    let ssim = ssim_pngs(false_png, true_png)?;
    Ok(CrossVariantResult {
        ssim,
        max_ssim,
        passed: ssim <= max_ssim,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    /// Write a 16×16 magenta PNG, optionally with a gray filled square in the
    /// centre (mimicking a rendered quad over the magenta background sentinel).
    fn write_png(
        dir: &camino::Utf8Path,
        name: &str,
        with_center_square: bool,
    ) -> camino::Utf8PathBuf {
        let mut img = RgbImage::new(16, 16);
        for px in img.pixels_mut() {
            *px = Rgb([255, 0, 255]);
        }
        if with_center_square {
            for y in 4..12 {
                for x in 4..12 {
                    img.put_pixel(x, y, Rgb([128, 128, 128]));
                }
            }
        }
        let path = dir.join(name);
        img.save(path.as_std_path()).unwrap();
        path
    }

    #[test]
    fn identical_renders_fail_must_differ_assertion() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = camino::Utf8Path::from_path(tmp.path()).unwrap();
        let a = write_png(dir, "a.png", false);
        let b = write_png(dir, "b.png", false);
        let r = cross_variant_diff(&a, &b, 0.85).unwrap();
        assert!(
            !r.passed,
            "identical renders must NOT pass a must-differ assertion (ssim={})",
            r.ssim
        );
    }

    #[test]
    fn divergent_renders_pass_must_differ_assertion() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = camino::Utf8Path::from_path(tmp.path()).unwrap();
        let culled = write_png(dir, "culled.png", false); // all background
        let shown = write_png(dir, "shown.png", true); // quad in frame
        let r = cross_variant_diff(&culled, &shown, 0.85).unwrap();
        assert!(
            r.passed,
            "culled (background) vs shown (quad) must diverge below 0.85 (ssim={})",
            r.ssim
        );
    }
}
