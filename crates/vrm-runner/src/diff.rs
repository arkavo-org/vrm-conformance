//! Bridges a `vrm_test_plan::TestPlan` plus two rendered PNGs (the
//! produced render and a known-good reference) into a `DiffResult`. SSIM
//! compares render vs reference; property assertions are evaluated on
//! the render alone (they measure absolute renderer behavior, not
//! similarity to a reference).

use anyhow::{Context, Result};
use camino::Utf8Path;
use vrm_diff_engine::pose_diff::{diff_pose, PoseDiffReport, PoseTolerances};
use vrm_diff_engine::positions::{diff_positions, PositionDiffReport};
use vrm_diff_engine::property::eval_property;
use vrm_diff_engine::result::DiffResult;
use vrm_diff_engine::ssim::ssim_pngs;
use vrm_ops::tools::{
    DumpBonePositionsResult, DumpExpressionWeightsResult, DumpHumanoidPoseResult,
    DumpLookAtStateResult,
};
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

/// Reference fixture file format for pose_diff. The runner writes the
/// adapter's dump_humanoid_pose / dump_expression_weights /
/// dump_look_at_state results under `<output_dir>/<id>_<renderer>.pose.json`
/// during execute_plan; the reference passed to `reference_pose_json` must
/// have the same shape.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ReferencePoseFixture {
    pub humanoid: DumpHumanoidPoseResult,
    pub expressions: DumpExpressionWeightsResult,
    pub look_at: DumpLookAtStateResult,
}

/// Run pose_diff against a reference fixture file. The actual side is the
/// per-test pose.json the runner wrote during execute_plan. Tolerances
/// come from `plan.diff.pose_tolerance` if set, else PoseTolerances::default().
pub fn diff_pose_one(
    plan: &TestPlan,
    actual_path: &Utf8Path,
    reference_path: &Utf8Path,
) -> Result<PoseDiffReport> {
    let actual: ReferencePoseFixture = serde_json::from_str(
        &std::fs::read_to_string(actual_path)
            .with_context(|| format!("reading actual pose dump {actual_path}"))?,
    )
    .with_context(|| format!("parsing actual pose dump {actual_path}"))?;
    let reference: ReferencePoseFixture = serde_json::from_str(
        &std::fs::read_to_string(reference_path)
            .with_context(|| format!("reading reference_pose_json {reference_path}"))?,
    )
    .with_context(|| format!("parsing reference_pose_json {reference_path}"))?;

    let tolerances = plan
        .diff
        .pose_tolerance
        .as_ref()
        .map(pose_tolerance_to_engine)
        .unwrap_or_default();

    Ok(diff_pose(
        &actual.humanoid,
        &reference.humanoid,
        &actual.expressions,
        &reference.expressions,
        &actual.look_at,
        &reference.look_at,
        &tolerances,
    ))
}

fn pose_tolerance_to_engine(t: &vrm_test_plan::PoseTolerance) -> PoseTolerances {
    PoseTolerances {
        per_bone_quaternion_radians: t.per_bone_quaternion_radians,
        hips_translation_m: t.hips_translation_m,
        per_preset_expression: t.per_preset_expression,
        per_custom_expression: t.per_custom_expression,
        look_at_yaw_pitch_degrees: t.look_at_yaw_pitch_degrees,
        offset_from_head_bone_m: t.offset_from_head_bone_m,
    }
}
