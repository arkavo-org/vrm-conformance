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
        ccd_colliders: None,
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
    // Excluded: matcap + shadeMultiplyTexture on the procedural sphere diverge
    // cross-renderer purely from the sphere's UV projection, NOT from renderer
    // texture-handling — the flat-quad checkerboard control proves VMK (and
    // three-vrm) sample texture-U correctly (TL=red/TR=green/BL=blue/BR=yellow);
    // on the sphere VMK+three-vrm agree and UniVRM is the outlier. matcap is
    // additionally noisy across all renderers (near-black, low-contrast SSIM).
    // Texture-orientation conformance uses the `emit-textured-quad` control, not
    // the sphere. See docs/findings.md (2026-06-07 shadetex SETTLED via flat-quad).
    if test_id.starts_with("mtoon_matcap_") || test_id.starts_with("mtoon_shadetex_") {
        return ConformanceStatus::Excluded {
            reason:
                "matcap/shadeMultiplyTexture on the procedural sphere is a UV-projection artifact, \
                 not a renderer texture-handling defect (proven spec-correct on the flat-quad \
                 checkerboard control); texture-orientation conformance uses the quad control"
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
        capture_positions: false,
        capture_synthetic_colliders: false,
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

/// Build a test plan for a VRMA hips-translation sweep triplet.
///
/// Starts from the default camera/lighting/output settings and overlays:
/// - `spec_section`: names VRMC_vrm_animation and the hips offset
/// - `animation.vrma`: path + `apply_at_time` at full offset (t = duration_s)
/// - `diff.pose_tolerance`: tight hips-translation tolerance
pub fn build_vrma_hips_translation_test_plan(
    params: &crate::vrma_params::VrmaHipsTranslationParams,
    vrm_relpath: &str,
    vrma_relpath: &str,
) -> TestPlan {
    let mtoon_defaults = crate::params::MToonParams::defaults(&params.id);
    let mut plan = build_default_test_plan(&mtoon_defaults, vrm_relpath);
    plan.spec_section = format!(
        "VRMC_vrm_animation (hips translation sweep: [{:+.2}, {:+.2}, {:+.2}] m)",
        params.offset_m[0], params.offset_m[1], params.offset_m[2]
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

/// Plan for the synthetic-collider validation corpus. Drives `render_sequence`
/// with both `capture_positions` and `capture_synthetic_colliders` true, a
/// fast (swept) or slow (static-ish) root translation to excite the hair
/// chain, and NO authored `ccd_colliders` — the synthetic colliders are dumped
/// at runtime by the adapter. Rendered twice (augment on/off) by the runner.
pub fn build_synthetic_collider_test_plan(
    params: &MToonParams,
    asset_relpath: &str,
    fast: bool,
) -> TestPlan {
    // Both speeds sweep the same ±0.30 m displacement; only velocity differs.
    // Fast: 12 frames @ 60 Hz → 0.20 s, Δx/frame ≈ 0.050 m (the lagging chain
    // is overtaken by the moving synthetic collider — the #313 swept case).
    // Slow: 120 frames @ 60 Hz → 2.0 s, Δx/frame ≈ 0.005 m (near-static contact
    // — the #309 resting-deflection case). The ±0.30 (vs the CCD sweep's ±0.50)
    // keeps the head-attached colliders within reach of the hair chain.
    let (frame_count, frame_hz) = if fast {
        (12u32, 60.0_f32)
    } else {
        (120u32, 60.0_f32)
    };
    let translation_start = [-0.30_f32, 0.0, 0.0];
    let translation_end = [0.30_f32, 0.0, 0.0];

    let mut plan = build_default_test_plan(params, asset_relpath);
    plan.spec_section = "VMK synthetic collider augmentation (#309/#313) — augment on/off".into();
    plan.physics = Some(PhysicsConfig { settle_steps: 60 });
    plan.post_processing = PostProcessing {
        tone_mapping: ToneMapping::None,
        exposure: 1.0,
    };
    plan.animation = None;
    plan.render_sequence = Some(RenderSequenceBlock {
        frame_count,
        frame_hz,
        physics_dt_seconds: 1.0 / 60.0,
        output_format: SequenceFormat::PngSequence,
        animate_root_transform: Some(SequenceRootTransformAnimation {
            translation_start,
            translation_end,
        }),
        apply_vrma: None,
        temporal_ssim_threshold: None,
        capture_positions: true,
        capture_synthetic_colliders: true,
    });
    plan.ccd_colliders = None;
    plan
}

pub fn write_test_yaml(plan: &TestPlan, out: &Utf8Path) -> Result<()> {
    let yaml = serde_yml::to_string(plan)?;
    std::fs::write(out, yaml)?;
    Ok(())
}

/// Tag a test plan as VRM 0.x: set the spec version AND move the camera to the
/// −Z side. VRM 0.x avatars face −Z (spec 0.0 README:238), so the camera must be
/// at negative Z to see the front; the runner's validate_camera_convention
/// rejects 0.x plans with a +Z camera. All v0 plans derive their camera from
/// build_default_test_plan (+Z), so this flip is required for every 0.x plan.
pub fn tag_plan_vrm0(plan: &mut TestPlan) {
    plan.spec_version = vrm_test_plan::SpecVersion::V0;
    // Force the camera onto the −Z side (target stays at the avatar; up unchanged).
    plan.camera.position[2] = -plan.camera.position[2].abs();
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
mod tag_plan_vrm0_tests {
    use super::*;

    #[test]
    fn tag_plan_vrm0_moves_camera_to_negative_z() {
        let p = MToonParams::defaults("cam_v0");
        let mut plan = build_default_test_plan(&p, "cam_v0.vrm");
        assert!(plan.camera.position[2] > 0.0, "default plan starts +Z");
        tag_plan_vrm0(&mut plan);
        assert_eq!(plan.spec_version, vrm_test_plan::SpecVersion::V0);
        assert!(
            plan.camera.position[2] < 0.0,
            "0.x plan camera must be on the -Z side"
        );
    }
}

/// Build a test plan for a CCD sweep cell.
///
/// # Design: fast vs slow root sweep
///
/// The goal is to straddle the tunneling threshold in a single corpus.
/// The chain's rest column sits at x=0; the world-fixed collider center is at
/// x=0.10. When the avatar root is swept +X, the chain crosses the collider.
/// Whether a non-CCD integrator tunnels depends on the per-substep displacement
/// relative to the collider radius:
///
/// ```text
/// tunneling if: Δx_per_substep > 2 * collider_radius
/// ```
///
/// With `physics_dt = 1/60 s` (the methodology floor):
///
/// | Speed | translation_start | translation_end | total_dx | frame_count | frame_hz | total_duration | Δx_per_substep |
/// |-------|-------------------|-----------------|----------|-------------|----------|----------------|----------------|
/// | fast  | [-0.50, 0, 0]     | [0.50, 0, 0]    | 1.0 m    |  12         |  60.0    |  0.20 s        |  ≈ 0.083 m     |
/// | slow  | [-0.50, 0, 0]     | [0.50, 0, 0]    | 1.0 m    | 120         |  60.0    |  2.0  s        |  ≈ 0.0083 m    |
///
/// Fast: Δx ≈ 1.0 / (12 × 1) = 0.083 m per frame × 1 substep = 0.083 m.
/// This is > 2 × 0.02 m (marginal radius) and >> 2 × 0.005 m (small radius),
/// so a non-CCD integrator WILL tunnel through the small-radius colliders.
///
/// Slow: Δx ≈ 1.0 / (120 × 1) = 0.0083 m per frame × 1 substep = 0.0083 m.
/// This is well below 2 × 0.005 m = 0.010 m, so even the smallest collider
/// is detected without CCD. Every compliant integrator — CCD or not — must
/// resolve the collision.
///
/// # Collider mapping
///
/// `CcdColliderWorldSpec.kind` → `ColliderWorldSpec` variant:
/// - `"sphere"` → `ColliderWorldSpec::Sphere { center, radius }` directly.
/// - `"capsule"` → `ColliderWorldSpec::Capsule { a, b, radius }` where
///   `a = capsule_head` (VRM head endpoint = node_translation + offset) and
///   `b = capsule_tail` (VRM tail endpoint = node_translation + tail_offset).
///   For the CCD sweep with node_center=[0.10,1.26,0.0], offset=[0,0,0],
///   tail_offset=[0.0,-0.10,0.0]:  a=[0.10,1.26,0.0],  b=[0.10,1.16,0.0].
pub fn build_spring_bone_ccd_test_plan(
    params: &MToonParams,
    asset_relpath: &str,
    scene: &crate::spring_bone::SpringBoneSceneParams,
) -> TestPlan {
    use crate::sweep::ccd_collider_world_spec;

    // Parse speed from the asset id suffix.
    let is_fast = params.id.ends_with("_fast");

    // Fast: 12 frames at 60 Hz → 0.20 s total, Δx_per_frame ≈ 0.083 m.
    // Slow: 120 frames at 60 Hz → 2.0 s total, Δx_per_frame ≈ 0.0083 m.
    let (frame_count, frame_hz) = if is_fast {
        (12u32, 60.0_f32)
    } else {
        (120u32, 60.0_f32)
    };

    // Both variants sweep the same total displacement so position data is
    // comparable across speed tiers. Start at x=−0.50 so the chain
    // approaches the collider from the side opposite its rest column.
    let translation_start = [-0.50_f32, 0.0, 0.0];
    let translation_end = [0.50_f32, 0.0, 0.0];

    // Map CcdColliderWorldSpec → ColliderWorldSpec.
    let world_spec = ccd_collider_world_spec(scene);
    let ccd_collider = match world_spec.kind {
        "sphere" => vrm_test_plan::ColliderWorldSpec::Sphere {
            center: world_spec.center,
            radius: world_spec.radius,
        },
        "capsule" => {
            // Use the true VRM world endpoints from ccd_collider_world_spec:
            //   a = capsule_head = node_translation + offset  (head semicircle)
            //   b = capsule_tail = node_translation + tail_offset  (tail semicircle)
            // These are the values the renderer will compute from the .vrm file
            // per the VRM spec capsule pseudocode (transformedOffset / transformedTail).
            let a = world_spec
                .capsule_head
                .expect("capsule kind must have capsule_head");
            let b = world_spec
                .capsule_tail
                .expect("capsule kind must have capsule_tail");
            vrm_test_plan::ColliderWorldSpec::Capsule {
                a,
                b,
                radius: world_spec.radius,
            }
        }
        other => panic!("unexpected CCD collider kind: {other}"),
    };

    // Build a base plan (inherit camera, lighting, output, diff settings).
    let mut plan = build_default_test_plan(params, asset_relpath);
    plan.spec_section = "VRMC_springBone CCD / tunneling".into();
    // Physics: 60-step settle before the sequence starts (collider contact may
    // need more time to converge than the default 30-step settle).
    plan.physics = Some(PhysicsConfig { settle_steps: 60 });
    // Ensure tone_mapping is None (CCD metric depends on position data, not
    // pixel colour, but we keep it consistent with the methodology convention).
    plan.post_processing = PostProcessing {
        tone_mapping: ToneMapping::None,
        exposure: 1.0,
    };
    // No single-frame animation block (mutually exclusive with render_sequence).
    plan.animation = None;
    plan.render_sequence = Some(RenderSequenceBlock {
        frame_count,
        frame_hz,
        physics_dt_seconds: 1.0 / 60.0,
        output_format: SequenceFormat::PngSequence,
        animate_root_transform: Some(SequenceRootTransformAnimation {
            translation_start,
            translation_end,
        }),
        apply_vrma: None,
        temporal_ssim_threshold: None,
        capture_positions: true,
        capture_synthetic_colliders: false,
    });
    plan.ccd_colliders = Some(vec![ccd_collider]);
    plan
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

#[cfg(test)]
mod ccd_plan_tests {
    use super::*;
    use crate::sweep::spring_bone_ccd_sweep;

    /// Compute the per-substep lateral displacement for a render_sequence block.
    /// With physics_dt = 1/60 s and frame_hz = 60, each frame = 1 physics substep.
    /// velocity = total_dx / (frame_count * physics_dt_per_frame)
    /// where physics_dt_per_frame = physics_dt_seconds (one substep per frame at 60 Hz).
    fn ccd_substep_velocity(rs: &RenderSequenceBlock) -> f32 {
        let anim = rs
            .animate_root_transform
            .as_ref()
            .expect("animate_root_transform required");
        let dx = (anim.translation_end[0] - anim.translation_start[0]).abs();
        // duration_seconds = frame_count / frame_hz
        let duration = rs.frame_count as f32 / rs.frame_hz;
        // Δx per substep = Δx per frame (1 substep per frame at matching rates)
        dx / (duration / rs.physics_dt_seconds)
    }

    #[test]
    fn ccd_test_plan_has_fast_sweep_capture_and_world_collider() {
        let cells = spring_bone_ccd_sweep();
        // pick a fast sphere cell
        let (mtoon, scene) = cells
            .iter()
            .find(|(m, _)| m.id.contains("sphere") && m.id.ends_with("_fast"))
            .expect("a fast sphere cell");
        let plan = build_spring_bone_ccd_test_plan(mtoon, &format!("{}.vrm", mtoon.id), scene);

        // render_sequence present, capture_positions true, no single-frame animation block
        let rs = plan
            .render_sequence
            .as_ref()
            .expect("render_sequence block");
        assert!(rs.capture_positions, "capture_positions must be true");
        assert!(
            plan.animation.is_none(),
            "render_sequence is mutually exclusive with animation"
        );

        // fast root motion: a long lateral translation
        let anim = rs.animate_root_transform.as_ref().expect("root anim");
        let dx = anim.translation_end[0] - anim.translation_start[0];
        assert!(
            dx.abs() >= 0.5,
            "fast sweep travels far laterally, got {dx}"
        );

        // tone mapping none
        assert!(matches!(
            plan.post_processing.tone_mapping,
            vrm_test_plan::ToneMapping::None
        ));

        // ccd_colliders populated and matches the cell's world collider exactly
        let colliders = plan.ccd_colliders.as_ref().expect("ccd_colliders");
        assert_eq!(colliders.len(), 1);
        match &colliders[0] {
            vrm_test_plan::ColliderWorldSpec::Sphere { center, radius } => {
                assert_eq!(*center, [0.10, 1.26, 0.0], "collider center pinned");
                assert!(*radius > 0.0);
            }
            _ => panic!("expected sphere for a sphere cell"),
        }
        // plan validates (render_sequence + ccd_colliders allowed together)
        plan.validate().expect("plan validates");
    }

    #[test]
    fn ccd_test_plan_slow_sweep_is_slower_than_fast() {
        let cells = spring_bone_ccd_sweep();
        let fast = cells.iter().find(|(m, _)| m.id.ends_with("_fast")).unwrap();
        let slow = cells.iter().find(|(m, _)| m.id.ends_with("_slow")).unwrap();
        let pf = build_spring_bone_ccd_test_plan(&fast.0, "f.vrm", &fast.1);
        let ps = build_spring_bone_ccd_test_plan(&slow.0, "s.vrm", &slow.1);
        let df = pf.render_sequence.unwrap();
        let ds = ps.render_sequence.unwrap();
        // slow cell has lower per-substep displacement than fast
        assert!(
            ccd_substep_velocity(&ds) < ccd_substep_velocity(&df),
            "slow substep velocity ({}) must be less than fast ({})",
            ccd_substep_velocity(&ds),
            ccd_substep_velocity(&df),
        );
    }

    #[test]
    fn ccd_plan_capsule_cell_has_a_and_b_endpoints() {
        let cells = spring_bone_ccd_sweep();
        let (mtoon, scene) = cells
            .iter()
            .find(|(m, _)| m.id.contains("capsule") && m.id.ends_with("_fast"))
            .expect("a fast capsule cell");
        let plan = build_spring_bone_ccd_test_plan(mtoon, &format!("{}.vrm", mtoon.id), scene);
        let colliders = plan.ccd_colliders.expect("ccd_colliders");
        match &colliders[0] {
            vrm_test_plan::ColliderWorldSpec::Capsule { a, b, radius } => {
                // VRM capsule geometry: node_center=[0.10,1.26,0.0], offset=[0,0,0],
                // tail_offset=[0.0,-0.10,0.0].
                //   a = head = node_center + offset = [0.10, 1.26, 0.0]
                //   b = tail = node_center + tail_offset = [0.10, 1.16, 0.0]
                assert!(
                    (a[0] - 0.10_f32).abs() < 1e-5,
                    "a.x pinned to 0.10, got {}",
                    a[0]
                );
                assert!(
                    (a[1] - 1.26_f32).abs() < 1e-5,
                    "a.y pinned to 1.26 (head = node_translation), got {}",
                    a[1]
                );
                assert!((a[2]).abs() < 1e-5, "a.z pinned to 0.0, got {}", a[2]);
                assert!(
                    (b[0] - 0.10_f32).abs() < 1e-5,
                    "b.x pinned to 0.10, got {}",
                    b[0]
                );
                assert!(
                    (b[1] - 1.16_f32).abs() < 1e-5,
                    "b.y pinned to 1.16 (tail = node_translation + tail_offset), got {}",
                    b[1]
                );
                assert!((b[2]).abs() < 1e-5, "b.z pinned to 0.0, got {}", b[2]);
                assert_eq!(a[0], b[0], "a.x == b.x (same lateral position)");
                assert!(*radius > 0.0);
            }
            other => panic!("expected Capsule, got {other:?}"),
        }
    }

    #[test]
    fn ccd_fast_substep_displacement_exceeds_marginal_collider_radius() {
        // Validates that the fast sweep is in the tunneling regime for the
        // marginal collider radius (0.02 m). A non-CCD integrator should tunnel.
        // tunneling condition: Δx_per_substep > 2 * r_marginal
        let cells = spring_bone_ccd_sweep();
        let (mtoon, scene) = cells.iter().find(|(m, _)| m.id.ends_with("_fast")).unwrap();
        let plan = build_spring_bone_ccd_test_plan(mtoon, "x.vrm", scene);
        let rs = plan.render_sequence.unwrap();
        let v = ccd_substep_velocity(&rs);
        let r_marginal = 0.02_f32; // marginal radius from the sweep
        assert!(
            v > 2.0 * r_marginal,
            "fast substep velocity {v} m should exceed 2 × marginal radius {} m",
            2.0 * r_marginal
        );
    }

    #[test]
    fn ccd_slow_substep_displacement_is_below_small_collider_radius() {
        // Validates that the slow sweep is below the tunneling threshold even
        // for the smallest collider radius (0.005 m): Δx < 2 * r_small.
        let cells = spring_bone_ccd_sweep();
        let (mtoon, scene) = cells.iter().find(|(m, _)| m.id.ends_with("_slow")).unwrap();
        let plan = build_spring_bone_ccd_test_plan(mtoon, "x.vrm", scene);
        let rs = plan.render_sequence.unwrap();
        let v = ccd_substep_velocity(&rs);
        let r_small = 0.005_f32; // smallest radius from the sweep
        assert!(
            v <= 2.0 * r_small,
            "slow substep velocity {v} m should be ≤ 2 × small radius {} m",
            2.0 * r_small
        );
    }
}

#[cfg(test)]
mod synthetic_collider_plan_tests {
    use super::*;

    #[test]
    fn synthetic_collider_plan_captures_positions_and_colliders_no_ccd() {
        let mtoon = MToonParams::defaults("synthcoll_swept");
        let plan = build_synthetic_collider_test_plan(&mtoon, "synthcoll_swept.vrm", true);
        let rs = plan.render_sequence.as_ref().expect("render_sequence");
        assert!(rs.capture_positions);
        assert!(rs.capture_synthetic_colliders);
        assert!(
            rs.animate_root_transform.is_some(),
            "swept uses root translation"
        );
        assert!(
            plan.ccd_colliders.is_none(),
            "synthetic colliders are runtime-dumped, not authored"
        );
        assert!(plan.animation.is_none());
        assert_eq!(rs.frame_count, 12, "fast variant is 12 frames");
        plan.validate().expect("plan must validate");
    }

    #[test]
    fn synthetic_collider_plan_slow_variant_is_120_frames_and_validates() {
        let mtoon = MToonParams::defaults("synthcoll_static");
        let plan = build_synthetic_collider_test_plan(&mtoon, "synthcoll_static.vrm", false);
        let rs = plan.render_sequence.as_ref().expect("render_sequence");
        assert_eq!(rs.frame_count, 120, "slow variant is 120 frames");
        assert!(rs.capture_positions);
        assert!(rs.capture_synthetic_colliders);
        assert!(plan.ccd_colliders.is_none());
        plan.validate().expect("plan must validate");
    }
}
