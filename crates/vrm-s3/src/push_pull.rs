//! Push: upload a PNG to S3, compute BLAKE3, return a ManifestEntry.
//! Pull: download by URL into a local file path.

use crate::manifest::{ManifestEntry, SubmissionMetadata};
use anyhow::Result;
use aws_sdk_s3::primitives::ByteStream;
use camino::Utf8Path;

#[derive(Debug, Clone)]
pub struct PushOptions {
    pub bucket: String,
    pub key_prefix: String,
    pub renderer_name: String,
    pub renderer_version: String,
    pub git_hash: String,
    pub metadata: SubmissionMetadata,
}

pub async fn push_png(file: &Utf8Path, test_id: &str, opts: &PushOptions) -> Result<ManifestEntry> {
    let bytes = std::fs::read(file.as_std_path())?;
    let hash = blake3::hash(&bytes);
    let blake3_str = format!("blake3:{}", hash.to_hex());

    let key = format!(
        "{}/{}/{}_{}.png",
        opts.key_prefix.trim_matches('/'),
        opts.renderer_name,
        test_id,
        &hash.to_hex().to_string()[..16]
    );

    let cfg = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_s3::Client::new(&cfg);

    client
        .put_object()
        .bucket(&opts.bucket)
        .key(&key)
        .body(ByteStream::from(bytes.clone()))
        .content_type("image/png")
        .send()
        .await?;

    Ok(ManifestEntry {
        test_id: test_id.into(),
        renderer_name: opts.renderer_name.clone(),
        renderer_version: opts.renderer_version.clone(),
        git_hash: opts.git_hash.clone(),
        metadata: opts.metadata.clone(),
        image_url: format!("s3://{}/{}", opts.bucket, key),
        image_blake3: blake3_str,
        byte_size: bytes.len() as u64,
        submitted_at: now_rfc3339(),
    })
}

pub async fn pull_png(url: &str, dest: &Utf8Path) -> Result<()> {
    let (bucket, key) = parse_s3_url(url)?;
    let cfg = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_s3::Client::new(&cfg);

    let obj = client.get_object().bucket(bucket).key(key).send().await?;
    let bytes = obj.body.collect().await?.into_bytes();
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent.as_std_path())?;
    }
    std::fs::write(dest.as_std_path(), bytes)?;
    Ok(())
}

fn parse_s3_url(url: &str) -> Result<(&str, &str)> {
    let stripped = url
        .strip_prefix("s3://")
        .ok_or_else(|| anyhow::anyhow!("expected s3:// URL, got {url}"))?;
    let (bucket, key) = stripped
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("malformed s3 url: missing key: {url}"))?;
    Ok((bucket, key))
}

/// Emit current UTC time as an RFC 3339 / ISO 8601 string.
///
/// RFC-0002's manifest schema requires real ISO 8601 timestamps, so this uses
/// the `time` crate rather than the `@unix:<secs>` placeholder the plan ships
/// with.
fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}
