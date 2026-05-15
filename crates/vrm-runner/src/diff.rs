//! Bridges a `vrm_test_plan::TestPlan` plus two rendered PNGs (the
//! produced render and a known-good reference) into a `DiffResult`. SSIM
//! compares render vs reference; property assertions are evaluated on
//! the render alone (they measure absolute renderer behavior, not
//! similarity to a reference).

use anyhow::{Context, Result};
use camino::Utf8Path;
use vrm_diff_engine::positions::{diff_positions, PositionDiffReport};
use vrm_diff_engine::property::eval_property;
use vrm_diff_engine::result::DiffResult;
use vrm_diff_engine::ssim::ssim_pngs;
use vrm_ops::tools::DumpBonePositionsResult;
use vrm_test_plan::TestPlan;

const PER_JOINT_TOL_SETTLE_M: f32 = 0.005;
const CHAIN_TOL_SETTLE_M: f32 = 0.020;
const PER_JOINT_TOL_SWING_M: f32 = 0.010;
const CHAIN_TOL_SWING_M: f32 = 0.040;

pub fn diff_one(
    plan: &TestPlan,
    render: &Utf8Path,
    reference: &Utf8Path,
    renderer: &str,
) -> Result<DiffResult> {
    let ssim = ssim_pngs(render, reference)
        .with_context(|| format!("ssim render={render} reference={reference}"))?
        as f32;
    let ssim_passed = ssim >= plan.diff.threshold;

    let mut properties = Vec::with_capacity(plan.properties.len());
    if !plan.properties.is_empty() {
        let render_img = image::open(render.as_std_path())
            .with_context(|| format!("decode render: {render}"))?
            .to_rgb8();
        for assertion in &plan.properties {
            let result = eval_property(&render_img, assertion)
                .with_context(|| format!("eval_property '{}'", assertion.name))?;
            properties.push(result);
        }
    }

    Ok(DiffResult {
        test_id: plan.id.clone(),
        renderer: renderer.into(),
        reference_renderer: plan.diff.reference_renderer.clone(),
        ssim,
        ssim_threshold: plan.diff.threshold,
        ssim_passed,
        properties,
    })
}

/// Reads a `DumpBonePositionsResult` from `reference_path`, selects
/// per-joint and chain tolerances based on whether the plan includes an
/// `animation.root_transform` (swing) or not (settle), and runs
/// `diff_positions` on the first spring chain.
///
/// Phase 1 of the springbone conformance closure design: single-chain
/// diffing only. Multi-chain N-way summary lands in a later phase.
pub fn diff_positions_one(
    plan: &TestPlan,
    actual: &DumpBonePositionsResult,
    reference_path: &camino::Utf8Path,
) -> anyhow::Result<PositionDiffReport> {
    let raw = std::fs::read_to_string(reference_path.as_std_path())?;
    let reference: DumpBonePositionsResult = serde_json::from_str(&raw)?;

    let (per_joint_tol, chain_tol) = if plan
        .animation
        .as_ref()
        .and_then(|a| a.root_transform.as_ref())
        .is_some()
    {
        (PER_JOINT_TOL_SWING_M, CHAIN_TOL_SWING_M)
    } else {
        (PER_JOINT_TOL_SETTLE_M, CHAIN_TOL_SETTLE_M)
    };

    // Empty-on-both-sides: structural pass (no chains to diff).
    if actual.springs.is_empty() && reference.springs.is_empty() {
        return Ok(PositionDiffReport {
            per_joint_max_drift_m: 0.0,
            chain_summed_drift_m: 0.0,
            per_joint_tolerance_m: per_joint_tol,
            chain_max_drift_m: chain_tol,
            worst_joint_index: 0,
            passed: true,
        });
    }

    if actual.springs.len() != reference.springs.len() {
        anyhow::bail!(
            "spring count mismatch: actual={} reference={}",
            actual.springs.len(),
            reference.springs.len()
        );
    }

    // v1.0 phase 1: diff first spring only. Multi-spring N-way summary
    // lands in phase 6 (multi-chain).
    let a = actual
        .springs
        .first()
        .ok_or_else(|| anyhow::anyhow!("actual dump contained zero springs"))?;
    let b = reference
        .springs
        .first()
        .ok_or_else(|| anyhow::anyhow!("reference dump contained zero springs"))?;

    Ok(diff_positions(a, b, per_joint_tol, chain_tol))
}
