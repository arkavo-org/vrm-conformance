# VRMC_springBone Phase 7 — VMK#162 Coupling Matrix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development.

**Goal:** Add an `execute-test-plan-matrix` runner mode that renders a baseline + N parameter-perturbed variants of the same plan, captures bone positions for each, and computes a position-delta matrix to detect VMK#162-style parameter-coupling regressions ("changing stiffness shifts the drag-tuned equilibrium").

**Architecture deviation from spec:** the original spec text described an in-memory perturbation matrix where the runner mutates spring params at runtime. That would require either an asset-generator subprocess shell-out OR a new adapter-side override op — both bigger than phase 7's budget. **Phase 7 uses pre-emitted asset variants:** the matrix YAML enumerates a baseline `.vrm` + N perturbation `.vrm` paths (each pre-generated via the existing sweep subcommands). The runner orchestrates N+1 renders + position dumps + delta computation.

This scope reduction keeps the deliverable focused on the runner infrastructure (the load-bearing piece) and lets users supply any perturbation strategy they want via the asset generator.

**Spec:** `docs/superpowers/specs/2026-05-15-springbone-conformance-closure-design.md` §9.

---

## File map

- `crates/vrm-test-plan/src/lib.rs` — new types: `CouplingMatrix`, `CouplingPerturbation`
- `crates/vrm-runner/src/execute_matrix.rs` (new) — matrix runner orchestrator
- `crates/vrm-runner/src/lib.rs` — `pub mod execute_matrix;`
- `crates/vrm-runner/src/cli.rs` — `ExecuteTestPlanMatrix` subcommand + describe entry
- `test-plans/manual/coupling/springbone_default_coupling.matrix.yaml` — example matrix YAML using existing sweep assets
- `docs/findings.md` — phase 7 entry

---

## Task 1: Coupling matrix schema in vrm-test-plan

**Files:** `crates/vrm-test-plan/src/lib.rs`

- [ ] **Step 1: Tests.** Add inline:

```rust
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
```

- [ ] **Step 2: Run, expect compile failure.**

- [ ] **Step 3: Add types.**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CouplingPerturbation {
    pub name: String,
    pub asset: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

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
```

Place near the other plan types. Don't import; if `serde` / `serde_yml` aren't already deps, they will be — confirm.

- [ ] **Step 4: Run + lint + commit:**
  ```
  cd /Users/arkavo/Projects/vrm-conformance && cargo test -p vrm-test-plan coupling_matrix
  cargo clippy -p vrm-test-plan --all-targets -- -D warnings
  cargo fmt -p vrm-test-plan -- --check
  git add crates/vrm-test-plan/src/lib.rs && git commit -m "feat(vrm-test-plan): CouplingMatrix schema for VMK#162 regression matrix"
  ```

---

## Task 2: Matrix executor module

**Files:** `crates/vrm-runner/src/execute_matrix.rs` (new), `crates/vrm-runner/src/lib.rs`

The orchestrator: load matrix YAML, run baseline + each perturbation through `execute_plan` with the perturbation's asset path overriding `plan.asset`, capture all dumps, compute per-perturbation drift vector (baseline_positions vs perturbation_positions), assert each per-joint drift ≤ threshold.

- [ ] **Step 1: Tests** in `execute_matrix.rs`:

```rust
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
        let outcomes = vec![
            PerturbationOutcome {
                name: "couples".into(),
                per_joint_drifts_m: vec![0.001, 0.020, 0.002], // joint 1: 20mm — coupling detected
                max_drift_m: 0.020,
            },
        ];
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
```

- [ ] **Step 2: Run, expect compile failure.**

- [ ] **Step 3: Implement.** New module:

```rust
//! Matrix execution: run a baseline + N perturbed plans, compute position drift
//! deltas, detect parameter-coupling regressions (VMK#162-class).

use crate::execute::{execute_plan, load_plan, ExecuteOptions};
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
        self.outcomes.iter().all(|o| o.max_drift_m <= self.coupling_threshold_m)
    }
    pub fn outliers(&self) -> Vec<String> {
        self.outcomes
            .iter()
            .filter(|o| o.max_drift_m > self.coupling_threshold_m)
            .map(|o| o.name.clone())
            .collect()
    }
}

pub fn per_joint_drift(a: &[[f32; 3]], b: &[[f32; 3]]) -> Vec<f32> {
    a.iter().zip(b.iter()).map(|(p, q)| {
        let dx = p[0] - q[0];
        let dy = p[1] - q[1];
        let dz = p[2] - q[2];
        (dx*dx + dy*dy + dz*dz).sqrt()
    }).collect()
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
    let baseline_positions = run_one_capture_positions(
        &base_plan,
        &matrix.baseline_asset,
        "baseline",
        opts,
    )?;

    // Run each perturbation.
    let mut outcomes = Vec::with_capacity(matrix.perturbations.len());
    for p in &matrix.perturbations {
        let perturbed_positions = run_one_capture_positions(
            &base_plan,
            &p.asset,
            &p.name,
            opts,
        )?;
        let baseline_joints = baseline_positions
            .springs
            .first()
            .map(|s| s.joint_positions.clone())
            .unwrap_or_default();
        let perturbed_joints = perturbed_positions
            .springs
            .first()
            .map(|s| s.joint_positions.clone())
            .unwrap_or_default();

        // Handle joint-count mismatch as structural failure (max_drift = INF).
        let drifts = if baseline_joints.len() != perturbed_joints.len() {
            vec![f32::INFINITY]
        } else {
            per_joint_drift(&baseline_joints, &perturbed_joints)
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
    // Clone the plan, override asset.
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
    };
    // execute_plan does not capture positions by itself unless reference_positions is set.
    // For matrix mode we need positions regardless, so call a lower-level path or
    // augment execute_plan to optionally return position_dump on demand.
    //
    // PRAGMATIC: extend execute_plan or execute.rs with a `capture_positions: bool`
    // option, or replicate the adapter loop here. Replicate is simpler — see below.
    crate::execute::execute_plan_capturing_positions(&plan, &exec_opts)
}
```

For `execute_plan_capturing_positions`: factor out from `execute.rs` so the existing `execute_plan` becomes a thin wrapper. The factored function always captures positions (returns `(ExecuteResult, Option<DumpBonePositionsResult>)`) — the caller decides whether to use them.

Actually simpler: add a public function in `crates/vrm-runner/src/execute.rs`:

```rust
pub fn execute_plan_capturing_positions(
    plan: &TestPlan,
    opts: &ExecuteOptions,
) -> Result<DumpBonePositionsResult> {
    // Spawn adapter, run the standard load_vrm → set_* → reset_physics → animate_root_transform
    // → render → dump_bone_positions → dispose pipeline. Return the dump.
    // (Skip diff/position_diff — that's not needed for matrix mode.)
    // Copy the body of execute_plan, but exit after the dump_bone_positions call.
}
```

Or refactor `execute_plan` to optionally return positions. Either is fine; pick whichever minimizes churn.

- [ ] **Step 4: Wire module into lib.rs.**

```rust
// crates/vrm-runner/src/lib.rs
pub mod execute_matrix;
```

- [ ] **Step 5: Run + lint + commit:**
  ```
  cd /Users/arkavo/Projects/vrm-conformance && cargo test -p vrm-runner execute_matrix
  cargo clippy -p vrm-runner --all-targets -- -D warnings
  cargo fmt -p vrm-runner -- --check
  git add crates/vrm-runner/ && git commit -m "feat(vrm-runner): execute_matrix module for VMK#162 coupling regression"
  ```

---

## Task 3: CLI subcommand `execute-test-plan-matrix`

**Files:** `crates/vrm-runner/src/cli.rs`, `crates/vrm-runner/src/main.rs` if needed

- [ ] **Step 1: Add subcommand variant** to the runner's CLI enum, parallel to `ExecuteTestPlan`:

```rust
ExecuteTestPlanMatrix {
    #[arg(long, value_name = "PATH")]
    matrix: Utf8PathBuf,
    #[arg(long, value_name = "PATH")]
    adapter_bin: Utf8PathBuf,
    #[arg(long, value_name = "ARG", action = ArgAction::Append)]
    adapter_args: Vec<String>,
    #[arg(long, value_name = "DIR")]
    asset_dir: Utf8PathBuf,
    #[arg(long, value_name = "DIR")]
    output_dir: Utf8PathBuf,
    #[arg(long, value_name = "NAME")]
    renderer_name: String,
    #[arg(long)]
    json: bool,
},
```

- [ ] **Step 2: Dispatch handler** that loads the matrix, calls `execute_matrix`, prints the JSON result. Mirror `ExecuteTestPlan`'s JSON output shape:

```json
{
  "ok": true,
  "matrix_path": "...",
  "baseline_plan": "springbone_default.test.yaml",
  "coupling_threshold_m": 0.015,
  "outcomes": [
    {"name": "stiffness_high", "per_joint_drifts_m": [...], "max_drift_m": 0.0042},
    ...
  ],
  "outliers": ["couples"],
  "overall_passed": false
}
```

- [ ] **Step 3:** Update the `Describe` catalog to include the new subcommand with its full input/output schema. CLAUDE.md treats the describe surface as load-bearing.

- [ ] **Step 4: Commit:**
  ```
  git add crates/vrm-runner/ && git commit -m "feat(vrm-runner): execute-test-plan-matrix CLI subcommand"
  ```

---

## Task 4: Example matrix YAML

**Files:** `test-plans/manual/coupling/springbone_default_coupling.matrix.yaml`

- [ ] **Step 1: Verify the assets exist.** The existing `emit-springbone-sweep` produces `springbone_stiffness_*.vrm` variants. Confirm filenames:

```
ls assets/generated/ 2>&1 | grep -E "springbone_stiffness|springbone_default|springbone_drag" | head -10
```

If `assets/generated/` doesn't have them committed, you may need to regenerate with `cargo run -p vrm-asset-generator --release -- emit-springbone-sweep --output-dir assets/generated/sweep` — but be careful not to bloat the commit. Prefer to write the matrix YAML referring to paths that WILL exist after `emit-springbone-sweep` is run, and document that the user must run the sweep first.

- [ ] **Step 2: Write `test-plans/manual/coupling/springbone_default_coupling.matrix.yaml`:**

```yaml
# Phase 7 VMK#162 coupling regression matrix.
# Run: `cargo run -p vrm-asset-generator --release -- emit-springbone-sweep \
#       --output-dir assets/generated/sweep`  first to populate the perturbation assets.
base_plan: ../../../assets/generated/sweep/springbone_default.test.yaml
baseline_asset: springbone_default.vrm
perturbations:
  - name: stiffness_high
    asset: springbone_stiffness_0p8.vrm
    description: stiffness baseline 0.5 → 0.8 (+0.3 = +60% relative)
  - name: stiffness_low
    asset: springbone_stiffness_0p2.vrm
    description: stiffness baseline 0.5 → 0.2 (-0.3 = -60% relative)
  - name: drag_high
    asset: springbone_drag_0p8.vrm
    description: drag baseline 0.5 → 0.8
  - name: drag_low
    asset: springbone_drag_0p2.vrm
    description: drag baseline 0.5 → 0.2
  - name: gravity_high
    asset: springbone_gravity_2.vrm
    description: gravity_power baseline 0.5 → 2.0
  - name: gravity_low
    asset: springbone_gravity_0.vrm
    description: gravity_power baseline 0.5 → 0.0
coupling_threshold_m: 0.015
```

(Perturbations use the existing sweep's available variant filenames — these are wider than ±10% because that's what the sweep emits. Document this in the comment.)

- [ ] **Step 3:** Test the matrix end-to-end against the mock renderer:

```
cd /Users/arkavo/Projects/vrm-conformance
# Ensure assets exist:
cargo run -p vrm-asset-generator --release -- emit-springbone-sweep --output-dir assets/generated/sweep 2>&1 | tail -3
# Run the matrix:
cargo run -p vrm-runner --release -- execute-test-plan-matrix \
  --matrix test-plans/manual/coupling/springbone_default_coupling.matrix.yaml \
  --adapter-bin target/release/vrm-mock-renderer \
  --asset-dir assets/generated/sweep \
  --output-dir /tmp/phase7-matrix-out \
  --renderer-name mock \
  --json 2>&1 | python3 -m json.tool
```

Expected: `ok: true`, all perturbations produce empty position arrays (mock has no springs), `max_drift_m` is 0 for all, `overall_passed: true`. The smoke verifies wiring.

- [ ] **Step 4:** Commit:
  ```
  git add test-plans/manual/coupling/ && git commit -m "test(coupling): VMK#162 sample matrix YAML over springbone_default sweep"
  ```

---

## Task 5: docs/findings.md phase-7 entry

```markdown
## Phase 7 — VMK#162 coupling matrix runner landed

**Trigger:** Phase 6 multi-chain merged. Final phase: the runner gains `execute-test-plan-matrix`, enabling self-comparison regressions of the form "changing one tuned parameter should not silently shift the equilibrium that other parameters establish" (VMK#162).

**Architecture deviation from spec:** the spec proposed runtime parameter mutation. Phase 7 ships pre-emitted asset variants instead — the matrix YAML enumerates a baseline `.vrm` + N perturbation `.vrm` paths, runner orchestrates N+1 renders + position dumps + per-joint delta computation. This sidesteps the need for an adapter-side `override_spring_params` op.

**Shipped:**
- `crates/vrm-test-plan/src/lib.rs`: `CouplingMatrix` + `CouplingPerturbation` types.
- `crates/vrm-runner/src/execute_matrix.rs`: orchestrator, `per_joint_drift`, `MatrixResult::passed()`/`outliers()`.
- `vrm-runner execute-test-plan-matrix` subcommand with full describe catalog entry.
- `test-plans/manual/coupling/springbone_default_coupling.matrix.yaml`: example matrix using existing emit-springbone-sweep variants.
- Smoke-tested through mock renderer end-to-end.

**Calibration deferred:** the example matrix uses `coupling_threshold_m: 0.015` as an opening guess. Real calibration requires running the matrix on three-vrm and godot-vrm (well-behaved baselines), observing their max coupling drift, and tuning the threshold above their max but below VMK's reported coupling magnitude. That measurement run is a separate manual step — not blocking infrastructure delivery.

**Forward:** the seven-phase VRMC_springBone gap closure is complete. The corpus across phases 2–6 ships 142 new test plans:
- Phase 2: 48 collider plans
- Phase 3: 36 extended-collider plans
- Phase 4: 8 gravityDir plans
- Phase 5: 14 per-joint taper plans
- Phase 6: 36 multi-chain plans
- Phase 7: 1 example coupling matrix YAML (calibration matrix)

Next: bootstrap-goldens on the new corpus and update `goldens/manifest.json`.
```

- [ ] Commit: `git add docs/findings.md && git commit -m "docs(findings): phase 7 coupling matrix landed (seven-phase springbone closure complete)"`

---

## Acceptance

- [ ] All tests green, clippy + fmt clean
- [ ] `execute-test-plan-matrix --matrix <yaml> --adapter-bin <mock> ...` runs end-to-end against mock and returns `overall_passed: true` with empty drift vectors
- [ ] Describe catalog includes the new subcommand
- [ ] Example matrix YAML committed
- [ ] findings.md entry present
