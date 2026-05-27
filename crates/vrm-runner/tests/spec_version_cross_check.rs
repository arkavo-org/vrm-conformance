//! Slice 1 third hard-error gate: runner cross-checks the adapter's
//! reported source_spec_version against the test plan's declared
//! spec_version. Mismatch aborts the run with a clear error.

use vrm_ops::SpecVersion;
use vrm_runner::execute::cross_check_source_spec_version;

#[test]
fn ok_when_plan_and_adapter_agree_on_v0() {
    assert!(
        cross_check_source_spec_version(SpecVersion::V0, SpecVersion::V0, "test_id").is_ok(),
        "V0/V0 should be accepted"
    );
}

#[test]
fn ok_when_plan_and_adapter_agree_on_v1() {
    assert!(
        cross_check_source_spec_version(SpecVersion::V1, SpecVersion::V1, "test_id").is_ok(),
        "V1/V1 should be accepted"
    );
}

#[test]
fn err_when_plan_v0_but_adapter_reports_v1() {
    let result =
        cross_check_source_spec_version(SpecVersion::V0, SpecVersion::V1, "mtoon_basic_v0_lit_001");
    assert!(
        result.is_err(),
        "V0 plan with V1 adapter report should fail"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("mtoon_basic_v0_lit_001"),
        "err msg should reference plan id; got: {err}"
    );
    assert!(
        err.contains("0.x"),
        "err msg should mention 0.x; got: {err}"
    );
    assert!(
        err.contains("1.0"),
        "err msg should mention 1.0; got: {err}"
    );
}

#[test]
fn err_when_plan_v1_but_adapter_reports_v0() {
    let result =
        cross_check_source_spec_version(SpecVersion::V1, SpecVersion::V0, "mtoon_basic_lit");
    assert!(
        result.is_err(),
        "V1 plan with V0 adapter report should fail"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("mtoon_basic_lit"),
        "err msg should reference plan id; got: {err}"
    );
    assert!(
        err.contains("1.0"),
        "err msg should mention 1.0; got: {err}"
    );
    assert!(
        err.contains("0.x"),
        "err msg should mention 0.x; got: {err}"
    );
}
