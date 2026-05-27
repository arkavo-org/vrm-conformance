//! Tests the runner-side helper that applies vrm-normalize when
//! as_spec_version is requested on a dump.

use std::collections::HashMap;
use vrm_ops::SpecVersion;
use vrm_runner::execute::apply_normalization_if_requested;

fn weights_v0() -> HashMap<String, f32> {
    let mut m = HashMap::new();
    m.insert("joy".into(), 100.0);
    m
}

#[test]
fn no_as_spec_version_returns_native() {
    let w = weights_v0();
    let out = apply_normalization_if_requested(&w, SpecVersion::V0, None).unwrap();
    assert_eq!(out.get("joy"), Some(&100.0));
    assert!(!out.contains_key("happy"));
}

#[test]
fn as_spec_version_v1_against_v0_normalizes() {
    let w = weights_v0();
    let out = apply_normalization_if_requested(&w, SpecVersion::V0, Some(SpecVersion::V1)).unwrap();
    assert!((out.get("happy").copied().unwrap_or(0.0) - 1.0).abs() < 1e-6);
    assert!(!out.contains_key("joy"));
}

#[test]
fn as_spec_version_v0_against_v1_rejected_with_code_32001() {
    let mut w = HashMap::new();
    w.insert("happy".into(), 1.0);
    let result = apply_normalization_if_requested(&w, SpecVersion::V1, Some(SpecVersion::V0));
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("-32001"),
        "err should mention -32001; got {err_msg}"
    );
}

#[test]
fn as_spec_version_v0_against_v0_returns_native() {
    // Asking for V0 against a V0 asset is a no-op (asset is already V0).
    let w = weights_v0();
    let out = apply_normalization_if_requested(&w, SpecVersion::V0, Some(SpecVersion::V0)).unwrap();
    assert_eq!(out.get("joy"), Some(&100.0));
}

#[test]
fn as_spec_version_v1_against_v1_returns_native() {
    let mut w = HashMap::new();
    w.insert("happy".into(), 1.0);
    let out = apply_normalization_if_requested(&w, SpecVersion::V1, Some(SpecVersion::V1)).unwrap();
    assert_eq!(out.get("happy"), Some(&1.0));
}
