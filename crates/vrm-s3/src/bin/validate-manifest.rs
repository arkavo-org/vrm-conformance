use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use clap::Parser;
use vrm_s3::manifest::Manifest;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "goldens/manifest.json")]
    manifest: Utf8PathBuf,
}

fn main() -> Result<()> {
    let a = Args::parse();
    if !a.manifest.exists() {
        eprintln!("manifest not present, treating as empty");
        return Ok(());
    }
    let raw = std::fs::read_to_string(&a.manifest)?;
    let m: Manifest = serde_json::from_str(&raw)?;

    let mut errors = Vec::new();
    for (i, e) in m.entries.iter().enumerate() {
        if !e.image_url.starts_with("s3://") {
            errors.push(format!(
                "[{i}] image_url must start with s3://: {}",
                e.image_url
            ));
        }
        if !e.image_blake3.starts_with("blake3:") {
            errors.push(format!("[{i}] image_blake3 must start with blake3:"));
        }
        match (&e.positions_url, &e.positions_blake3) {
            (Some(_url), Some(hash)) => {
                if !hash.starts_with("blake3:") || hash.len() != "blake3:".len() + 64 {
                    errors.push(format!(
                        "[{i}] positions_blake3 malformed (expected blake3:<64-hex>): {hash}"
                    ));
                }
            }
            (Some(_), None) => {
                errors.push(format!(
                    "[{i}] positions_url set without positions_blake3 ({})",
                    e.test_id
                ));
            }
            (None, Some(_)) => {
                errors.push(format!(
                    "[{i}] positions_blake3 set without positions_url ({})",
                    e.test_id
                ));
            }
            (None, None) => {}
        }
        for (name, val) in [
            ("os", &e.metadata.os),
            ("os_version", &e.metadata.os_version),
            ("gpu_vendor", &e.metadata.gpu_vendor),
            ("gpu_model", &e.metadata.gpu_model),
        ] {
            if val.trim().is_empty() {
                errors.push(format!("[{i}] metadata.{name} must be non-empty"));
            }
        }
        if e.git_hash.len() < 7 {
            errors.push(format!("[{i}] git_hash too short: {}", e.git_hash));
        }
    }

    if !errors.is_empty() {
        for e in &errors {
            eprintln!("{e}");
        }
        bail!("{} errors in manifest", errors.len());
    }
    eprintln!("manifest OK ({} entries)", m.entries.len());
    Ok(())
}
