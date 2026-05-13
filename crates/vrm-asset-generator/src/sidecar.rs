//! Sidecar emission: `.meta.json` and `.test.yaml`, both derived from the
//! same `MToonParams` that produced the `.vrm`.

use crate::params::MToonParams;
use anyhow::Result;
use camino::Utf8Path;
use serde_json::json;
use vrm_test_plan::{
    AmbientLight, AnimationConfig, BboxRegion, Camera, ColorSpace, Diff, DiffMode,
    DirectionalLight, Lighting, Output, PhysicsConfig, PostProcessing, PropertyAssertion,
    RootTransformAnimation, TestPlan, ToneMapping,
};

pub fn write_meta_json(
    params: &MToonParams,
    spring_bone: Option<&crate::spring_bone::SpringBoneParams>,
    vrm_path: &Utf8Path,
    out: &Utf8Path,
) -> Result<()> {
    let bytes = std::fs::read(vrm_path)?;
    let hash = blake3::hash(&bytes);
    let mut meta = json!({
        "id": params.id,
        "license": "CC0-1.0",
        "generator": format!("arkavo-org/vrm-conformance vrm-asset-generator {}", env!("CARGO_PKG_VERSION")),
        "spec_section": "VRMC_materials_mtoon",
        "blake3": format!("blake3:{}", hash.to_hex()),
        "byte_size": bytes.len(),
        "params": params,
    });
    if let Some(sb) = spring_bone {
        meta["spring_bone"] = serde_json::to_value(sb)?;
        meta["spec_section"] =
            serde_json::Value::String("VRMC_materials_mtoon + VRMC_springBone".into());
    }
    std::fs::write(out, serde_json::to_vec_pretty(&meta)?)?;
    Ok(())
}

pub fn build_default_test_plan(params: &MToonParams, asset_relpath: &str) -> TestPlan {
    TestPlan {
        id: params.id.clone(),
        spec_section: "VRMC_materials_mtoon".into(),
        asset: asset_relpath.into(),
        camera: Camera {
            // Camera framed on the head-mounted sphere (head ≈ y=1.36, sphere radius 0.3).
            position: [0.0, 1.4, 1.5],
            target: [0.0, 1.4, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_degrees: 30.0,
        },
        lighting: Lighting {
            directional: DirectionalLight {
                dir: [-0.3, -0.6, -0.7],
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
            },
            ambient: AmbientLight {
                color: [0.5, 0.5, 0.5],
                intensity: 0.3,
            },
            cast_shadows: false, // see docs/methodology.md
            receive_shadows: false,
        },
        post_processing: PostProcessing {
            tone_mapping: ToneMapping::None, // pinned for MToon math
            exposure: 1.0,
        },
        output: Output {
            width: 1024,
            height: 1024,
            color_space: ColorSpace::Srgb,
            msaa: 4,
        },
        diff: Diff {
            mode: DiffMode::Ssim,
            threshold: 0.985,
            reference_renderer: "vrm-metal-kit".into(),
        },
        ignore_renderers: Vec::new(),
        properties: default_properties(params),
        physics: None,
        animation: None,
    }
}

/// Same as `build_default_test_plan` but with `physics: { settle_steps: 30 }`
/// — used by the spring-bone emit path so the runner knows to settle the
/// chain before rendering.
pub fn build_spring_bone_test_plan(params: &MToonParams, asset_relpath: &str) -> TestPlan {
    let mut plan = build_default_test_plan(params, asset_relpath);
    plan.physics = Some(PhysicsConfig { settle_steps: 30 });
    plan.spec_section = "VRMC_materials_mtoon + VRMC_springBone".into();
    plan
}

/// Settle + swing variant: physics resets to rest, then animate_root_transform
/// translates the avatar 15 cm sideways over 0.25 s @ 60 Hz before the
/// render fires. Captures the chain mid-swing so renderer differences in
/// inertia/drag handling surface, rather than just gravity equilibrium.
///
/// Numbers chosen so the sweep is visually meaningful but bounded:
/// - 0.15 m matches "a brisk head-turn" of a 1.7 m avatar
/// - 0.25 s is ≈ 1/4 second — long enough that drag matters, short enough
///   that high-stiffness chains haven't fully tracked the motion yet
/// - 60 Hz is the spring-bone determinism convention (`docs/methodology.md`)
pub fn build_spring_bone_swing_test_plan(params: &MToonParams, asset_relpath: &str) -> TestPlan {
    let mut plan = build_spring_bone_test_plan(params, asset_relpath);
    plan.animation = Some(AnimationConfig {
        root_transform: Some(RootTransformAnimation {
            translation_start: [0.0, 0.0, 0.0],
            translation_end: [0.15, 0.0, 0.0],
            duration_seconds: 0.25,
            fps: 60,
        }),
    });
    plan.spec_section = "VRMC_materials_mtoon + VRMC_springBone (swing)".into();
    plan
}

fn default_properties(_params: &MToonParams) -> Vec<PropertyAssertion> {
    // v0.1 default: one general-purpose lower-quad average-luminance check.
    // Test-specific assertions get added per parameter combination later.
    vec![PropertyAssertion {
        name: "avg_luminance_lower_left_quad".into(),
        region: BboxRegion::BboxLowerLeftQuadrant,
        expected: 0.4,
        tolerance: 0.3,
    }]
}

pub fn write_test_yaml(plan: &TestPlan, out: &Utf8Path) -> Result<()> {
    let yaml = serde_yml::to_string(plan)?;
    std::fs::write(out, yaml)?;
    Ok(())
}
