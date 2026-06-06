//! `penetration-diff` subcommand implementation.
//!
//! Loads a per-frame spring-bone positions JSON (produced by the runner when
//! `capture_positions = true`) and the test plan that drove the capture,
//! then delegates to `vrm_diff_engine::penetration::worst_penetration` using
//! the plan's `ccd_colliders` list.
//!
//! The positions file is a JSON array of [`crate::execute::FramePositionsEntry`]
//! values.  Entries are sorted by `frame_index` before the call so that the
//! `worst_frame` index in the returned report is meaningful.
//!
//! ## worst_frame translation
//!
//! The engine returns `worst_frame` as a 0-based **slice index** into the
//! sorted `frames` slice.  When an adapter captures only a subset of frames
//! (e.g. frames 0, 5, 10), that slice index ≠ the original `frame_index`.
//! [`run_penetration_diff`] translates the slice index back to the real
//! `frame_index` and stores it in
//! [`PenetrationDiffResult::worst_frame_index`].  The engine-level slice index
//! is retained as `worst_frame_slice` for debugging.

use anyhow::{bail, Context, Result};
use camino::Utf8Path;
use serde::{Deserialize, Serialize};
use vrm_diff_engine::penetration::{worst_penetration, ColliderSpec};
use vrm_ops::tools::SpringPositions;
use vrm_test_plan::ColliderWorldSpec;

use crate::execute::FramePositionsEntry;

/// Map a `ColliderWorldSpec` (from the test plan) to a
/// `vrm_diff_engine::penetration::ColliderSpec`.
/// The two enums are structurally identical; this mapper lives in the runner
/// so that `vrm-test-plan` stays free of any `vrm-diff-engine` dependency.
pub fn to_collider_spec(c: &ColliderWorldSpec) -> ColliderSpec {
    match c {
        ColliderWorldSpec::Sphere { center, radius } => ColliderSpec::Sphere {
            center: *center,
            radius: *radius,
        },
        ColliderWorldSpec::Capsule { a, b, radius } => ColliderSpec::Capsule {
            a: *a,
            b: *b,
            radius: *radius,
        },
    }
}

/// Map a `vrm_ops::tools::SequenceCollider` (per-frame capture) to the engine
/// `ColliderSpec`. Structurally identical; lives in the runner per the
/// dependency-direction rule.
pub fn to_collider_spec_seq(c: &vrm_ops::tools::SequenceCollider) -> ColliderSpec {
    use vrm_ops::tools::SequenceCollider as S;
    match c {
        S::Sphere { center, radius } => ColliderSpec::Sphere {
            center: *center,
            radius: *radius,
        },
        S::Capsule { a, b, radius } => ColliderSpec::Capsule {
            a: *a,
            b: *b,
            radius: *radius,
        },
    }
}

/// Runner-level penetration diff result.  Wraps the engine's `PenetrationReport`
/// and adds `worst_frame_index` — the real `frame_index` from the positions
/// JSON (not the 0-based slice index used internally by the engine).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PenetrationDiffResult {
    pub max_penetration_depth_m: f32,
    pub epsilon_m: f32,
    /// The original `frame_index` field from the positions JSON entry that
    /// contained the worst penetration.  This is what callers should surface
    /// in CLI/JSON output.
    pub worst_frame_index: u32,
    /// The 0-based slice index into the sorted frames slice (engine-internal).
    /// Retained for debugging; most callers want `worst_frame_index`.
    pub worst_frame_slice: usize,
    pub worst_spring: usize,
    pub worst_joint: usize,
    pub passed: bool,
}

/// Load the positions JSON from `positions_json_path`, reshape it into
/// `Vec<Vec<SpringPositions>>` (one inner `Vec` per frame, ordered by
/// `frame_index`), load the test plan at `plan_path`, map colliders
/// to engine specs, and call the appropriate penetration check.
///
/// When `colliders_json_path` is `Some`, the per-frame moving colliders file
/// is used (bone-attached synthetic colliders captured alongside positions).
/// When `None`, the plan's static `ccd_colliders` list is used instead.
///
/// The returned [`PenetrationDiffResult::worst_frame_index`] is the real
/// `frame_index` from the positions JSON (not the engine's slice index).
///
/// Returns an error when:
/// - Any file cannot be read or is invalid JSON/YAML.
/// - The plan cannot be parsed.
/// - Neither `colliders_json_path` nor the plan's `ccd_colliders` provide
///   any colliders.
pub fn run_penetration_diff(
    positions_json_path: &Utf8Path,
    plan_path: &Utf8Path,
    colliders_json_path: Option<&Utf8Path>,
    epsilon_m: f32,
) -> Result<PenetrationDiffResult> {
    // ── Load positions ────────────────────────────────────────────────────────
    let positions_raw = std::fs::read_to_string(positions_json_path)
        .with_context(|| format!("failed to read positions file {positions_json_path}"))?;

    let mut entries: Vec<FramePositionsEntry> = serde_json::from_str(&positions_raw)
        .with_context(|| format!("failed to parse positions JSON {positions_json_path}"))?;

    // Sort by frame_index so the engine's worst_frame slice index is
    // deterministic regardless of file ordering.
    entries.sort_by_key(|e| e.frame_index);

    // Retain the original frame_index values so we can translate back after
    // the engine call.
    let original_frame_indices: Vec<u32> = entries.iter().map(|e| e.frame_index).collect();

    // Reshape: Vec<FramePositionsEntry> → Vec<Vec<SpringPositions>>
    let frames: Vec<Vec<SpringPositions>> = entries.into_iter().map(|e| e.springs).collect();

    // ── Load plan ─────────────────────────────────────────────────────────────
    let plan_raw = std::fs::read_to_string(plan_path)
        .with_context(|| format!("failed to read plan file {plan_path}"))?;

    let plan: vrm_test_plan::TestPlan = serde_yml::from_str(&plan_raw)
        .with_context(|| format!("failed to parse test plan {plan_path}"))?;

    // ── Run penetration check (branch on collider source) ────────────────────
    let report = if let Some(col_path) = colliders_json_path {
        use crate::execute::FrameCollidersEntry;
        let col_raw = std::fs::read_to_string(col_path)
            .with_context(|| format!("failed to read colliders file {col_path}"))?;
        let mut col_entries: Vec<FrameCollidersEntry> = serde_json::from_str(&col_raw)
            .with_context(|| format!("failed to parse colliders JSON {col_path}"))?;
        col_entries.sort_by_key(|e| e.frame_index);
        let by_index: std::collections::HashMap<u32, Vec<ColliderSpec>> = col_entries
            .into_iter()
            .map(|e| {
                (
                    e.frame_index,
                    e.colliders.iter().map(to_collider_spec_seq).collect(),
                )
            })
            .collect();
        let colliders_per_frame: Vec<Vec<ColliderSpec>> = original_frame_indices
            .iter()
            .map(|fi| by_index.get(fi).cloned().unwrap_or_default())
            .collect();
        vrm_diff_engine::penetration::worst_penetration_per_frame(
            &frames,
            &colliders_per_frame,
            epsilon_m,
        )
    } else {
        let world_specs = plan.ccd_colliders.as_deref().unwrap_or(&[]);
        if world_specs.is_empty() {
            bail!(
                "plan has no ccd_colliders and no --colliders given — cannot run penetration-diff"
            );
        }
        let colliders: Vec<ColliderSpec> = world_specs.iter().map(to_collider_spec).collect();
        worst_penetration(&frames, &colliders, epsilon_m)
    };

    // Translate the engine's slice index back to the real frame_index.
    let worst_frame_index = original_frame_indices
        .get(report.worst_frame)
        .copied()
        .unwrap_or(0);

    Ok(PenetrationDiffResult {
        max_penetration_depth_m: report.max_penetration_depth_m,
        epsilon_m: report.epsilon_m,
        worst_frame_index,
        worst_frame_slice: report.worst_frame,
        worst_spring: report.worst_spring,
        worst_joint: report.worst_joint,
        passed: report.passed,
    })
}

#[cfg(test)]
mod per_frame_colliders_tests {
    use super::*;
    use crate::execute::{FrameCollidersEntry, FramePositionsEntry};
    use vrm_ops::tools::{SequenceCollider, SpringPositions};

    #[test]
    fn colliders_path_measures_against_moving_colliders() {
        let dir = tempfile::tempdir().unwrap();
        let d = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();

        let positions = vec![
            FramePositionsEntry {
                frame_index: 0,
                timestamp_seconds: 0.0,
                springs: vec![SpringPositions {
                    name: "hair".into(),
                    joint_positions: vec![[0.10, 0.0, 0.0]],
                }],
            },
            FramePositionsEntry {
                frame_index: 1,
                timestamp_seconds: 0.033,
                springs: vec![SpringPositions {
                    name: "hair".into(),
                    joint_positions: vec![[0.10, 0.0, 0.0]],
                }],
            },
        ];
        let colliders = vec![
            FrameCollidersEntry {
                frame_index: 0,
                timestamp_seconds: 0.0,
                colliders: vec![SequenceCollider::Sphere {
                    center: [0.20, 0.0, 0.0],
                    radius: 0.05,
                }],
            },
            FrameCollidersEntry {
                frame_index: 1,
                timestamp_seconds: 0.033,
                colliders: vec![SequenceCollider::Sphere {
                    center: [0.12, 0.0, 0.0],
                    radius: 0.05,
                }],
            },
        ];
        let pos_path = d.join("x_vmk_positions.json");
        let col_path = d.join("x_vmk_colliders.json");
        std::fs::write(&pos_path, serde_json::to_string(&positions).unwrap()).unwrap();
        std::fs::write(&col_path, serde_json::to_string(&colliders).unwrap()).unwrap();

        let plan_path = d.join("x.test.yaml");
        std::fs::write(&plan_path, MINIMAL_PLAN_YAML).unwrap();

        let r = run_penetration_diff(&pos_path, &plan_path, Some(&col_path), 0.002).unwrap();
        assert!(!r.passed);
        assert!((r.max_penetration_depth_m - 0.03).abs() < 1e-5);
        assert_eq!(r.worst_frame_index, 1);
    }

    const MINIMAL_PLAN_YAML: &str = r#"id: x
spec_section: test
asset: x.vrm
camera:
  position:
  - 0.0
  - 1.4
  - 1.5
  target:
  - 0.0
  - 1.4
  - 0.0
  up:
  - 0.0
  - 1.0
  - 0.0
  fov_degrees: 30.0
lighting:
  directional:
    dir:
    - -0.3
    - -0.6
    - -0.7
    color:
    - 1.0
    - 1.0
    - 1.0
    intensity: 1.0
  ambient:
    color:
    - 0.5
    - 0.5
    - 0.5
    intensity: 0.3
  cast_shadows: false
  receive_shadows: false
output:
  width: 1024
  height: 1024
  color_space: srgb
  msaa: 4
diff:
  mode: ssim
  threshold: 0.85
  reference_renderer: univrm
  conformance_status:
    kind: included
"#;
}
