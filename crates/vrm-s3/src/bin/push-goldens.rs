use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;
use vrm_s3::{
    manifest::{Manifest, SubmissionMetadata},
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
    /// S3 bucket.
    #[arg(long, env = "VRM_GOLDENS_BUCKET")]
    bucket: String,
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
}

#[tokio::main]
async fn main() -> Result<()> {
    let a = Args::parse();
    let opts = PushOptions {
        bucket: a.bucket,
        key_prefix: a.key_prefix,
        renderer_name: a.renderer_name,
        renderer_version: a.renderer_version,
        git_hash: a.git_hash,
        metadata: SubmissionMetadata {
            os: a.os,
            os_version: a.os_version,
            gpu_vendor: a.gpu_vendor,
            gpu_model: a.gpu_model,
            driver_version: a.driver_version,
            build_flags: a.build_flags,
        },
    };
    let entry = push_png(&a.file, &a.test_id, &opts).await?;

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
