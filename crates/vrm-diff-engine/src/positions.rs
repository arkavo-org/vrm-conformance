//! Position-space diff for spring-bone joint world coordinates.
//!
//! Two thresholds: per-joint max drift and chain-summed drift. Both must
//! hold for `passed = true`. A chain of N joints each 1 mm off baseline
//! is a different bug from a chain with one joint 10 mm off.

use serde::{Deserialize, Serialize};
use vrm_ops::tools::SpringPositions;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PositionDiffReport {
    pub per_joint_max_drift_m: f32,
    pub chain_summed_drift_m: f32,
    pub per_joint_tolerance_m: f32,
    pub chain_max_drift_m: f32,
    pub worst_joint_index: usize,
    pub passed: bool,
}

/// Returns a structural-failure report when joint counts differ. Callers
/// MUST treat `passed = false` as a hard failure regardless of magnitudes —
/// counts must match for thresholds to be meaningful.
pub fn diff_positions(
    actual: &SpringPositions,
    reference: &SpringPositions,
    per_joint_tolerance_m: f32,
    chain_max_drift_m: f32,
) -> PositionDiffReport {
    if actual.joint_positions.len() != reference.joint_positions.len() {
        return PositionDiffReport {
            per_joint_max_drift_m: f32::INFINITY,
            chain_summed_drift_m: f32::INFINITY,
            per_joint_tolerance_m,
            chain_max_drift_m,
            worst_joint_index: 0,
            passed: false,
        };
    }

    let mut per_joint_max = 0.0_f32;
    let mut chain_summed = 0.0_f32;
    let mut worst_joint = 0usize;

    for (i, (a, b)) in actual
        .joint_positions
        .iter()
        .zip(reference.joint_positions.iter())
        .enumerate()
    {
        let dx = a[0] - b[0];
        let dy = a[1] - b[1];
        let dz = a[2] - b[2];
        let d = (dx * dx + dy * dy + dz * dz).sqrt();
        chain_summed += d;
        if d > per_joint_max {
            per_joint_max = d;
            worst_joint = i;
        }
    }

    let passed = per_joint_max <= per_joint_tolerance_m && chain_summed <= chain_max_drift_m;

    PositionDiffReport {
        per_joint_max_drift_m: per_joint_max,
        chain_summed_drift_m: chain_summed,
        per_joint_tolerance_m,
        chain_max_drift_m,
        worst_joint_index: worst_joint,
        passed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vrm_ops::tools::SpringPositions;

    fn pos(name: &str, joints: Vec<[f32; 3]>) -> SpringPositions {
        SpringPositions {
            name: name.into(),
            joint_positions: joints,
        }
    }

    #[test]
    fn identical_positions_pass() {
        let a = pos("c", vec![[0.0, 1.0, 0.0], [0.0, 0.95, 0.0]]);
        let report = diff_positions(&a, &a, 0.005, 0.020);
        assert!(report.passed);
        assert_eq!(report.per_joint_max_drift_m, 0.0);
        assert_eq!(report.chain_summed_drift_m, 0.0);
    }

    #[test]
    fn single_joint_drift_within_tolerance_passes() {
        let a = pos("c", vec![[0.0, 1.0, 0.0], [0.0, 0.95, 0.0]]);
        let b = pos("c", vec![[0.0, 1.0, 0.0], [0.0, 0.953, 0.0]]); // 3 mm drift
        let report = diff_positions(&a, &b, 0.005, 0.020);
        assert!(report.passed);
        assert!((report.per_joint_max_drift_m - 0.003).abs() < 1e-5);
        assert_eq!(report.worst_joint_index, 1);
    }

    #[test]
    fn single_joint_exceeds_per_joint_tolerance_fails() {
        let a = pos("c", vec![[0.0, 1.0, 0.0], [0.0, 0.95, 0.0]]);
        let b = pos("c", vec![[0.0, 1.0, 0.0], [0.0, 0.94, 0.0]]); // 10 mm drift
        let report = diff_positions(&a, &b, 0.005, 0.020);
        assert!(!report.passed);
        assert_eq!(report.worst_joint_index, 1);
    }

    #[test]
    fn chain_summed_drift_exceeds_threshold_fails_even_if_each_joint_within() {
        // 5 joints × 4.5 mm each: per-joint 4.5 mm (under 5 mm tol),
        // chain-summed = 22.5 mm (over 20 mm tol). Expected: FAIL.
        let a = pos(
            "c",
            vec![
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
        );
        let b = pos(
            "c",
            vec![
                [0.0045, 1.0, 0.0],
                [0.0045, 1.0, 0.0],
                [0.0045, 1.0, 0.0],
                [0.0045, 1.0, 0.0],
                [0.0045, 1.0, 0.0],
            ],
        );
        let report = diff_positions(&a, &b, 0.005, 0.020);
        assert!(
            !report.passed,
            "per-joint 4.5mm under 5mm tol but chain-sum 22.5mm exceeds 20mm tol; expected fail"
        );
        assert!(report.chain_summed_drift_m > 0.020);
    }

    #[test]
    fn mismatched_joint_count_fails_with_zero_passed() {
        let a = pos("c", vec![[0.0, 1.0, 0.0], [0.0, 0.95, 0.0]]);
        let b = pos("c", vec![[0.0, 1.0, 0.0]]);
        let report = diff_positions(&a, &b, 0.005, 0.020);
        assert!(
            !report.passed,
            "different joint counts MUST fail; this is a structural error not a threshold one"
        );
    }
}
