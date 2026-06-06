//! YAML schema for VRM conformance test plans.
//!
//! See `docs/methodology.md` for why specific defaults exist (tone_mapping=none,
//! shadows off, MSAA 4x).

use serde::{Deserialize, Serialize};
pub use vrm_ops::SpecVersion;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestPlan {
    pub id: String,
    #[serde(default = "default_spec_version_v1")]
    pub spec_version: SpecVersion,
    pub spec_section: String,
    pub asset: String,
    pub camera: Camera,
    pub lighting: Lighting,
    #[serde(default)]
    pub post_processing: PostProcessing,
    pub output: Output,
    pub diff: Diff,
    #[serde(default)]
    pub ignore_renderers: Vec<String>,
    #[serde(default)]
    pub properties: Vec<PropertyAssertion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physics: Option<PhysicsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<AnimationConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_sequence: Option<RenderSequenceBlock>,
    /// Cross-variant SSIM assertion. When present, this test's render and the
    /// render of `sibling_id` (SAME renderer) MUST visibly differ. See
    /// `CrossVariantAssertion`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cross_variant: Option<CrossVariantAssertion>,
    /// World-fixed colliders to check for spring-bone non-penetration.
    /// When present (CCD conformance plans), the runner can run
    /// `penetration-diff` against the captured per-frame positions.
    /// Self-contained — does NOT pull in `vrm-diff-engine`; the runner
    /// maps `ColliderWorldSpec` → `vrm_diff_engine::penetration::ColliderSpec`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ccd_colliders: Option<Vec<ColliderWorldSpec>>,
}

/// World-fixed collider geometry for spring-bone CCD conformance checks.
/// Mirrors `vrm_diff_engine::penetration::ColliderSpec` but lives here so
/// `vrm-test-plan` stays free of any `vrm-diff-engine` dependency.
/// The runner maps this type to the diff-engine type via
/// `vrm_runner::penetration_diff::to_collider_spec`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ColliderWorldSpec {
    Sphere {
        center: [f32; 3],
        radius: f32,
    },
    Capsule {
        a: [f32; 3],
        b: [f32; 3],
        radius: f32,
    },
}

/// Cross-variant SSIM assertion: the render of THIS test and the render of
/// `sibling_id` (same renderer) MUST visibly differ. Pass iff their SSIM is
/// at or below `max_ssim`. Used by the doubleSided back-face-culling spec
/// test, where doubleSided=false culls the surface (all-background frame) and
/// doubleSided=true renders it — a conformant renderer's two outputs diverge;
/// a name-heuristic renderer's do not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossVariantAssertion {
    /// Test id (and asset stem) of the sibling variant to compare against.
    pub sibling_id: String,
    /// Pass iff `ssim(this_render, sibling_render) <= max_ssim`.
    pub max_ssim: f32,
}

/// Optional physics-stepping config for spring-bone / collider tests.
/// When present, the runner calls `reset_physics(settle_steps)` between
/// `set_post_processing` and `render`. Defaults to 30 steps at 60 Hz
/// (0.5 s) per `docs/methodology.md` "Spring bone initial state".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicsConfig {
    pub settle_steps: u32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self { settle_steps: 30 }
    }
}

/// Optional root-transform animation block. When present, the runner calls
/// `animate_root_transform` between `reset_physics` (if any) and `render`,
/// so the spring-bone chain experiences inertia/drag rather than only the
/// gravity settle. See `docs/methodology.md` "Spring bone excitation".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_transform: Option<RootTransformAnimation>,
    /// VRMA animation source. Independent of `root_transform`: a plan
    /// can use either or both — `root_transform` drives root translation
    /// in world space; `vrma` drives internal humanoid pose, expressions,
    /// and lookAt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vrma: Option<VrmaAnimation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RootTransformAnimation {
    pub translation_start: [f32; 3],
    pub translation_end: [f32; 3],
    pub duration_seconds: f32,
    #[serde(default = "default_animation_fps")]
    pub fps: u32,
}

fn default_animation_fps() -> u32 {
    60
}

fn default_spec_version_v1() -> SpecVersion {
    SpecVersion::V1
}

/// VRMA (VRMC_vrm_animation) animation block. When present, the runner
/// loads the .vrma, calls `apply_vrma_at_time(t)` between
/// `set_post_processing` and `reset_physics`/`render`, then dumps
/// humanoid pose + expression weights + lookAt state for pose-vector
/// diff against the consortium reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VrmaAnimation {
    /// Path to the `.vrma` file (filesystem path or BLAKE3 ref).
    pub path: String,
    /// Sample time in seconds for `apply_vrma_at_time`.
    pub apply_at_time: f32,
}

/// Per-channel tolerances for the pose_diff signal. Mirrors
/// `vrm_diff_engine::pose_diff::PoseTolerances`. Optional — if absent,
/// the runner uses the diff-engine defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoseTolerance {
    pub per_bone_quaternion_radians: f32,
    pub hips_translation_m: f32,
    pub per_preset_expression: f32,
    pub per_custom_expression: f32,
    pub look_at_yaw_pitch_degrees: f32,
    pub offset_from_head_bone_m: f32,
}

/// Optional sequence-capture config. When present, the runner dispatches
/// `render_sequence` instead of (or in addition to) the single-frame
/// `render` op. Field names mirror `vrm_ops::tools::RenderSequenceParams`
/// for direct projection in `plan_to_ops`.
///
/// `output.width` / `output.height` / `output.color_space` / `output.msaa`
/// are reused — sequence plans share the `Output` block with single-frame
/// plans rather than carrying duplicate geometry fields.
///
/// Mutual exclusion: a plan with both `animation` and `render_sequence`
/// set is ambiguous (the existing `animation.root_transform` block is
/// the single-frame counterpart to `render_sequence.animate_root_transform`,
/// and `animation.vrma` overlaps with `render_sequence.apply_vrma`).
/// `TestPlan::validate` (added in the next task) rejects such plans.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderSequenceBlock {
    pub frame_count: u32,
    pub frame_hz: f32,
    pub physics_dt_seconds: f32,
    pub output_format: SequenceFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate_root_transform: Option<SequenceRootTransformAnimation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_vrma: Option<VrmaPlaybackSpec>,
    /// Optional override of the RFC-0004 default temporal_ssim_threshold (0.90).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal_ssim_threshold: Option<f64>,
    /// When true, the runner sets `capture_positions = true` in
    /// `RenderSequenceParams` and, after the sequence completes, persists
    /// the per-frame spring-bone joint positions to
    /// `<output_dir>/<plan_id>_<renderer>_positions.json`.
    /// Default false so existing sequence plans parse unchanged.
    #[serde(default)]
    pub capture_positions: bool,
    /// When true, the runner sets `capture_synthetic_colliders = true` in
    /// `RenderSequenceParams` and persists the per-frame world-space synthetic
    /// colliders to `<output_dir>/<plan_id>_<renderer>_colliders.json`.
    /// Default false so existing sequence plans parse unchanged.
    #[serde(default)]
    pub capture_synthetic_colliders: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SequenceFormat {
    PngSequence,
    Mp4,
    Mov,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SequenceRootTransformAnimation {
    pub translation_start: [f32; 3],
    pub translation_end: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VrmaPlaybackSpec {
    pub vrma_handle: u32,
    pub start_seconds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fov_degrees: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lighting {
    pub directional: DirectionalLight,
    pub ambient: AmbientLight,
    #[serde(default)]
    pub cast_shadows: bool,
    #[serde(default)]
    pub receive_shadows: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DirectionalLight {
    pub dir: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AmbientLight {
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostProcessing {
    #[serde(default = "default_tone_mapping")]
    pub tone_mapping: ToneMapping,
    #[serde(default = "default_exposure")]
    pub exposure: f32,
}

impl Default for PostProcessing {
    fn default() -> Self {
        Self {
            tone_mapping: ToneMapping::None,
            exposure: 1.0,
        }
    }
}

fn default_tone_mapping() -> ToneMapping {
    ToneMapping::None
}

fn default_exposure() -> f32 {
    1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToneMapping {
    None,
    Linear,
    Reinhard,
    Aces,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Output {
    pub width: u32,
    pub height: u32,
    pub color_space: ColorSpace,
    pub msaa: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorSpace {
    Linear,
    Srgb,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diff {
    pub mode: DiffMode,
    pub threshold: f32,
    pub reference_renderer: String,
    /// Conformance-pass-rate status for this test. `Included` (the default) means
    /// this test counts toward the corpus's headline pass-rate. `Excluded` means
    /// the test renders (so regressions are still surfaced visually) but the
    /// SSIM number does not roll into the conformance claim — for tests where
    /// whole-frame SSIM is the wrong metric (see `docs/methodology.md`).
    ///
    /// Per [vrm-conformance#3]: outline tests at width ≥ 0.05 m produce a
    /// spec-correct flooded mesh; whole-frame SSIM only measures silhouette
    /// anti-aliasing on a frame with no other signal.
    #[serde(default)]
    pub conformance_status: ConformanceStatus,
    /// Per-channel pose-diff tolerances when `animation.vrma` is set.
    /// If absent, the runner uses `PoseTolerances::default()`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pose_tolerance: Option<PoseTolerance>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConformanceStatus {
    #[default]
    Included,
    Excluded {
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffMode {
    Ssim,
    Consensus,
}

// Property assertions and bbox regions are defined in `vrm-diff-engine`
// (the consumer of these types) and re-exported here so test-plan YAML
// authors can use the canonical names without a direct diff-engine
// dependency leaking through.
pub use vrm_diff_engine::property::{BboxRegion, PropertyAssertion};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum TestPlanError {
    #[error(
        "test plan declares both `animation` and `render_sequence` — these are \
        mutually exclusive (single-frame animation vs multi-frame sequence). \
        Use `render_sequence.animate_root_transform` or `render_sequence.apply_vrma` \
        for sequence-mode tests; use `animation.root_transform` or \
        `animation.vrma` for single-frame tests."
    )]
    BothAnimationAndRenderSequence,

    #[error(
        "test plan has `ccd_colliders` but `render_sequence` is absent or \
        `capture_positions` is false — the penetration-diff metric requires \
        `render_sequence` with `capture_positions: true` so the runner can \
        record per-frame joint positions. Either remove `ccd_colliders` or add \
        a `render_sequence` block with `capture_positions: true`."
    )]
    CcdCollidersRequireCapture,
}

impl TestPlan {
    /// Validate cross-field rules that serde can't express alone.
    ///
    /// Currently checks:
    ///   - `animation` and `render_sequence` are mutually exclusive
    ///     (RFC-0004: a plan is single-frame OR sequence, not both).
    ///   - `ccd_colliders` requires `render_sequence` with `capture_positions: true`
    ///     (the penetration-diff metric needs per-frame joint positions).
    pub fn validate(&self) -> Result<(), TestPlanError> {
        if self.animation.is_some() && self.render_sequence.is_some() {
            return Err(TestPlanError::BothAnimationAndRenderSequence);
        }
        if let Some(colliders) = &self.ccd_colliders {
            if !colliders.is_empty() {
                let capture_ok = self
                    .render_sequence
                    .as_ref()
                    .map(|rs| rs.capture_positions)
                    .unwrap_or(false);
                if !capture_ok {
                    return Err(TestPlanError::CcdCollidersRequireCapture);
                }
            }
        }
        Ok(())
    }
}

/// One parameter-perturbed variant in a coupling matrix run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CouplingPerturbation {
    pub name: String,
    /// Asset filename (resolved relative to asset_dir at runtime).
    pub asset: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Coupling matrix: baseline + N perturbation variants rendered through
/// the same base plan. The runner computes per-joint position drift between
/// the baseline and each perturbation to detect VMK#162-class coupling
/// regressions ("changing one tuned parameter silently shifts the equilibrium
/// that other parameters establish").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CouplingMatrix {
    /// Path to the base test plan (resolved relative to the matrix YAML's dir).
    pub base_plan: String,
    /// Baseline asset filename (resolved relative to asset_dir at runtime).
    pub baseline_asset: String,
    pub perturbations: Vec<CouplingPerturbation>,
    /// Max allowed per-joint position delta between baseline and any perturbation.
    /// Cross-perturbation drift exceeding this is flagged as coupling.
    pub coupling_threshold_m: f32,
}

#[cfg(test)]
mod spec_version_tests {
    use super::*;

    fn minimal_yaml_with_spec_version(v: &str) -> String {
        format!(
            r#"
id: t
spec_version: "{v}"
spec_section: test
asset: a.vrm
camera:
  position: [0, 1.3, 1.5]
  target: [0, 1.3, 0]
  up: [0, 1, 0]
  fov_degrees: 30
lighting:
  directional: {{ dir: [0, -1, 0], color: [1, 1, 1], intensity: 1.0 }}
  ambient: {{ color: [1, 1, 1], intensity: 0.2 }}
output:
  width: 256
  height: 256
  color_space: srgb
  msaa: 1
diff:
  mode: ssim
  threshold: 0.95
  reference_renderer: mock
"#
        )
    }

    #[test]
    fn parses_spec_version_v0() {
        let yaml = minimal_yaml_with_spec_version("0.x");
        let plan: TestPlan = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(plan.spec_version, SpecVersion::V0);
    }

    #[test]
    fn parses_spec_version_v1() {
        let yaml = minimal_yaml_with_spec_version("1.0");
        let plan: TestPlan = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(plan.spec_version, SpecVersion::V1);
    }

    #[test]
    fn defaults_to_v1_when_absent() {
        // Back-compat: legacy plans without spec_version parse as VRM 1.0.
        let yaml = r#"
id: t
spec_section: test
asset: a.vrm
camera:
  position: [0, 1.3, 1.5]
  target: [0, 1.3, 0]
  up: [0, 1, 0]
  fov_degrees: 30
lighting:
  directional: { dir: [0, -1, 0], color: [1, 1, 1], intensity: 1.0 }
  ambient: { color: [1, 1, 1], intensity: 0.2 }
output:
  width: 256
  height: 256
  color_space: srgb
  msaa: 1
diff:
  mode: ssim
  threshold: 0.95
  reference_renderer: mock
"#;
        let plan: TestPlan = serde_yml::from_str(yaml).unwrap();
        assert_eq!(plan.spec_version, SpecVersion::V1);
    }
}

#[cfg(test)]
mod coupling_matrix_tests {
    use super::*;

    #[test]
    fn coupling_matrix_yaml_roundtrips() {
        let raw = r#"
base_plan: springbone_default.test.yaml
baseline_asset: springbone_default.vrm
perturbations:
  - name: stiffness_high
    asset: springbone_stiffness_0p55.vrm
    description: stiffness +10%
  - name: stiffness_low
    asset: springbone_stiffness_0p45.vrm
    description: stiffness -10%
coupling_threshold_m: 0.015
"#;
        let m: CouplingMatrix = serde_yml::from_str(raw).unwrap();
        assert_eq!(m.base_plan, "springbone_default.test.yaml");
        assert_eq!(m.baseline_asset, "springbone_default.vrm");
        assert_eq!(m.perturbations.len(), 2);
        assert_eq!(m.perturbations[0].name, "stiffness_high");
        assert_eq!(m.perturbations[0].asset, "springbone_stiffness_0p55.vrm");
        assert!((m.coupling_threshold_m - 0.015).abs() < 1e-6);
    }

    #[test]
    fn coupling_perturbation_description_is_optional() {
        let raw = r#"
base_plan: x.test.yaml
baseline_asset: x.vrm
perturbations:
  - { name: bare, asset: y.vrm }
coupling_threshold_m: 0.01
"#;
        let m: CouplingMatrix = serde_yml::from_str(raw).unwrap();
        assert!(m.perturbations[0].description.is_none());
    }
}

#[cfg(test)]
mod vrma_schema_tests {
    use super::*;

    #[test]
    fn animation_config_with_vrma_roundtrips() {
        let raw = r#"
root_transform:
  translation_start: [0.0, 0.0, 0.0]
  translation_end: [0.2, 0.0, 0.0]
  duration_seconds: 0.5
vrma:
  path: /tmp/x.vrma
  apply_at_time: 0.5
"#;
        let ac: AnimationConfig = serde_yml::from_str(raw).unwrap();
        let vrma = ac.vrma.as_ref().expect("vrma block");
        assert_eq!(vrma.path, "/tmp/x.vrma");
        assert!((vrma.apply_at_time - 0.5).abs() < 1e-6);
        assert!(ac.root_transform.is_some());
    }

    #[test]
    fn animation_config_without_vrma_still_parses() {
        let raw = r#"
root_transform:
  translation_start: [0.0, 0.0, 0.0]
  translation_end: [0.2, 0.0, 0.0]
  duration_seconds: 0.5
"#;
        let ac: AnimationConfig = serde_yml::from_str(raw).unwrap();
        assert!(ac.vrma.is_none());
        assert!(ac.root_transform.is_some());
    }

    #[test]
    fn diff_with_pose_tolerance_roundtrips() {
        let raw = r#"
mode: ssim
threshold: 0.95
reference_renderer: univrm
pose_tolerance:
  per_bone_quaternion_radians: 0.020
  hips_translation_m: 0.010
  per_preset_expression: 0.010
  per_custom_expression: 0.010
  look_at_yaw_pitch_degrees: 2.0
  offset_from_head_bone_m: 0.002
"#;
        let d: Diff = serde_yml::from_str(raw).unwrap();
        let tol = d.pose_tolerance.as_ref().expect("pose_tolerance");
        assert!((tol.per_bone_quaternion_radians - 0.020).abs() < 1e-6);
        assert!((tol.look_at_yaw_pitch_degrees - 2.0).abs() < 1e-6);
    }

    #[test]
    fn diff_without_pose_tolerance_still_parses() {
        let raw = r#"
mode: ssim
threshold: 0.95
reference_renderer: univrm
"#;
        let d: Diff = serde_yml::from_str(raw).unwrap();
        assert!(d.pose_tolerance.is_none());
        assert_eq!(d.reference_renderer, "univrm");
    }
}

#[cfg(test)]
mod render_sequence_tests {
    use super::*;

    #[test]
    fn render_sequence_block_minimal_roundtrips() {
        let raw = r#"
frame_count: 60
frame_hz: 30.0
physics_dt_seconds: 0.01666
output_format: png_sequence
"#;
        let b: RenderSequenceBlock = serde_yml::from_str(raw).unwrap();
        assert_eq!(b.frame_count, 60);
        assert!((b.frame_hz - 30.0).abs() < 1e-6);
        assert!(matches!(b.output_format, SequenceFormat::PngSequence));
        assert!(b.animate_root_transform.is_none());
        assert!(b.apply_vrma.is_none());
        assert!(b.temporal_ssim_threshold.is_none());
    }

    #[test]
    fn render_sequence_block_with_root_animation_roundtrips() {
        let raw = r#"
frame_count: 30
frame_hz: 30.0
physics_dt_seconds: 0.01666
output_format: mp4
animate_root_transform:
  translation_start: [0.0, 0.0, 0.0]
  translation_end: [1.0, 0.0, 0.0]
temporal_ssim_threshold: 0.92
"#;
        let b: RenderSequenceBlock = serde_yml::from_str(raw).unwrap();
        let anim = b.animate_root_transform.unwrap();
        assert_eq!(anim.translation_end, [1.0, 0.0, 0.0]);
        assert!(matches!(b.output_format, SequenceFormat::Mp4));
        assert_eq!(b.temporal_ssim_threshold, Some(0.92));
    }

    #[test]
    fn render_sequence_block_with_vrma_roundtrips() {
        let raw = r#"
frame_count: 60
frame_hz: 30.0
physics_dt_seconds: 0.01666
output_format: mov
apply_vrma:
  vrma_handle: 3
  start_seconds: 0.5
"#;
        let b: RenderSequenceBlock = serde_yml::from_str(raw).unwrap();
        let v = b.apply_vrma.unwrap();
        assert_eq!(v.vrma_handle, 3);
        assert!((v.start_seconds - 0.5).abs() < 1e-6);
        assert!(matches!(b.output_format, SequenceFormat::Mov));
    }

    #[test]
    fn test_plan_without_render_sequence_still_parses() {
        // Sanity check: existing plans (no render_sequence field) must
        // continue to deserialize cleanly.
        let raw = r#"
id: smoke
spec_section: vrm-1.0/methodology
asset: smoke.vrm
camera:
  position: [0.0, 1.2, 1.5]
  target: [0.0, 1.0, 0.0]
  up: [0.0, 1.0, 0.0]
  fov_degrees: 30.0
lighting:
  directional:
    dir: [0.0, -1.0, 0.0]
    color: [1.0, 1.0, 1.0]
    intensity: 1.0
  ambient:
    color: [1.0, 1.0, 1.0]
    intensity: 0.3
output:
  width: 512
  height: 512
  color_space: linear
  msaa: 4
diff:
  mode: ssim
  threshold: 0.90
  reference_renderer: vrm-metal-kit
"#;
        let plan: TestPlan = serde_yml::from_str(raw).unwrap();
        assert!(
            plan.render_sequence.is_none(),
            "absent field must default to None"
        );
    }

    #[test]
    fn test_plan_with_render_sequence_parses() {
        let raw = r#"
id: swing_seq
spec_section: vrm-1.0/methodology
asset: swing.vrm
camera:
  position: [0.0, 1.2, 1.5]
  target: [0.0, 1.0, 0.0]
  up: [0.0, 1.0, 0.0]
  fov_degrees: 30.0
lighting:
  directional:
    dir: [0.0, -1.0, 0.0]
    color: [1.0, 1.0, 1.0]
    intensity: 1.0
  ambient:
    color: [1.0, 1.0, 1.0]
    intensity: 0.3
output:
  width: 512
  height: 512
  color_space: linear
  msaa: 4
diff:
  mode: ssim
  threshold: 0.90
  reference_renderer: vrm-metal-kit
render_sequence:
  frame_count: 60
  frame_hz: 30.0
  physics_dt_seconds: 0.01666
  output_format: png_sequence
  animate_root_transform:
    translation_start: [0.0, 0.0, 0.0]
    translation_end: [1.0, 0.0, 0.0]
"#;
        let plan: TestPlan = serde_yml::from_str(raw).unwrap();
        let seq = plan.render_sequence.expect("render_sequence should parse");
        assert_eq!(seq.frame_count, 60);
        assert!(seq.animate_root_transform.is_some());
    }

    #[test]
    fn render_sequence_block_capture_positions_defaults_false() {
        // Legacy plans without capture_positions must parse with default=false.
        let raw = r#"
frame_count: 10
frame_hz: 30.0
physics_dt_seconds: 0.01666
output_format: png_sequence
"#;
        let b: RenderSequenceBlock = serde_yml::from_str(raw).unwrap();
        assert!(
            !b.capture_positions,
            "capture_positions must default to false"
        );
    }

    #[test]
    fn render_sequence_block_capture_synthetic_colliders_defaults_false() {
        let yaml = "frame_count: 2\nframe_hz: 30.0\nphysics_dt_seconds: 0.016666668\noutput_format: png_sequence\n";
        let block: RenderSequenceBlock = serde_yml::from_str(yaml).unwrap();
        assert!(!block.capture_synthetic_colliders);
    }

    #[test]
    fn render_sequence_block_capture_positions_explicit_true_roundtrips() {
        let raw = r#"
frame_count: 5
frame_hz: 30.0
physics_dt_seconds: 0.01666
output_format: png_sequence
capture_positions: true
"#;
        let b: RenderSequenceBlock = serde_yml::from_str(raw).unwrap();
        assert!(b.capture_positions);
        // Serialise back to YAML and re-parse; must survive the round-trip.
        let yaml = serde_yml::to_string(&b).unwrap();
        let back: RenderSequenceBlock = serde_yml::from_str(&yaml).unwrap();
        assert!(back.capture_positions);
    }

    fn make_minimal_plan() -> TestPlan {
        TestPlan {
            id: "x".into(),
            spec_version: SpecVersion::V1,
            spec_section: "x".into(),
            asset: "x.vrm".into(),
            camera: Camera {
                position: [0.0; 3],
                target: [0.0; 3],
                up: [0.0; 3],
                fov_degrees: 30.0,
            },
            lighting: Lighting {
                directional: DirectionalLight {
                    dir: [0.0; 3],
                    color: [1.0; 3],
                    intensity: 1.0,
                },
                ambient: AmbientLight {
                    color: [1.0; 3],
                    intensity: 0.3,
                },
                cast_shadows: false,
                receive_shadows: false,
            },
            post_processing: PostProcessing::default(),
            output: Output {
                width: 512,
                height: 512,
                color_space: ColorSpace::Linear,
                msaa: 4,
            },
            diff: Diff {
                mode: DiffMode::Ssim,
                threshold: 0.9,
                reference_renderer: "vmk".into(),
                conformance_status: ConformanceStatus::default(),
                pose_tolerance: None,
            },
            ignore_renderers: vec![],
            properties: vec![],
            physics: None,
            animation: None,
            render_sequence: None,
            cross_variant: None,
            ccd_colliders: None,
        }
    }

    #[test]
    fn validate_accepts_neither_animation_nor_sequence() {
        let plan = make_minimal_plan();
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn validate_accepts_animation_alone() {
        let mut plan = make_minimal_plan();
        plan.animation = Some(AnimationConfig {
            root_transform: None,
            vrma: None,
        });
        plan.render_sequence = None;
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn validate_accepts_render_sequence_alone() {
        let mut plan = make_minimal_plan();
        plan.animation = None;
        plan.render_sequence = Some(RenderSequenceBlock {
            frame_count: 30,
            frame_hz: 30.0,
            physics_dt_seconds: 0.01666,
            output_format: SequenceFormat::PngSequence,
            animate_root_transform: None,
            apply_vrma: None,
            temporal_ssim_threshold: None,
            capture_positions: false,
            capture_synthetic_colliders: false,
        });
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn validate_rejects_both_animation_and_render_sequence() {
        let mut plan = make_minimal_plan();
        plan.animation = Some(AnimationConfig {
            root_transform: None,
            vrma: None,
        });
        plan.render_sequence = Some(RenderSequenceBlock {
            frame_count: 30,
            frame_hz: 30.0,
            physics_dt_seconds: 0.01666,
            output_format: SequenceFormat::PngSequence,
            animate_root_transform: None,
            apply_vrma: None,
            temporal_ssim_threshold: None,
            capture_positions: false,
            capture_synthetic_colliders: false,
        });
        assert_eq!(
            plan.validate(),
            Err(TestPlanError::BothAnimationAndRenderSequence)
        );
    }

    #[test]
    fn cross_variant_round_trips_and_is_omitted_when_absent() {
        // Absent → key not serialized (keeps the existing corpus byte-stable).
        let plan = make_minimal_plan();
        assert!(plan.cross_variant.is_none());
        let yaml = serde_yml::to_string(&plan).unwrap();
        assert!(
            !yaml.contains("cross_variant"),
            "cross_variant must be omitted when None"
        );

        // Present → round-trips through YAML.
        let mut plan2 = make_minimal_plan();
        plan2.cross_variant = Some(CrossVariantAssertion {
            sibling_id: "sibling_xyz".into(),
            max_ssim: 0.85,
        });
        let yaml2 = serde_yml::to_string(&plan2).unwrap();
        let back: TestPlan = serde_yml::from_str(&yaml2).unwrap();
        let cv = back
            .cross_variant
            .expect("cross_variant present after round-trip");
        assert_eq!(cv.sibling_id, "sibling_xyz");
        assert!((cv.max_ssim - 0.85).abs() < 1e-6);
    }
}

#[cfg(test)]
mod ccd_colliders_tests {
    use super::*;

    fn make_minimal_plan() -> TestPlan {
        TestPlan {
            id: "x".into(),
            spec_version: SpecVersion::V1,
            spec_section: "x".into(),
            asset: "x.vrm".into(),
            camera: Camera {
                position: [0.0; 3],
                target: [0.0; 3],
                up: [0.0; 3],
                fov_degrees: 30.0,
            },
            lighting: Lighting {
                directional: DirectionalLight {
                    dir: [0.0; 3],
                    color: [1.0; 3],
                    intensity: 1.0,
                },
                ambient: AmbientLight {
                    color: [1.0; 3],
                    intensity: 0.3,
                },
                cast_shadows: false,
                receive_shadows: false,
            },
            post_processing: PostProcessing::default(),
            output: Output {
                width: 512,
                height: 512,
                color_space: ColorSpace::Linear,
                msaa: 4,
            },
            diff: Diff {
                mode: DiffMode::Ssim,
                threshold: 0.9,
                reference_renderer: "vmk".into(),
                conformance_status: ConformanceStatus::default(),
                pose_tolerance: None,
            },
            ignore_renderers: vec![],
            properties: vec![],
            physics: None,
            animation: None,
            render_sequence: None,
            cross_variant: None,
            ccd_colliders: None,
        }
    }

    #[test]
    fn ccd_colliders_absent_parses_as_none() {
        // Legacy plans without ccd_colliders must default to None.
        let plan = make_minimal_plan();
        assert!(plan.ccd_colliders.is_none());
        let yaml = serde_yml::to_string(&plan).unwrap();
        assert!(
            !yaml.contains("ccd_colliders"),
            "ccd_colliders must be omitted from YAML when None"
        );
    }

    #[test]
    fn ccd_colliders_sphere_and_capsule_roundtrip() {
        let sphere = ColliderWorldSpec::Sphere {
            center: [0.0, 1.0, 0.0],
            radius: 0.05,
        };
        let capsule = ColliderWorldSpec::Capsule {
            a: [0.0, 0.8, 0.0],
            b: [0.0, 1.2, 0.0],
            radius: 0.03,
        };
        let mut plan = make_minimal_plan();
        plan.ccd_colliders = Some(vec![sphere.clone(), capsule.clone()]);

        let yaml = serde_yml::to_string(&plan).unwrap();
        let back: TestPlan = serde_yml::from_str(&yaml).unwrap();
        let colliders = back
            .ccd_colliders
            .expect("ccd_colliders present after roundtrip");
        assert_eq!(colliders.len(), 2);
        match &colliders[0] {
            ColliderWorldSpec::Sphere { center, radius } => {
                assert_eq!(*center, [0.0_f32, 1.0, 0.0]);
                assert!((*radius - 0.05).abs() < 1e-6);
            }
            other => panic!("expected Sphere, got {other:?}"),
        }
        match &colliders[1] {
            ColliderWorldSpec::Capsule { a, b, radius } => {
                assert_eq!(*a, [0.0_f32, 0.8, 0.0]);
                assert_eq!(*b, [0.0_f32, 1.2, 0.0]);
                assert!((*radius - 0.03).abs() < 1e-6);
            }
            other => panic!("expected Capsule, got {other:?}"),
        }
    }

    #[test]
    fn validate_accepts_render_sequence_with_ccd_colliders() {
        // CCD plans have both render_sequence and ccd_colliders — validate must accept.
        let mut plan = make_minimal_plan();
        plan.render_sequence = Some(RenderSequenceBlock {
            frame_count: 30,
            frame_hz: 30.0,
            physics_dt_seconds: 0.01666,
            output_format: SequenceFormat::PngSequence,
            animate_root_transform: None,
            apply_vrma: None,
            temporal_ssim_threshold: None,
            capture_positions: true,
            capture_synthetic_colliders: false,
        });
        plan.ccd_colliders = Some(vec![ColliderWorldSpec::Sphere {
            center: [0.0, 1.0, 0.0],
            radius: 0.05,
        }]);
        assert!(plan.validate().is_ok());
    }

    /// Fix 3: ccd_colliders without render_sequence must fail validate.
    #[test]
    fn validate_rejects_ccd_colliders_without_render_sequence() {
        let mut plan = make_minimal_plan();
        // No render_sequence — ccd_colliders alone is invalid.
        plan.render_sequence = None;
        plan.ccd_colliders = Some(vec![ColliderWorldSpec::Sphere {
            center: [0.0, 1.0, 0.0],
            radius: 0.05,
        }]);
        assert_eq!(
            plan.validate(),
            Err(TestPlanError::CcdCollidersRequireCapture),
            "ccd_colliders without render_sequence must fail with CcdCollidersRequireCapture"
        );
    }

    /// Fix 3: ccd_colliders with render_sequence but capture_positions=false must fail.
    #[test]
    fn validate_rejects_ccd_colliders_when_capture_positions_false() {
        let mut plan = make_minimal_plan();
        plan.render_sequence = Some(RenderSequenceBlock {
            frame_count: 30,
            frame_hz: 30.0,
            physics_dt_seconds: 0.01666,
            output_format: SequenceFormat::PngSequence,
            animate_root_transform: None,
            apply_vrma: None,
            temporal_ssim_threshold: None,
            capture_positions: false, // <-- must be true for CCD
            capture_synthetic_colliders: false,
        });
        plan.ccd_colliders = Some(vec![ColliderWorldSpec::Sphere {
            center: [0.0, 1.0, 0.0],
            radius: 0.05,
        }]);
        assert_eq!(
            plan.validate(),
            Err(TestPlanError::CcdCollidersRequireCapture),
            "ccd_colliders with capture_positions=false must fail with CcdCollidersRequireCapture"
        );
    }

    /// Fix 3: empty ccd_colliders does not trigger the guard (Some([]) is allowed).
    #[test]
    fn validate_accepts_empty_ccd_colliders_without_render_sequence() {
        let mut plan = make_minimal_plan();
        plan.render_sequence = None;
        plan.ccd_colliders = Some(vec![]); // empty list — no colliders to check
        assert!(
            plan.validate().is_ok(),
            "empty ccd_colliders must not require render_sequence"
        );
    }
}
