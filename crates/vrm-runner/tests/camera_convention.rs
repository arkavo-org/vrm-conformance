//! Slice 1 success criterion: runner rejects test plans whose camera
//! Z direction disagrees with the spec_version's avatar-facing convention.
//! VRM 0.x: avatar faces -Z, camera must be at negative Z.
//! VRM 1.0: avatar faces +Z, camera must be at positive Z.

use vrm_runner::execute::validate_camera_convention;
use vrm_test_plan::{
    AmbientLight, Camera, ColorSpace, Diff, DiffMode, DirectionalLight, Lighting, Output,
    PostProcessing, SpecVersion, TestPlan, ToneMapping,
};

fn plan_with(spec_version: SpecVersion, camera_z: f32) -> TestPlan {
    TestPlan {
        id: "t".into(),
        spec_version,
        spec_section: "test".into(),
        asset: "a.vrm".into(),
        camera: Camera {
            position: [0.0, 1.3, camera_z],
            target: [0.0, 1.3, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_degrees: 30.0,
        },
        lighting: Lighting {
            directional: DirectionalLight {
                dir: [0.0, -1.0, 0.0],
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
            },
            ambient: AmbientLight {
                color: [0.5, 0.5, 0.5],
                intensity: 0.3,
            },
            cast_shadows: false,
            receive_shadows: false,
        },
        post_processing: PostProcessing {
            tone_mapping: ToneMapping::None,
            exposure: 1.0,
        },
        output: Output {
            width: 256,
            height: 256,
            color_space: ColorSpace::Srgb,
            msaa: 1,
        },
        diff: Diff {
            mode: DiffMode::Ssim,
            threshold: 0.95,
            reference_renderer: "three-vrm".into(),
            conformance_status: Default::default(),
            pose_tolerance: None,
        },
        ignore_renderers: vec![],
        properties: vec![],
        physics: None,
        augment_colliders: None,
        animation: None,
        render_sequence: None,
        cross_variant: None,
        ccd_colliders: None,
    }
}

#[test]
fn rejects_v0_plan_with_positive_z_camera() {
    let plan = plan_with(SpecVersion::V0, 1.5);
    let result = validate_camera_convention(&plan);
    assert!(
        result.is_err(),
        "expected error for V0 plan with positive-Z camera, got Ok"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("0.x"),
        "error should mention spec version 0.x; got: {err}"
    );
    assert!(
        err.contains("-Z"),
        "error should mention -Z convention; got: {err}"
    );
}

#[test]
fn accepts_v0_plan_with_negative_z_camera() {
    let plan = plan_with(SpecVersion::V0, -1.5);
    assert!(
        validate_camera_convention(&plan).is_ok(),
        "V0 plan with negative-Z camera should be accepted"
    );
}

#[test]
fn rejects_v1_plan_with_negative_z_camera() {
    let plan = plan_with(SpecVersion::V1, -1.5);
    let result = validate_camera_convention(&plan);
    assert!(
        result.is_err(),
        "expected error for V1 plan with negative-Z camera, got Ok"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("1.0"),
        "error should mention spec version 1.0; got: {err}"
    );
    assert!(
        err.contains("+Z"),
        "error should mention +Z convention; got: {err}"
    );
}

#[test]
fn accepts_v1_plan_with_positive_z_camera() {
    let plan = plan_with(SpecVersion::V1, 1.5);
    assert!(
        validate_camera_convention(&plan).is_ok(),
        "V1 plan with positive-Z camera should be accepted"
    );
}
