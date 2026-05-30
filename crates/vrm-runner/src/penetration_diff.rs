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

use anyhow::{bail, Context, Result};
use camino::Utf8Path;
use vrm_diff_engine::penetration::{worst_penetration, ColliderSpec, PenetrationReport};
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

/// Load the positions JSON from `positions_json_path`, reshape it into
/// `Vec<Vec<SpringPositions>>` (one inner `Vec` per frame, ordered by
/// `frame_index`), load the test plan at `plan_path`, map `ccd_colliders`
/// to engine specs, and call `worst_penetration`.
///
/// Returns an error when:
/// - The file cannot be read or is invalid JSON.
/// - The plan cannot be parsed.
/// - The plan has no `ccd_colliders` (or an empty list).
pub fn run_penetration_diff(
    positions_json_path: &Utf8Path,
    plan_path: &Utf8Path,
    epsilon_m: f32,
) -> Result<PenetrationReport> {
    // ── Load positions ────────────────────────────────────────────────────────
    let positions_raw = std::fs::read_to_string(positions_json_path)
        .with_context(|| format!("failed to read positions file {positions_json_path}"))?;

    let mut entries: Vec<FramePositionsEntry> = serde_json::from_str(&positions_raw)
        .with_context(|| format!("failed to parse positions JSON {positions_json_path}"))?;

    // Sort by frame_index so the `worst_frame` index in the report is
    // meaningful and deterministic regardless of file ordering.
    entries.sort_by_key(|e| e.frame_index);

    // Reshape: Vec<FramePositionsEntry> → Vec<Vec<SpringPositions>>
    let frames: Vec<Vec<SpringPositions>> = entries.into_iter().map(|e| e.springs).collect();

    // ── Load plan ─────────────────────────────────────────────────────────────
    let plan_raw = std::fs::read_to_string(plan_path)
        .with_context(|| format!("failed to read plan file {plan_path}"))?;

    let plan: vrm_test_plan::TestPlan = serde_yml::from_str(&plan_raw)
        .with_context(|| format!("failed to parse test plan {plan_path}"))?;

    // ── Extract and map colliders ─────────────────────────────────────────────
    let world_specs = plan.ccd_colliders.as_deref().unwrap_or(&[]);
    if world_specs.is_empty() {
        bail!("plan has no ccd_colliders — cannot run penetration-diff");
    }

    let colliders: Vec<ColliderSpec> = world_specs.iter().map(to_collider_spec).collect();

    // ── Run penetration check ─────────────────────────────────────────────────
    Ok(worst_penetration(&frames, &colliders, epsilon_m))
}
