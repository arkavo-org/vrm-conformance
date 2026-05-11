use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;
use vrm_s3::{
    manifest::{Manifest, ManifestEntry, SubmissionMetadata},
    push_pull::{push_png, PushOptions},
};

#[derive(Debug, Parser)]
struct Args {
    /// PNG file to upload.
    #[arg(long)]
    file: Utf8PathBuf,
    /// Test ID this PNG belongs to.
    #[arg(long)]
    test_id: String,
    /// S3 bucket. Required unless `--local` is set.
    #[arg(long, env = "VRM_GOLDENS_BUCKET")]
    bucket: Option<String>,
    /// S3 key prefix.
    #[arg(long, default_value = "v0.1")]
    key_prefix: String,
    /// Renderer name.
    #[arg(long)]
    renderer_name: String,
    #[arg(long)]
    renderer_version: String,
    #[arg(long)]
    git_hash: String,
    #[arg(long)]
    os: String,
    #[arg(long)]
    os_version: String,
    #[arg(long)]
    gpu_vendor: String,
    #[arg(long)]
    gpu_model: String,
    #[arg(long, default_value = "")]
    driver_version: String,
    #[arg(long, default_value = "release")]
    build_flags: String,
    /// Path to manifest.json to update in place.
    #[arg(long, default_value = "goldens/manifest.json")]
    manifest: Utf8PathBuf,
    /// Skip the S3 upload entirely and write `file://<absolute-path>` as
    /// the image_url. The local PNG file is left untouched; only the
    /// manifest gets a new entry. Used for bootstrap workflows where the
    /// PNGs live on disk and S3 publication is a separate step.
    #[arg(long)]
    local: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let a = Args::parse();
    let metadata = SubmissionMetadata {
        os: a.os,
        os_version: a.os_version,
        gpu_vendor: a.gpu_vendor,
        gpu_model: a.gpu_model,
        driver_version: a.driver_version,
        build_flags: a.build_flags,
    };

    let entry = if a.local {
        local_manifest_entry(
            &a.file,
            &a.test_id,
            &a.renderer_name,
            &a.renderer_version,
            &a.git_hash,
            metadata,
        )?
    } else {
        let bucket = a
            .bucket
            .ok_or_else(|| anyhow::anyhow!("missing --bucket / VRM_GOLDENS_BUCKET (or use --local)"))?;
        let opts = PushOptions {
            bucket,
            key_prefix: a.key_prefix,
            renderer_name: a.renderer_name,
            renderer_version: a.renderer_version,
            git_hash: a.git_hash,
            metadata,
        };
        push_png(&a.file, &a.test_id, &opts).await?
    };

    let mut m: Manifest = if a.manifest.exists() {
        serde_json::from_str(&std::fs::read_to_string(&a.manifest)?)?
    } else {
        Manifest::empty()
    };
    m.upsert(entry.clone());

    if let Some(p) = a.manifest.parent() {
        std::fs::create_dir_all(p.as_std_path())?;
    }
    std::fs::write(&a.manifest, serde_json::to_vec_pretty(&m)?)?;

    println!("{}", serde_json::to_string(&entry)?);
    Ok(())
}

/// Build a manifest entry pointing at the local PNG via a `file://` URL,
/// without touching S3. BLAKE3 is computed from the file bytes, byte-size
/// reflects the on-disk file, and submitted_at is the current UTC time.
fn local_manifest_entry(
    file: &Utf8PathBuf,
    test_id: &str,
    renderer_name: &str,
    renderer_version: &str,
    git_hash: &str,
    metadata: SubmissionMetadata,
) -> Result<ManifestEntry> {
    let bytes = std::fs::read(file.as_std_path())?;
    let hash = blake3::hash(&bytes);
    let blake3_str = format!("blake3:{}", hash.to_hex());

    // Resolve to absolute path so the manifest stays valid regardless of
    // the consumer's working directory.
    let abs = std::fs::canonicalize(file.as_std_path())?;
    let abs_utf8 =
        Utf8PathBuf::from_path_buf(abs).map_err(|p| anyhow::anyhow!("non-UTF8 path: {p:?}"))?;
    let image_url = format!("file://{abs_utf8}");

    Ok(ManifestEntry {
        test_id: test_id.into(),
        renderer_name: renderer_name.into(),
        renderer_version: renderer_version.into(),
        git_hash: git_hash.into(),
        metadata,
        image_url,
        image_blake3: blake3_str,
        byte_size: bytes.len() as u64,
        submitted_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string()),
    })
}
