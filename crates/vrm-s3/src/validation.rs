use crate::manifest::{ManifestEntry, ManifestEntryKind};
use vrm_ops::SpecVersion;

const BLAKE3_LEN: usize = "blake3:".len() + 64;

/// Returns `true` when the `test_id` naming convention signals a VRM 0.x asset.
///
/// Convention: a `test_id` is considered 0.x-named when it contains the
/// substring `_v0_` anywhere, or ends with the suffix `_v0`.
fn test_id_implies_v0(test_id: &str) -> bool {
    test_id.contains("_v0_") || test_id.ends_with("_v0")
}

/// Validate a manifest's entries. Returns a list of errors (empty = valid).
///
/// Per RFC-0004 and existing manifest conventions:
/// - Image-kind entries must have populated `image_url` + `image_blake3`.
/// - Sequence-kind entries must have a populated sequence block with consistent
///   `frame_count`, well-formed per-frame URLs and hashes, and must NOT carry
///   the top-level image fields (`image_url`, `image_blake3`, `byte_size`).
/// - Both kinds: `positions_url`/`positions_blake3` are paired; metadata fields
///   must be non-empty; `git_hash` must be at least 7 characters.
pub fn validate_entries(entries: &[ManifestEntry]) -> Vec<String> {
    let mut errors = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        validate_entry(i, e, &mut errors);
    }
    errors
}

fn validate_entry(i: usize, e: &ManifestEntry, errors: &mut Vec<String>) {
    match e.kind {
        ManifestEntryKind::Image => {
            let image_url = e.image_url.as_deref().unwrap_or("");
            let image_blake3 = e.image_blake3.as_deref().unwrap_or("");
            if !image_url.starts_with("s3://") {
                errors.push(format!(
                    "[{i}] image_url must start with s3://: {image_url}"
                ));
            }
            if !image_blake3.starts_with("blake3:") {
                errors.push(format!("[{i}] image_blake3 must start with blake3:"));
            }
            if e.image_url.is_none() {
                errors.push(format!("[{i}] kind=image but image_url is None"));
            }
            if e.image_blake3.is_none() {
                errors.push(format!("[{i}] kind=image but image_blake3 is None"));
            }
        }
        ManifestEntryKind::Sequence => {
            // Sequence entries must not carry top-level image fields.
            if e.image_url.is_some() {
                errors.push(format!(
                    "[{i}] kind=sequence but image_url is set ({})",
                    e.test_id
                ));
            }
            if e.image_blake3.is_some() {
                errors.push(format!(
                    "[{i}] kind=sequence but image_blake3 is set ({})",
                    e.test_id
                ));
            }
            if e.byte_size.is_some() {
                errors.push(format!(
                    "[{i}] kind=sequence but byte_size is set ({})",
                    e.test_id
                ));
            }

            let Some(seq) = &e.sequence else {
                errors.push(format!(
                    "[{i}] kind=sequence but sequence block is missing ({})",
                    e.test_id
                ));
                return;
            };

            if seq.frames.is_empty() {
                errors.push(format!("[{i}] sequence has 0 frames ({})", e.test_id));
            }
            if seq.frame_count as usize != seq.frames.len() {
                errors.push(format!(
                    "[{i}] frame_count ({}) != frames.len() ({}) ({})",
                    seq.frame_count,
                    seq.frames.len(),
                    e.test_id
                ));
            }

            for (j, frame) in seq.frames.iter().enumerate() {
                if !frame.image_url.starts_with("s3://") {
                    errors.push(format!(
                        "[{i}/frame {j}] image_url must start with s3://: {}",
                        frame.image_url
                    ));
                }
                if !frame.blake3.starts_with("blake3:") || frame.blake3.len() != BLAKE3_LEN {
                    errors.push(format!(
                        "[{i}/frame {j}] blake3 malformed (expected blake3:<64-hex>): {}",
                        frame.blake3
                    ));
                }
                if frame.index != j as u32 {
                    errors.push(format!(
                        "[{i}/frame {j}] index field ({}) doesn't match position ({}) ({})",
                        frame.index, j, e.test_id
                    ));
                }
            }

            // muxed_url and muxed_blake3 are paired — either both Some or both None.
            match (&seq.muxed_url, &seq.muxed_blake3) {
                (Some(_), Some(hash)) => {
                    if !hash.starts_with("blake3:") || hash.len() != BLAKE3_LEN {
                        errors.push(format!(
                            "[{i}] muxed_blake3 malformed (expected blake3:<64-hex>): {hash}"
                        ));
                    }
                }
                (Some(_), None) => {
                    errors.push(format!(
                        "[{i}] muxed_url set without muxed_blake3 ({})",
                        e.test_id
                    ));
                }
                (None, Some(_)) => {
                    errors.push(format!(
                        "[{i}] muxed_blake3 set without muxed_url ({})",
                        e.test_id
                    ));
                }
                (None, None) => {}
            }
        }
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

    // Cross-check: test_id naming convention must agree with spec_version field.
    // A test_id is 0.x-named when it contains `_v0_` or ends with `_v0`.
    // All other test_ids are treated as 1.0-named.
    let id_is_v0 = test_id_implies_v0(&e.test_id);
    match (id_is_v0, e.spec_version) {
        (true, SpecVersion::V1) => {
            errors.push(format!(
                "[{i}] spec_version mismatch: test_id '{}' implies 0.x \
                 (contains _v0_ or ends with _v0) but spec_version is 1.0",
                e.test_id
            ));
        }
        (false, SpecVersion::V0) => {
            errors.push(format!(
                "[{i}] spec_version mismatch: test_id '{}' implies 1.0 \
                 (no _v0_ segment) but spec_version is 0.x",
                e.test_id
            ));
        }
        _ => {}
    }
}
