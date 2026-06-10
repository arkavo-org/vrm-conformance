//! Blank-frame gate: a render whose pixels are (almost) entirely one color
//! is an adapter error frame, never a pass.
//!
//! Motivation (2026-06-10): a VRMMetalKit build with unusable Metal
//! pipelines (stale-toolchain metallib slices, upstream VMK#336) renders
//! every plan as the bare clear color — solid magenta — while the adapter
//! still reports `ok: true`. Reference-less `execute-test-plan` runs then
//! summarize `overall_passed: true`, and a bootstrap run could even push the
//! blank frame as a *golden*. This gate makes that class of failure loud at
//! the chokepoint every render flows through, regardless of adapter or
//! root cause.
//!
//! Calibration against the local golden corpus: real renders top out at a
//! dominant-color fraction of ~0.57 (procedural sphere sweeps over a flat
//! background); error frames are exactly 1.0. The threshold is therefore
//! not delicate.

use anyhow::{bail, Context, Result};
use camino::Utf8Path;
use std::collections::HashMap;

/// Fraction of pixels sharing the single most common RGBA value at or above
/// which a render is rejected as a blank/error frame.
pub const BLANK_FRAME_THRESHOLD: f64 = 0.995;

/// Returns the fraction of pixels equal to the image's most common RGBA
/// value, in `0.0..=1.0`.
pub fn dominant_color_fraction(png: &Utf8Path) -> Result<f64> {
    let img = image::open(png.as_std_path())
        .with_context(|| format!("blank-frame gate: cannot decode {png}"))?
        .to_rgba8();
    let total = u64::from(img.width()) * u64::from(img.height());
    if total == 0 {
        bail!("blank-frame gate: zero-pixel image {png}");
    }
    let mut counts: HashMap<[u8; 4], u64> = HashMap::new();
    for p in img.pixels() {
        *counts.entry(p.0).or_insert(0) += 1;
    }
    let max = counts.values().copied().max().unwrap_or(0);
    #[allow(clippy::cast_precision_loss)]
    Ok(max as f64 / total as f64)
}

/// Hard-fails when `png` is a blank/error frame. `what` names the artifact
/// in the error message (plan id, sequence frame, …).
pub fn reject_blank_frame(png: &Utf8Path, what: &str) -> Result<()> {
    let fraction = dominant_color_fraction(png)?;
    if fraction >= BLANK_FRAME_THRESHOLD {
        bail!(
            "blank render rejected: {what} ({png}) is {:.2}% a single color \
             (threshold {:.1}%). This is an error frame — an adapter with \
             unusable pipelines renders the bare clear color — not a valid \
             render; refusing to let it pass or become a golden.",
            fraction * 100.0,
            BLANK_FRAME_THRESHOLD * 100.0
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use image::{Rgba, RgbaImage};

    fn write_png(name: &str, img: &RgbaImage) -> Utf8PathBuf {
        let dir = Utf8PathBuf::from_path_buf(std::env::temp_dir()).unwrap();
        let path = dir.join(format!("blank_gate_test_{name}.png"));
        img.save(path.as_std_path()).unwrap();
        path
    }

    #[test]
    fn solid_magenta_is_rejected() {
        let img = RgbaImage::from_pixel(64, 64, Rgba([255, 0, 255, 255]));
        let path = write_png("solid", &img);
        assert!((dominant_color_fraction(&path).unwrap() - 1.0).abs() < 1e-9);
        let err = reject_blank_frame(&path, "solid-test").unwrap_err();
        assert!(err.to_string().contains("blank render rejected"));
    }

    #[test]
    fn near_solid_above_threshold_is_rejected() {
        // 64x64 = 4096 px; 8 off-pixels → 99.80% dominant, above 99.5%.
        let mut img = RgbaImage::from_pixel(64, 64, Rgba([255, 0, 255, 255]));
        for x in 0..8 {
            img.put_pixel(x, 0, Rgba([0, 0, 0, 255]));
        }
        let path = write_png("near_solid", &img);
        assert!(reject_blank_frame(&path, "near-solid-test").is_err());
    }

    #[test]
    fn real_content_passes() {
        // Half background, half "avatar" — far below the threshold.
        let mut img = RgbaImage::from_pixel(64, 64, Rgba([40, 40, 60, 255]));
        for y in 0..64 {
            for x in 0..32 {
                img.put_pixel(x, y, Rgba([x as u8 * 4, y as u8 * 3, 128, 255]));
            }
        }
        let path = write_png("content", &img);
        assert!(dominant_color_fraction(&path).unwrap() < 0.51);
        assert!(reject_blank_frame(&path, "content-test").is_ok());
    }

    #[test]
    fn sphere_on_flat_background_passes() {
        // Mimics the procedural sweep corpus shape: ~57% flat background.
        let mut img = RgbaImage::from_pixel(100, 100, Rgba([54, 54, 54, 255]));
        for y in 0..100i32 {
            for x in 0..100i32 {
                let (dx, dy) = (x - 50, y - 50);
                if dx * dx + dy * dy < 37 * 37 {
                    img.put_pixel(x as u32, y as u32, Rgba([200, 30, 30, 255]));
                }
            }
        }
        let path = write_png("sphere", &img);
        let f = dominant_color_fraction(&path).unwrap();
        assert!(f < BLANK_FRAME_THRESHOLD, "fraction {f}");
        assert!(reject_blank_frame(&path, "sphere-test").is_ok());
    }
}
