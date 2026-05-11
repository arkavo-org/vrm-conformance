//! Tests for the file:// URL path in `pull_png`. Used by the bootstrap
//! workflow: when AWS creds aren't configured, push-goldens writes file://
//! URLs into the manifest, and pull-goldens has to be able to mirror them
//! to a local cache without hitting S3.

use camino::Utf8PathBuf;
use vrm_s3::push_pull::pull_png;

#[tokio::test]
async fn pull_png_handles_file_url_by_copy() {
    let dir = tempfile::tempdir().unwrap();
    let src = Utf8PathBuf::from_path_buf(dir.path().join("source.png")).unwrap();
    let dest_dir = dir.path().join("mirror");
    let dest = Utf8PathBuf::from_path_buf(dest_dir.join("nested").join("copy.png")).unwrap();

    // Write a small PNG-like payload (real bytes don't matter for the copy
    // contract; only the round-trip equality does).
    let payload: Vec<u8> = (0u8..=255u8).cycle().take(2048).collect();
    std::fs::write(src.as_std_path(), &payload).unwrap();

    let url = format!("file://{src}");
    pull_png(&url, &dest).await.expect("pull_png should succeed for file:// URL");

    let copied = std::fs::read(dest.as_std_path()).unwrap();
    assert_eq!(
        copied, payload,
        "pulled bytes must match source; copy is byte-for-byte"
    );
}

#[tokio::test]
async fn pull_png_rejects_unknown_scheme() {
    let dir = tempfile::tempdir().unwrap();
    let dest = Utf8PathBuf::from_path_buf(dir.path().join("dest.png")).unwrap();

    // http:// is neither file:// nor s3://. pull_png currently falls
    // through to the s3:// parser, which rejects this with a clear error.
    let err = pull_png("http://example.com/x.png", &dest)
        .await
        .expect_err("non-file:// non-s3:// URL must error");
    assert!(
        err.to_string().contains("s3://"),
        "error should mention s3:// expectation, got: {err}"
    );
}
