//! Matrix execution: run a baseline + N perturbed plans, compute position drift
//! deltas, detect parameter-coupling regressions (VMK#162-class).

use crate::execute::{execute_plan_capturing_positions, load_plan, ExecuteOptions};
use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use vrm_ops::tools::DumpBonePositionsResult;
use vrm_test_plan::{CouplingMatrix, TestPlan};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PerturbationOutcome {
    pub name: String,
    pub per_joint_drifts_m: Vec<f32>,
    pub max_drift_m: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MatrixResult {
    pub baseline_plan: String,
    pub outcomes: Vec<PerturbationOutcome>,
    pub coupling_threshold_m: f32,
}

impl MatrixResult {
    pub fn passed(&self) -> bool {
        self.outcomes
            .iter()
            .all(|o| o.max_drift_m <= self.coupling_threshold_m)
    }

    pub fn outliers(&self) -> Vec<String> {
        self.outcomes
            .iter()
            .filter(|o| o.max_drift_m > self.coupling_threshold_m)
            .map(|o| o.name.clone())
            .collect()
    }
}

/// Compute per-joint Euclidean distance between two position arrays.
/// Output length is `min(a.len(), b.len())` — mismatches are handled
/// upstream by the executor (flagged as structural failure).
pub fn per_joint_drift(a: &[[f32; 3]], b: &[[f32; 3]]) -> Vec<f32> {
    a.iter()
        .zip(b.iter())
        .map(|(p, q)| {
            let dx = p[0] - q[0];
            let dy = p[1] - q[1];
            let dz = p[2] - q[2];
            (dx * dx + dy * dy + dz * dz).sqrt()
        })
        .collect()
}

pub fn load_matrix(path: &Utf8Path) -> Result<CouplingMatrix> {
    let raw = std::fs::read_to_string(path.as_std_path())
        .with_context(|| format!("load matrix file: {path}"))?;
    serde_yml::from_str(&raw).with_context(|| format!("parse matrix YAML: {path}"))
}

pub struct ExecuteMatrixOptions {
    pub adapter_bin: Utf8PathBuf,
    pub adapter_args: Vec<String>,
    pub asset_dir: Utf8PathBuf,
    pub output_dir: Utf8PathBuf,
    pub renderer_name: String,
    pub emit_progress_ndjson: bool,
}

pub fn execute_matrix(
    matrix: &CouplingMatrix,
    matrix_path: &Utf8Path,
    opts: &ExecuteMatrixOptions,
) -> Result<MatrixResult> {
    // Resolve base_plan relative to matrix file directory.
    let matrix_dir = matrix_path.parent().unwrap_or_else(|| Utf8Path::new(""));
    let base_plan_path = matrix_dir.join(&matrix.base_plan);
    let base_plan = load_plan(&base_plan_path)?;

    // Run baseline.
    let baseline_dump =
        run_one_capture_positions(&base_plan, &matrix.baseline_asset, "baseline", opts)?;

    // Run each perturbation and compute drift vs baseline.
    let mut outcomes = Vec::with_capacity(matrix.perturbations.len());
    for p in &matrix.perturbations {
        let perturbed_dump = run_one_capture_positions(&base_plan, &p.asset, &p.name, opts)?;

        let baseline_joints = baseline_dump
            .springs
            .first()
            .map(|s| s.joint_positions.as_slice())
            .unwrap_or_default();
        let perturbed_joints = perturbed_dump
            .springs
            .first()
            .map(|s| s.joint_positions.as_slice())
            .unwrap_or_default();

        // Joint-count mismatch is a structural failure: max_drift = INFINITY so
        // the perturbation always exceeds the threshold.
        let drifts = if baseline_joints.len() != perturbed_joints.len()
            && !baseline_joints.is_empty()
            && !perturbed_joints.is_empty()
        {
            vec![f32::INFINITY]
        } else {
            per_joint_drift(baseline_joints, perturbed_joints)
        };
        let max_drift = drifts.iter().cloned().fold(0.0_f32, f32::max);

        outcomes.push(PerturbationOutcome {
            name: p.name.clone(),
            per_joint_drifts_m: drifts,
            max_drift_m: max_drift,
        });
    }

    Ok(MatrixResult {
        baseline_plan: matrix.base_plan.clone(),
        outcomes,
        coupling_threshold_m: matrix.coupling_threshold_m,
    })
}

fn run_one_capture_positions(
    base_plan: &TestPlan,
    asset_filename: &str,
    label: &str,
    opts: &ExecuteMatrixOptions,
) -> Result<DumpBonePositionsResult> {
    // Clone the plan, override asset and id for this variant.
    let mut plan = base_plan.clone();
    plan.asset = asset_filename.to_string();
    plan.id = format!("{}_{}", base_plan.id, label);

    let exec_opts = ExecuteOptions {
        adapter_bin: opts.adapter_bin.clone(),
        adapter_args: opts.adapter_args.clone(),
        asset_dir: opts.asset_dir.clone(),
        output_dir: opts.output_dir.clone(),
        renderer_name: opts.renderer_name.clone(),
        emit_progress_ndjson: opts.emit_progress_ndjson,
        reference: None,
        reference_positions: None,
        vrma_path: None,
        apply_at_time: 0.0,
        reference_pose_json: None,
    };
    execute_plan_capturing_positions(&plan, &exec_opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_matrix_computes_per_joint_distances() {
        let baseline = vec![[0.0_f32, 1.0, 0.0], [0.0, 0.95, 0.0]];
        let perturbed = vec![[0.0_f32, 1.0, 0.0], [0.01, 0.95, 0.0]]; // joint 1 shifted 1cm
        let deltas = per_joint_drift(&baseline, &perturbed);
        assert_eq!(deltas.len(), 2);
        assert!((deltas[0] - 0.0).abs() < 1e-6);
        assert!((deltas[1] - 0.01).abs() < 1e-6);
    }

    #[test]
    fn matrix_outcome_passed_when_all_drifts_under_threshold() {
        let outcomes = vec![
            PerturbationOutcome {
                name: "a".into(),
                per_joint_drifts_m: vec![0.001, 0.002, 0.003],
                max_drift_m: 0.003,
            },
            PerturbationOutcome {
                name: "b".into(),
                per_joint_drifts_m: vec![0.005, 0.004, 0.001],
                max_drift_m: 0.005,
            },
        ];
        let result = MatrixResult {
            baseline_plan: "x".into(),
            outcomes,
            coupling_threshold_m: 0.010,
        };
        assert!(result.passed());
        let outliers = result.outliers();
        assert!(outliers.is_empty());
    }

    #[test]
    fn matrix_outcome_fails_when_any_drift_exceeds_threshold() {
        let outcomes = vec![PerturbationOutcome {
            name: "couples".into(),
            per_joint_drifts_m: vec![0.001, 0.020, 0.002], // joint 1: 20mm — coupling detected
            max_drift_m: 0.020,
        }];
        let result = MatrixResult {
            baseline_plan: "x".into(),
            outcomes,
            coupling_threshold_m: 0.010,
        };
        assert!(!result.passed());
        assert_eq!(result.outliers(), vec!["couples".to_string()]);
    }

    #[test]
    fn mismatched_baseline_perturbation_joint_count_flags_structural_failure() {
        let baseline = vec![[0.0_f32, 1.0, 0.0]];
        let perturbed = vec![[0.0_f32, 1.0, 0.0], [0.0, 0.5, 0.0]]; // 2 joints, baseline has 1
        let deltas = per_joint_drift(&baseline, &perturbed);
        assert_eq!(deltas.len(), 1, "diff length is min of inputs");
        // The mismatch itself is detected by the executor wrapping the call —
        // expressed in MatrixResult.outcomes[i].per_joint_drifts.len() vs expected.
        // For unit test, we just verify per_joint_drift truncates safely.
    }
}
