//! SSIM (Structural Similarity) over RGB PNGs.

use camino::Utf8Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SsimError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("image decode: {0}")]
    Decode(#[from] image::ImageError),
    #[error("dimension mismatch: {0}x{1} vs {2}x{3}")]
    Dimension(u32, u32, u32, u32),
    #[error("ssim computation failed: {0}")]
    Compute(String),
}

pub fn ssim_pngs(a: &Utf8Path, b: &Utf8Path) -> Result<f64, SsimError> {
    let img_a = image::open(a.as_std_path())?.to_rgb8();
    let img_b = image::open(b.as_std_path())?.to_rgb8();

    if img_a.dimensions() != img_b.dimensions() {
        let (aw, ah) = img_a.dimensions();
        let (bw, bh) = img_b.dimensions();
        return Err(SsimError::Dimension(aw, ah, bw, bh));
    }

    let result = image_compare::rgb_hybrid_compare(&img_a, &img_b)
        .map_err(|e| SsimError::Compute(e.to_string()))?;

    Ok(result.score)
}
