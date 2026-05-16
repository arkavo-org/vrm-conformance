//! YAML schema for VRM conformance test plans.
//!
//! See `docs/methodology.md` for why specific defaults exist (tone_mapping=none,
//! shadows off, MSAA 4x).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestPlan {
    pub id: String,
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
