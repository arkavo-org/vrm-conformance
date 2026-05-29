//! Sidecar emission: `.meta.json` and `.test.yaml`, both derived from the
//! same `MToonParams` that produced the `.vrm`.

use crate::params::MToonParams;
use crate::spring_bone::SpringBoneSceneParams;
use anyhow::Result;
use camino::Utf8Path;
use serde_json::json;
use vrm_test_plan::{
    AmbientLight, AnimationConfig, BboxRegion, Camera, ColorSpace, ConformanceStatus,
    CrossVariantAssertion, Diff, DiffMode, DirectionalLight, Lighting, Output, PhysicsConfig,
    PostProcessing, PropertyAssertion, RenderSequenceBlock, RootTransformAnimation, SequenceFormat,
    SequenceRootTransformAnimation, TestPlan, ToneMapping, VrmaAnimation,
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
        spec_version: vrm_test_plan::SpecVersion::V1,
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
            // Per vrm-conformance#2: 0.985 was scoped for self-diff stability;
            // cross-renderer agreement against the consortium reference
            // (UniVRM) sits in the 0.85-0.95 band by construction (engine-level
            // MSAA + color-pipeline residual). 0.85 is the operational
            // conformance threshold. Tighter clusters (e.g., rimLighting at
            // ~0.97) are scope for a future tightening pass, not 1.0.
            threshold: conformance_threshold_for(&params.id),
            reference_renderer: "univrm".into(),
            conformance_status: conformance_status_for(&params.id),
            pose_tolerance: None,
        },
        ignore_renderers: Vec::new(),
        properties: default_properties(params),
        physics: None,
        animation: None,
        render_sequence: None,
        cross_variant: None,
    }
}

/// Build the test plan for one doubleSided back-face-culling spec-test variant.
///
/// Camera sits on the −Z side looking toward +Z, so it views the quad's BACK
/// face (the quad's front normal is +Z). Camera-behind is deliberate over
/// rotating the quad 180° — it avoids confounding with VMK's documented
/// 180°-flip bug (VMK#299). When `cross_variant_sibling` is set (the `false`
/// variant), attaches a CrossVariantAssertion requiring the two renders to
/// diverge at or below SSIM 0.85.
pub fn build_doublesided_quad_test_plan(
    params: &MToonParams,
    asset_relpath: &str,
    cross_variant_sibling: Option<&str>,
) -> TestPlan {
    let mut plan = build_default_test_plan(params, asset_relpath);
    plan.spec_section = "glTF material.doubleSided (back-face culling)".into();
    plan.camera = Camera {
        position: [0.0, 1.36, -1.5],
        target: [0.0, 1.36, 0.0],
        up: [0.0, 1.0, 0.0],
        fov_degrees: 30.0,
    };
    if let Some(sibling) = cross_variant_sibling {
        plan.cross_variant = Some(CrossVariantAssertion {
            sibling_id: sibling.to_string(),
            max_ssim: 0.85,
        });
    }
    plan
}

/// Per-cluster conformance threshold. See `docs/methodology.md` and
/// `vrm-conformance#2`. Default 0.85 covers the engine-level residual band
/// (silhouette AA, color pipeline rounding) without conflating it with
/// MToon-math correctness. Tests that historically cluster tighter (rim
/// lighting) keep a tighter threshold so future drift is caught.
fn conformance_threshold_for(test_id: &str) -> f32 {
    if test_id.starts_with("mtoon_rimLightingMix") {
        0.95
    } else {
        0.85
    }
}

/// Per vrm-conformance#3: outline tests at width ≥ 0.05 m on a 30-cm sphere
/// produce a spec-correct flooded mesh. Whole-frame SSIM measures only
/// silhouette anti-aliasing on a frame with no other signal — three-vrm and
/// the consortium reference (UniVRM) agree at 0.9988 SSIM on these tests,
/// while VMK sits at 0.63 purely from AA. The metric is wrong for these
/// tests; render them (regression catch) but don't count them in the
/// conformance pass-rate until a ring-band SSIM metric exists.
fn conformance_status_for(test_id: &str) -> ConformanceStatus {
    // Excluded: outline width 0.05 and 0.1, both world and screen modes.
    if test_id == "mtoon_outline_world_0p05"
        || test_id == "mtoon_outline_world_0p1"
        || test_id == "mtoon_outline_screen_0p05"
        || test_id == "mtoon_outline_screen_0p1"
    {
        return ConformanceStatus::Excluded {
            reason:
                "outline width ≥ 0.05 m on a 30-cm sphere produces spec-correct flood; whole-frame \
                 SSIM measures silhouette AA only (see docs/methodology.md and \
                 vrm-conformance#3 for the four-renderer agreement matrix)"
                    .into(),
        };
    }
    ConformanceStatus::Included
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
        vrma: None,
    });
    plan.spec_section = "VRMC_materials_mtoon + VRMC_springBone (swing)".into();
    plan
}

/// Sequence variant of the swing plan. Emits a `render_sequence:` block
/// instead of `animation: { root_transform }` so the runner dispatches
/// `render_sequence` (multi-frame capture) rather than the single-frame
/// `render` path.
///
/// Capture rate: 60 frames @ 30 Hz with `physics_dt_seconds = 1/60`.
/// Animation: `[0,0,0] → [0.15, 0, 0]` linearly across all 60 frames.
/// Total animation duration: 2.0 s.
///
/// This is slower than the single-frame swing plan's 0.25 s, trading
/// reduced physics excitation for a much richer temporal signal — 60
/// frames of captured trajectory vs 1 single end-frame.
///
/// Starts from the SETTLE plan (not the swing plan): the swing plan
/// carries an `animation:` block which is mutually exclusive with
/// `render_sequence:` per TestPlan::validate.
pub fn build_spring_bone_swing_sequence_test_plan(
    params: &MToonParams,
    asset_relpath: &str,
) -> TestPlan {
    let mut plan = build_spring_bone_test_plan(params, asset_relpath);
    plan.render_sequence = Some(RenderSequenceBlock {
        frame_count: 60,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        output_format: SequenceFormat::PngSequence,
        animate_root_transform: Some(SequenceRootTransformAnimation {
            translation_start: [0.0, 0.0, 0.0],
            translation_end: [0.15, 0.0, 0.0],
        }),
        apply_vrma: None,
        temporal_ssim_threshold: None,
    });
    plan.spec_section = "VRMC_materials_mtoon + VRMC_springBone (sequence)".into();
    plan
}

/// Settle variant for collider tests: same framing as `build_spring_bone_test_plan`
/// but uses `settle_steps: 60` — collider tests converge slower under contact.
///
/// The `_scene` parameter is accepted for API consistency (future multi-chain
/// collider variants may need to customize the plan based on scene shape);
/// currently unused.
pub fn build_spring_bone_collider_test_plan(
    params: &MToonParams,
    _scene: &SpringBoneSceneParams,
    asset_relpath: &str,
) -> TestPlan {
    let mut plan = build_default_test_plan(params, asset_relpath);
    plan.physics = Some(PhysicsConfig { settle_steps: 60 });
    plan.spec_section = "VRMC_springBone + colliders".into();
    plan
}

/// Swing variant for collider tests. Same animation block as the existing
/// `build_spring_bone_swing_test_plan` but with 60-step settle (matching the
/// settle variant) and the collider spec_section label.
pub fn build_spring_bone_collider_swing_test_plan(
    params: &MToonParams,
    _scene: &SpringBoneSceneParams,
    asset_relpath: &str,
) -> TestPlan {
    let mut plan = build_spring_bone_collider_test_plan(params, _scene, asset_relpath);
    plan.animation = Some(AnimationConfig {
        root_transform: Some(RootTransformAnimation {
            translation_start: [0.0, 0.0, 0.0],
            translation_end: [0.15, 0.0, 0.0],
            duration_seconds: 0.25,
            fps: 60,
        }),
        vrma: None,
    });
    plan.spec_section = "VRMC_springBone + colliders (swing)".into();
    plan
}

/// Settle variant for extended collider tests.
/// Uses the same 60-step settle as `build_spring_bone_collider_test_plan`
/// but names both extensions in `spec_section` so the runner can display
/// the correct spec reference.
pub fn build_spring_bone_extended_test_plan(
    params: &MToonParams,
    _scene: &SpringBoneSceneParams,
    asset_relpath: &str,
) -> TestPlan {
    let mut plan = build_default_test_plan(params, asset_relpath);
    plan.physics = Some(PhysicsConfig { settle_steps: 60 });
    plan.spec_section = "VRMC_springBone + VRMC_springBone_extended_collider".into();
    plan
}

/// Swing variant for extended collider tests. Same animation block as
/// `build_spring_bone_collider_swing_test_plan` with 60-step settle and the
/// extended collider spec_section label.
pub fn build_spring_bone_extended_swing_test_plan(
    params: &MToonParams,
    scene: &SpringBoneSceneParams,
    asset_relpath: &str,
) -> TestPlan {
    let mut plan = build_spring_bone_extended_test_plan(params, scene, asset_relpath);
    plan.animation = Some(AnimationConfig {
        root_transform: Some(RootTransformAnimation {
            translation_start: [0.0, 0.0, 0.0],
            translation_end: [0.15, 0.0, 0.0],
            duration_seconds: 0.25,
            fps: 60,
        }),
        vrma: None,
    });
    plan.spec_section = "VRMC_springBone + VRMC_springBone_extended_collider (swing)".into();
    plan
}

/// Settle variant for multi-chain tests. Uses 60-step settle (same as collider
/// tests — multi-chain scenes may include colliders). Spec section names both
/// spring-bone and the multi-chain axis.
pub fn build_spring_bone_multichain_test_plan(
    params: &MToonParams,
    _scene: &SpringBoneSceneParams,
    asset_relpath: &str,
) -> vrm_test_plan::TestPlan {
    let mut plan = build_default_test_plan(params, asset_relpath);
    plan.physics = Some(vrm_test_plan::PhysicsConfig { settle_steps: 60 });
    plan.spec_section = "VRMC_springBone multi-chain".into();
    plan
}

/// Swing variant for multi-chain tests. Adds animate_root_transform so chains
/// are captured mid-swing rather than at gravity equilibrium.
pub fn build_spring_bone_multichain_swing_test_plan(
    params: &MToonParams,
    scene: &SpringBoneSceneParams,
    asset_relpath: &str,
) -> vrm_test_plan::TestPlan {
    let mut plan = build_spring_bone_multichain_test_plan(params, scene, asset_relpath);
    plan.animation = Some(vrm_test_plan::AnimationConfig {
        root_transform: Some(vrm_test_plan::RootTransformAnimation {
            translation_start: [0.0, 0.0, 0.0],
            translation_end: [0.15, 0.0, 0.0],
            duration_seconds: 0.25,
            fps: 60,
        }),
        vrma: None,
    });
    plan.spec_section = "VRMC_springBone multi-chain (swing)".into();
    plan
}

/// Build a test plan for a VRMA humanoid sweep triplet.
///
/// Starts from the default camera/lighting/output settings (same as all
/// MToon conformance tests) and overlays:
/// - `spec_section`: names VRMC_vrm_animation and the specific bone axis
/// - `animation.vrma`: path to the `.vrma` file + `apply_at_time`
/// - `diff.pose_tolerance`: tight per-bone quaternion tolerance
pub fn build_vrma_humanoid_test_plan(
    params: &crate::vrma_params::VrmaHumanoidParams,
    vrm_relpath: &str,
    vrma_relpath: &str,
) -> TestPlan {
    let mtoon_defaults = crate::params::MToonParams::defaults(&params.id);
    let mut plan = build_default_test_plan(&mtoon_defaults, vrm_relpath);
    plan.spec_section = format!(
        "VRMC_vrm_animation (humanoid sweep: {} {:+.0}\u{00b0})",
        params.bone_name, params.angle_deg
    );
    plan.animation = Some(AnimationConfig {
        root_transform: None,
        vrma: Some(VrmaAnimation {
            path: vrma_relpath.into(),
            apply_at_time: params.duration_s,
        }),
    });
    plan.diff.pose_tolerance = Some(vrm_test_plan::PoseTolerance {
        per_bone_quaternion_radians: 0.010,
        hips_translation_m: 0.005,
        per_preset_expression: 0.005,
        per_custom_expression: 0.005,
        look_at_yaw_pitch_degrees: 1.0,
        offset_from_head_bone_m: 0.001,
    });
    plan
}

/// Build a test plan for a VRMA expression sweep triplet.
///
/// Starts from the default camera/lighting/output settings and overlays:
/// - `spec_section`: names VRMC_vrm_animation and the expression kind + name
/// - `animation.vrma`: path to the `.vrma` file + `apply_at_time` at peak weight (t=duration/2)
/// - `diff.pose_tolerance`: tight per-expression tolerance
pub fn build_vrma_expression_test_plan(
    params: &crate::vrma_params::VrmaExpressionParams,
    vrm_relpath: &str,
    vrma_relpath: &str,
) -> TestPlan {
    let mtoon_defaults = crate::params::MToonParams::defaults(&params.id);
    let mut plan = build_default_test_plan(&mtoon_defaults, vrm_relpath);
    let kind_label = if params.is_preset { "preset" } else { "custom" };
    plan.spec_section = format!(
        "VRMC_vrm_animation (expression sweep: {} {})",
        kind_label, params.expression_name
    );
    plan.animation = Some(AnimationConfig {
        root_transform: None,
        vrma: Some(VrmaAnimation {
            path: vrma_relpath.into(),
            apply_at_time: params.duration_s / 2.0, // peak weight
        }),
    });
    plan.diff.pose_tolerance = Some(vrm_test_plan::PoseTolerance {
        per_bone_quaternion_radians: 0.010,
        hips_translation_m: 0.005,
        per_preset_expression: 0.005,
        per_custom_expression: 0.005,
        look_at_yaw_pitch_degrees: 1.0,
        offset_from_head_bone_m: 0.001,
    });
    plan
}

/// Build a test plan for a VRMA lookAt sweep triplet.
///
/// Starts from the default camera/lighting/output settings and overlays:
/// - `spec_section`: names VRMC_vrm_animation and the gaze direction + avatar config
/// - `animation.vrma`: path to the `.vrma` file + `apply_at_time`
/// - `diff.pose_tolerance`: tight lookAt yaw/pitch tolerance
pub fn build_vrma_lookat_test_plan(
    params: &crate::vrma_params::VrmaLookAtParams,
    vrm_relpath: &str,
    vrma_relpath: &str,
) -> TestPlan {
    let mtoon_defaults = crate::params::MToonParams::defaults(&params.id);
    let mut plan = build_default_test_plan(&mtoon_defaults, vrm_relpath);
    plan.spec_section = format!(
        "VRMC_vrm_animation (lookAt sweep: {:?} {:+.0}\u{00b0} vs avatar.{:?})",
        params.axis, params.angle_deg, params.avatar_lookat_type
    );
    plan.animation = Some(AnimationConfig {
        root_transform: None,
        vrma: Some(VrmaAnimation {
            path: vrma_relpath.into(),
            apply_at_time: params.duration_s,
        }),
    });
    plan.diff.pose_tolerance = Some(vrm_test_plan::PoseTolerance {
        per_bone_quaternion_radians: 0.010,
        hips_translation_m: 0.005,
        per_preset_expression: 0.005,
        per_custom_expression: 0.005,
        look_at_yaw_pitch_degrees: 1.0,
        offset_from_head_bone_m: 0.001,
    });
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

#[cfg(test)]
mod doublesided_plan_tests {
    use super::*;

    #[test]
    fn doublesided_quad_plan_has_back_camera_and_cross_variant_on_false_only() {
        let p = MToonParams::defaults("doublesided_quad_false");
        let plan = build_doublesided_quad_test_plan(
            &p,
            "doublesided_quad_false.vrm",
            Some("doublesided_quad_true"),
        );
        // Camera sits behind the quad (−Z side) so the back face is in frame.
        assert!(
            plan.camera.position[2] < 0.0,
            "camera must be on the -Z side, got {:?}",
            plan.camera.position
        );
        let cv = plan
            .cross_variant
            .as_ref()
            .expect("false variant carries cross_variant");
        assert_eq!(cv.sibling_id, "doublesided_quad_true");
        assert!((cv.max_ssim - 0.85).abs() < 1e-6);
        assert!(plan.validate().is_ok());

        // True variant carries no cross_variant (single, non-redundant declaration).
        let p_true = MToonParams::defaults("doublesided_quad_true");
        let plan_true =
            build_doublesided_quad_test_plan(&p_true, "doublesided_quad_true.vrm", None);
        assert!(plan_true.cross_variant.is_none());
    }
}

#[cfg(test)]
mod extended_plan_tests {
    use super::*;
    use crate::spring_bone::*;

    #[test]
    fn extended_plan_settle_has_60_settle_steps() {
        let mtoon = MToonParams::defaults("test_ext");
        let scene = SpringBoneSceneParams::single_spring(SpringBoneParams::defaults("t"));
        let plan = build_spring_bone_extended_test_plan(&mtoon, &scene, "out.vrm");
        assert_eq!(plan.physics.unwrap().settle_steps, 60);
        assert!(plan.animation.is_none());
    }

    #[test]
    fn extended_plan_spec_section_names_both_extensions() {
        let mtoon = MToonParams::defaults("test_ext");
        let scene = SpringBoneSceneParams::single_spring(SpringBoneParams::defaults("t"));
        let plan = build_spring_bone_extended_test_plan(&mtoon, &scene, "out.vrm");
        assert!(
            plan.spec_section
                .contains("VRMC_springBone_extended_collider"),
            "spec_section should name the extension: {}",
            plan.spec_section
        );
    }
}

#[cfg(test)]
mod render_sequence_tests {
    use super::*;
    use vrm_test_plan::SequenceFormat;

    #[test]
    fn swing_sequence_plan_uses_render_sequence_not_animation() {
        let params = MToonParams::defaults("swing_seq_test");
        let plan = build_spring_bone_swing_sequence_test_plan(&params, "swing_seq_test.vrm");

        // Sequence plan must NOT carry animation (validator would reject).
        assert!(
            plan.animation.is_none(),
            "sequence plan must omit animation"
        );

        // Must carry render_sequence.
        let seq = plan
            .render_sequence
            .as_ref()
            .expect("render_sequence required");
        assert_eq!(seq.frame_count, 60);
        assert!((seq.frame_hz - 30.0).abs() < 1e-6);
        assert!((seq.physics_dt_seconds - 1.0 / 60.0).abs() < 1e-9);
        assert!(matches!(seq.output_format, SequenceFormat::PngSequence));
        assert!(seq.apply_vrma.is_none());
        assert!(seq.temporal_ssim_threshold.is_none());

        let anim = seq
            .animate_root_transform
            .as_ref()
            .expect("translation required");
        assert_eq!(anim.translation_start, [0.0, 0.0, 0.0]);
        assert_eq!(anim.translation_end, [0.15, 0.0, 0.0]);

        // Physics settle preserved from the settle plan.
        assert_eq!(plan.physics.as_ref().unwrap().settle_steps, 30);

        // Validator accepts (no animation + render_sequence collision).
        assert!(plan.validate().is_ok(), "plan should validate");
    }
}

#[cfg(test)]
mod collider_plan_tests {
    use super::*;
    use crate::spring_bone::*;

    #[test]
    fn collider_plan_settle_has_60_settle_steps() {
        let mtoon = MToonParams::defaults("test_collider_settle");
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("test")],
            colliders: vec![],
            collider_groups: vec![],
            spring_collider_groups: vec![vec![]],
        };
        let plan = build_spring_bone_collider_test_plan(&mtoon, &scene, "out.vrm");
        let physics = plan.physics.expect("plan must carry physics config");
        assert_eq!(
            physics.settle_steps, 60,
            "collider tests use 60-step settle (slower convergence under contact)"
        );
        assert!(
            plan.animation.is_none(),
            "settle plan has no animation block"
        );
    }

    #[test]
    fn collider_plan_swing_carries_animation_block() {
        let mtoon = MToonParams::defaults("test_collider_swing");
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("test")],
            colliders: vec![],
            collider_groups: vec![],
            spring_collider_groups: vec![vec![]],
        };
        let plan = build_spring_bone_collider_swing_test_plan(&mtoon, &scene, "out.vrm");
        assert!(plan.animation.is_some());
        let anim = plan.animation.unwrap();
        let root = anim.root_transform.unwrap();
        assert!((root.duration_seconds - 0.25).abs() < 1e-6);
        assert_eq!(root.fps, 60);
    }
}
