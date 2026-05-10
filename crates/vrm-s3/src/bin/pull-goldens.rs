//! Pull every entry in goldens/manifest.json from S3 to a local mirror,
//! verifying BLAKE3 against the manifest's claim for each file. Exits
//! non-zero on the first hash mismatch (do not silently overwrite a
//! local file with bytes that don't match the manifest's content ref).

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use clap::Parser;
use serde_json::json;
use vrm_s3::{
    manifest::{Manifest, ManifestEntry},
    push_pull::{pull_png, verify_blake3},
};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Pull goldens listed in manifest.json from S3 to a local mirror"
)]
struct Args {
    /// Path to the manifest file. Defaults to goldens/manifest.json in cwd.
    #[arg(long, default_value = "goldens/manifest.json")]
    manifest: Utf8PathBuf,

    /// Local mirror directory. PNGs land at <output-dir>/<test_id>/<renderer>.png.
    #[arg(long)]
    output_dir: Utf8PathBuf,

    /// NDJSON progress on stderr; final JSON summary on stdout. If unset,
    /// human-readable text only.
    #[arg(long)]
    json: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let manifest_bytes = std::fs::read(args.manifest.as_std_path())
        .with_context(|| format!("read manifest: {}", args.manifest))?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("parse manifest: {}", args.manifest))?;

    std::fs::create_dir_all(args.output_dir.as_std_path())?;

    let total = manifest.entries.len();
    let mut pulled: Vec<Utf8PathBuf> = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();

    for (i, entry) in manifest.entries.iter().enumerate() {
        let dest = compute_dest(&args.output_dir, entry);
        if args.json {
            let evt = json!({
                "event": "progress",
                "op": "pull-goldens",
                "index": i,
                "total": total,
                "test_id": entry.test_id,
                "renderer_name": entry.renderer_name,
                "image_url": entry.image_url,
                "dest": dest,
            });
            eprintln!("{}", serde_json::to_string(&evt)?);
        } else {
            eprintln!(
                "[{:3}/{}] {} ({}) → {}",
                i + 1,
                total,
                entry.test_id,
                entry.renderer_name,
                dest
            );
        }

        match pull_one(&entry.image_url, &dest, &entry.image_blake3).await {
            Ok(()) => pulled.push(dest),
            Err(e) => {
                failures.push((
                    format!("{}/{}", entry.test_id, entry.renderer_name),
                    e.to_string(),
                ));
            }
        }
    }

    if args.json {
        let summary = json!({
            "ok": failures.is_empty(),
            "pulled": pulled,
            "failures": failures
                .iter()
                .map(|(k, v)| json!({"entry": k, "error": v}))
                .collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string(&summary)?);
    } else {
        println!(
            "pulled {} entries to {}; {} failures",
            pulled.len(),
            args.output_dir,
            failures.len()
        );
    }

    if !failures.is_empty() {
        for (entry, err) in &failures {
            eprintln!("FAIL {entry}: {err}");
        }
        anyhow::bail!("{} of {} pulls failed", failures.len(), total);
    }

    Ok(())
}

async fn pull_one(image_url: &str, dest: &Utf8PathBuf, expected_blake3: &str) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent.as_std_path())?;
    }
    pull_png(image_url, dest).await?;
    let bytes = std::fs::read(dest.as_std_path())
        .with_context(|| format!("read downloaded file: {dest}"))?;
    verify_blake3(&bytes, expected_blake3)
        .with_context(|| format!("verify {dest} against manifest"))?;
    Ok(())
}

fn compute_dest(output_dir: &Utf8PathBuf, entry: &ManifestEntry) -> Utf8PathBuf {
    output_dir
        .join(&entry.test_id)
        .join(format!("{}.png", entry.renderer_name))
}
