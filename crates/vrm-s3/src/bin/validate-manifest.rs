use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use clap::Parser;
use vrm_s3::manifest::Manifest;
use vrm_s3::validation::validate_entries;

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

    let errors = validate_entries(&m.entries);
    if !errors.is_empty() {
        for e in &errors {
            eprintln!("{e}");
        }
        bail!("{} errors in manifest", errors.len());
    }
    eprintln!("manifest OK ({} entries)", m.entries.len());
    Ok(())
}
