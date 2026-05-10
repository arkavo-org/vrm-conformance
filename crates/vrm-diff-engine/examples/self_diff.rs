//! Self-diff sanity example.
//!
//! Loads a PNG and computes its SSIM against itself. Should always print
//! `self-SSIM = 1` (or extremely close to 1.0). Useful as a smoke check that
//! the diff engine is wired up and that a renderer-produced PNG is at least
//! readable as a valid image.

use camino::Utf8PathBuf;

fn main() -> anyhow::Result<()> {
    let p = std::env::args()
        .nth(1)
        .map(Utf8PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("usage: self_diff <png>"))?;
    let score = vrm_diff_engine::ssim::ssim_pngs(&p, &p)?;
    println!("self-SSIM = {score}");
    Ok(())
}
