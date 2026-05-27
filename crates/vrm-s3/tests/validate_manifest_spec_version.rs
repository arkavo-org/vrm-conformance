//! Tests that the manifest validator surfaces `spec_version` mismatches
//! between the declared field and the spec version inferred from test_id naming.
//!
//! Convention: VRM 0.x test IDs contain `_v0_` somewhere (e.g. `mtoon_basic_v0_lit_001`
//! or end with `_v0`). VRM 1.0 test IDs don't.

use vrm_ops::SpecVersion;
use vrm_s3::manifest::{ManifestEntry, ManifestEntryKind, SubmissionMetadata};
use vrm_s3::validation::validate_entries;

fn meta() -> SubmissionMetadata {
    SubmissionMetadata {
        os: "macos".into(),
        os_version: "14".into(),
        gpu_vendor: "Apple".into(),
        gpu_model: "M2".into(),
        driver_version: "Metal 3".into(),
        build_flags: "release".into(),
    }
}

fn entry_with(spec_version: SpecVersion, test_id: &str) -> ManifestEntry {
    ManifestEntry {
        test_id: test_id.into(),
        spec_version,
        renderer_name: "r".into(),
        renderer_version: "0".into(),
        git_hash: "abcdef1".into(),
        metadata: meta(),
        kind: ManifestEntryKind::Image,
        image_url: Some(format!("s3://bucket/{test_id}.png")),
        image_blake3: Some(format!("blake3:{}", "a".repeat(64))),
        byte_size: Some(1024),
        submitted_at: "2026-05-26T00:00:00Z".into(),
        positions_url: None,
        positions_blake3: None,
        vrma_url: None,
        vrma_blake3: None,
        sequence: None,
    }
}

#[test]
fn rejects_v0_test_id_with_v1_spec_version() {
    // test_id contains _v0_ but spec_version says 1.0 — should be rejected.
    let e = entry_with(SpecVersion::V1, "mtoon_basic_v0_lit_001");
    let errs = validate_entries(&[e]);
    assert!(
        !errs.is_empty(),
        "expected validation error for v0-named test_id with V1 spec_version; got no errors"
    );
    assert!(
        errs.iter().any(|s| s.contains("spec_version")),
        "expected spec_version mismatch in errors; got {errs:?}"
    );
}

#[test]
fn accepts_v0_test_id_with_v0_spec_version() {
    let e = entry_with(SpecVersion::V0, "mtoon_basic_v0_lit_001");
    let errs = validate_entries(&[e]);
    assert!(errs.is_empty(), "expected no errors; got {errs:?}");
}

#[test]
fn accepts_v1_test_id_with_v1_spec_version() {
    let e = entry_with(SpecVersion::V1, "mtoon_basic_lit_001");
    let errs = validate_entries(&[e]);
    assert!(errs.is_empty(), "expected no errors; got {errs:?}");
}

#[test]
fn rejects_v1_test_id_with_v0_spec_version() {
    // test_id has no _v0_ but spec_version says 0.x — should be rejected.
    let e = entry_with(SpecVersion::V0, "mtoon_basic_lit_001");
    let errs = validate_entries(&[e]);
    assert!(
        !errs.is_empty(),
        "expected validation error for v1-named test_id with V0 spec_version; got no errors"
    );
    assert!(
        errs.iter().any(|s| s.contains("spec_version")),
        "expected spec_version mismatch in errors; got {errs:?}"
    );
}

#[test]
fn rejects_v0_suffix_only_with_v1_spec_version() {
    // test_id ends with _v0 (no trailing segment) — still a VRM 0.x id.
    let e = entry_with(SpecVersion::V1, "expressions_preset_basic_v0");
    let errs = validate_entries(&[e]);
    assert!(
        !errs.is_empty(),
        "expected validation error for _v0-suffixed test_id with V1 spec_version"
    );
}

#[test]
fn accepts_v0_suffix_only_with_v0_spec_version() {
    let e = entry_with(SpecVersion::V0, "expressions_preset_basic_v0");
    let errs = validate_entries(&[e]);
    assert!(errs.is_empty(), "expected no errors; got {errs:?}");
}
