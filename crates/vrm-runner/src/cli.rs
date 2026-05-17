use crate::execute::{execute_plan, load_plan, ExecuteOptions};
use crate::execute_matrix::{execute_matrix, load_matrix, ExecuteMatrixOptions};
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use serde_json::json;

#[derive(Debug, Parser)]
#[command(version, about = "VRM conformance runner")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Cmd,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Execute a test plan against one renderer adapter.
    ExecuteTestPlan {
        #[arg(long)]
        plan: Utf8PathBuf,
        #[arg(long)]
        adapter_bin: Utf8PathBuf,
        #[arg(long, value_delimiter = ' ', num_args = 0..)]
        adapter_args: Vec<String>,
        #[arg(long)]
        asset_dir: Utf8PathBuf,
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long, default_value = "vrm-metal-kit")]
        renderer_name: String,
        /// Optional reference PNG; when set, the runner diffs the produced
        /// render against it and includes a DiffResult in the JSON summary.
        #[arg(long)]
        reference: Option<Utf8PathBuf>,
        /// Optional reference positions JSON (same shape as
        /// `DumpBonePositionsResult`); when set, the runner calls
        /// `dump_bone_positions` after render and includes a
        /// `position_diff` block in the JSON summary.
        #[arg(long, value_name = "PATH")]
        reference_positions: Option<Utf8PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Cost-preview a test plan without executing.
    PlanTestPlan {
        #[arg(long)]
        plan: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Diff a render PNG against a reference PNG using a test plan.
    Diff {
        #[arg(long)]
        plan: Utf8PathBuf,
        #[arg(long)]
        render: Utf8PathBuf,
        #[arg(long)]
        reference: Utf8PathBuf,
        #[arg(long, default_value = "vrm-metal-kit")]
        renderer_name: String,
        #[arg(long)]
        json: bool,
    },
    /// Run N-way consensus diff over renders from multiple renderers.
    /// Each `--render <name>=<png>` pair contributes one renderer to the
    /// consensus pool. The test plan's `diff.threshold` is used unless
    /// overridden with `--threshold`. Exits non-zero when any renderer
    /// is flagged as an outlier (i.e., disagrees with one or more peers
    /// at the SSIM threshold).
    ConsensusDiff {
        #[arg(long)]
        plan: Utf8PathBuf,
        /// One `name=path` pair per renderer. Repeat the flag. The same
        /// PNG dimensions are required across all renderers.
        #[arg(long = "render", value_parser = parse_named_path)]
        renders: Vec<(String, Utf8PathBuf)>,
        /// Override the plan's `diff.threshold`. Optional.
        #[arg(long)]
        threshold: Option<f32>,
        /// One `name=path` pair per renderer pointing to its
        /// `DumpBonePositionsResult` JSON file. Repeat the flag. When
        /// provided alongside `--render`, position consensus is also
        /// computed and included in the JSON output.
        #[arg(long = "render-positions", value_parser = parse_named_path)]
        render_positions: Vec<(String, Utf8PathBuf)>,
        /// Override the default 1 cm outlier threshold for position
        /// consensus. A renderer whose mean pairwise drift vs all peers
        /// exceeds the median renderer's mean by this much (in metres)
        /// is flagged as an outlier.
        #[arg(long, default_value_t = 0.010)]
        position_outlier_threshold_m: f32,
        #[arg(long)]
        json: bool,
    },
    /// Execute a batched corpus through a batch-mode adapter (UniVRM).
    /// Mirrors `ExecuteTestPlan` shape but takes a directory of plans
    /// and invokes the adapter once for the whole batch. See
    /// `docs/superpowers/specs/2026-05-12-adapter-univrm-design.md`.
    ExecuteTestBatch {
        /// Directory containing `*.test.yaml` test plans. Each plan is
        /// paired with its sibling `.vrm` (same stem).
        #[arg(long)]
        plans: Utf8PathBuf,
        /// Path to the adapter launcher (e.g.
        /// `adapters/univrm/launcher.sh` for real Unity, or a mock
        /// fixture for tests).
        #[arg(long)]
        adapter_bin: Utf8PathBuf,
        /// Directory where rendered PNGs and the per-renderer local
        /// manifest are written.
        #[arg(long)]
        output_dir: Utf8PathBuf,
        /// Renderer name recorded in the local manifest.
        #[arg(long, default_value = "univrm")]
        renderer_name: String,
        /// Emit JSON summary to stdout.
        #[arg(long)]
        json: bool,
    },
    /// Execute a coupling matrix: render a baseline + N parameter-perturbed
    /// variants of the same plan, capture bone positions for each, and compute
    /// per-joint position-delta vectors to detect VMK#162-style coupling
    /// regressions.
    ExecuteTestPlanMatrix {
        #[arg(long, value_name = "PATH")]
        matrix: Utf8PathBuf,
        #[arg(long, value_name = "PATH")]
        adapter_bin: Utf8PathBuf,
        #[arg(long, value_name = "ARG", action = ArgAction::Append, default_value = None)]
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
    /// Print the operation catalog.
    Describe {
        #[arg(long, value_enum, default_value_t = DescribeFormat::Json)]
        format: DescribeFormat,
    },
}

/// Parse one `name=path` value for the consensus-diff --render flag.
fn parse_named_path(s: &str) -> Result<(String, Utf8PathBuf), String> {
    let (name, path) = s
        .split_once('=')
        .ok_or_else(|| format!("expected name=path, got '{s}' (missing '=' separator)"))?;
    if name.is_empty() {
        return Err(format!("empty renderer name in '{s}'"));
    }
    Ok((name.into(), Utf8PathBuf::from(path)))
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DescribeFormat {
    Json,
    Text,
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Cmd::ExecuteTestPlan {
            plan,
            adapter_bin,
            adapter_args,
            asset_dir,
            output_dir,
            renderer_name,
            reference,
            reference_positions,
            json: emit_json,
        } => {
            let plan_value = load_plan(&plan)?;
            let opts = ExecuteOptions {
                adapter_bin,
                adapter_args,
                asset_dir,
                output_dir,
                renderer_name,
                emit_progress_ndjson: emit_json,
                reference,
                reference_positions,
                vrma_path: None,
                apply_at_time: 0.0,
                reference_pose_json: None,
            };
            let result = execute_plan(&plan_value, &opts)?;
            if emit_json {
                let overall_passed = match (&result.diff, &result.position_diff) {
                    (Some(d), Some(p)) => d.overall_passed() && p.passed,
                    (Some(d), None) => d.overall_passed(),
                    (None, Some(p)) => p.passed,
                    (None, None) => true,
                };
                let mut summary = json!({
                    "ok": true,
                    "test_id": result.test_id,
                    "renderer": result.renderer,
                    "output_png": result.output_png,
                    "actual_color_space": format!("{:?}", result.actual_color_space),
                    "overall_passed": overall_passed,
                });
                if let Some(diff) = &result.diff {
                    summary["diff"] = serde_json::to_value(diff)?;
                }
                if let Some(pos_diff) = &result.position_diff {
                    summary["position_diff"] = serde_json::to_value(pos_diff)?;
                }
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!("rendered {} → {}", result.test_id, result.output_png);
                if let Some(diff) = &result.diff {
                    println!(
                        "  diff: SSIM={:.4} ({}), overall {}",
                        diff.ssim,
                        if diff.ssim_passed { "PASS" } else { "FAIL" },
                        if diff.overall_passed() {
                            "PASS"
                        } else {
                            "FAIL"
                        }
                    );
                }
                if let Some(pos_diff) = &result.position_diff {
                    println!(
                        "  position_diff: per_joint_max={:.4}m chain_summed={:.4}m ({})",
                        pos_diff.per_joint_max_drift_m,
                        pos_diff.chain_summed_drift_m,
                        if pos_diff.passed { "PASS" } else { "FAIL" }
                    );
                }
            }
            Ok(())
        }
        Cmd::PlanTestPlan {
            plan,
            json: emit_json,
        } => {
            let p = load_plan(&plan)?;
            // v0.1 trivial estimate: one render.
            let preview = json!({
                "ok": true,
                "test_id": p.id,
                "estimated_renders": 1,
                "estimated_seconds": 4.0,
                "outputs": [
                    format!("{}_{{renderer}}.png", p.id)
                ]
            });
            if emit_json {
                println!("{}", serde_json::to_string(&preview)?);
            } else {
                println!("would render: {}", p.id);
            }
            Ok(())
        }
        Cmd::Diff {
            plan,
            render,
            reference,
            renderer_name,
            json: emit_json,
        } => {
            use crate::diff::diff_one;

            let plan_value = load_plan(&plan)?;
            let result = diff_one(&plan_value, &render, &reference, &renderer_name)?;
            let passed = result.overall_passed();

            if emit_json {
                println!("{}", serde_json::to_string(&result)?);
            } else {
                println!(
                    "{}: SSIM={:.4} (threshold {:.4}, {}), {} property assertion(s) {}",
                    result.test_id,
                    result.ssim,
                    result.ssim_threshold,
                    if result.ssim_passed { "PASS" } else { "FAIL" },
                    result.properties.len(),
                    if result.properties.iter().all(|p| p.passed) {
                        "PASS"
                    } else {
                        "FAIL"
                    },
                );
            }

            if !passed {
                std::process::exit(1);
            }
            Ok(())
        }
        Cmd::ConsensusDiff {
            plan,
            renders,
            threshold,
            render_positions,
            position_outlier_threshold_m,
            json: emit_json,
        } => {
            use vrm_diff_engine::consensus::{consensus_diff, position_consensus, RendererRender};
            use vrm_ops::tools::DumpBonePositionsResult;

            let plan_value = load_plan(&plan)?;
            let effective_threshold = threshold.unwrap_or(plan_value.diff.threshold);

            let render_refs: Vec<RendererRender<'_>> = renders
                .iter()
                .map(|(name, path)| RendererRender {
                    name: name.clone(),
                    png_path: path.as_path(),
                })
                .collect();

            let result = if !render_refs.is_empty() {
                Some(consensus_diff(
                    &plan_value.id,
                    &render_refs,
                    effective_threshold,
                )?)
            } else {
                None
            };

            // Position consensus — only when positions were supplied.
            let pos_result = if !render_positions.is_empty() {
                let mut entries: Vec<(String, vrm_ops::tools::SpringPositions)> =
                    Vec::with_capacity(render_positions.len());
                for (name, path) in &render_positions {
                    let raw = std::fs::read_to_string(path).map_err(|e| {
                        anyhow::anyhow!("failed to read positions file {path}: {e}")
                    })?;
                    let dump: DumpBonePositionsResult =
                        serde_json::from_str(&raw).map_err(|e| {
                            anyhow::anyhow!("failed to parse positions JSON {path}: {e}")
                        })?;
                    // Phase 1: first spring only.
                    if let Some(first) = dump.springs.into_iter().next() {
                        entries.push((name.clone(), first));
                    }
                }
                Some(position_consensus(&entries, position_outlier_threshold_m))
            } else {
                None
            };

            let overall_passed = match (&result, &pos_result) {
                (Some(r), Some(p)) => r.consensus_passed && p.outliers.is_empty(),
                (Some(r), None) => r.consensus_passed,
                (None, Some(p)) => p.outliers.is_empty(),
                (None, None) => true,
            };

            if emit_json {
                let mut out = serde_json::Map::new();
                if let Some(ref r) = result {
                    // Flatten the ConsensusResult fields into the top-level output
                    // (preserving backward-compat with callers that already parse this).
                    if let serde_json::Value::Object(map) = serde_json::to_value(r)? {
                        for (k, v) in map {
                            out.insert(k, v);
                        }
                    }
                }
                if let Some(ref p) = pos_result {
                    out.insert("position_consensus".into(), serde_json::to_value(p)?);
                }
                out.insert("overall_passed".into(), overall_passed.into());
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::Value::Object(out))?
                );
            } else {
                if let Some(ref r) = result {
                    println!(
                        "{}: {} renderer(s), threshold={:.4}",
                        r.test_id,
                        r.renderers.len(),
                        r.threshold,
                    );
                    for (i, name) in r.renderers.iter().enumerate() {
                        println!(
                            "  {name}: agreement={}/{}",
                            r.agreement_count[i],
                            r.renderers.len() - 1
                        );
                    }
                    if r.consensus_passed {
                        println!("  consensus: PASS");
                    } else {
                        println!("  consensus: FAIL  outliers={:?}", r.outliers);
                    }
                }
                if let Some(ref p) = pos_result {
                    println!(
                        "  position consensus: mean_drift={:.4}m threshold={:.4}m",
                        p.mean_pairwise_drift_m, p.outlier_threshold_m,
                    );
                    if p.outliers.is_empty() {
                        println!("  position consensus: PASS");
                    } else {
                        println!("  position consensus: FAIL  outliers={:?}", p.outliers);
                    }
                }
            }

            if !overall_passed {
                std::process::exit(1);
            }
            Ok(())
        }
        Cmd::ExecuteTestBatch {
            plans,
            adapter_bin,
            output_dir,
            renderer_name,
            json: emit_json,
        } => {
            let opts = crate::execute_batch::RunOptions {
                plans_dir: plans,
                adapter_bin,
                output_dir,
                renderer_name,
            };
            let summary = crate::execute_batch::run(&opts)?;
            if emit_json {
                let payload = json!({
                    "ok": summary.error_count == 0,
                    "total_tests": summary.total_tests,
                    "ok_count": summary.ok_count,
                    "error_count": summary.error_count,
                    "local_manifest": summary.local_manifest_path,
                });
                println!("{}", serde_json::to_string(&payload)?);
            } else {
                println!(
                    "batched {} tests: {} ok, {} error → {}",
                    summary.total_tests,
                    summary.ok_count,
                    summary.error_count,
                    summary.local_manifest_path
                );
            }
            Ok(())
        }
        Cmd::ExecuteTestPlanMatrix {
            matrix: matrix_path,
            adapter_bin,
            adapter_args,
            asset_dir,
            output_dir,
            renderer_name,
            json: emit_json,
        } => {
            let matrix = load_matrix(&matrix_path)?;
            let opts = ExecuteMatrixOptions {
                adapter_bin,
                adapter_args,
                asset_dir,
                output_dir,
                renderer_name,
                emit_progress_ndjson: emit_json,
            };
            let result = execute_matrix(&matrix, &matrix_path, &opts)?;
            let passed = result.passed();
            let outliers = result.outliers();

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "matrix_path": matrix_path.as_str(),
                    "baseline_plan": result.baseline_plan,
                    "coupling_threshold_m": result.coupling_threshold_m,
                    "outcomes": result.outcomes,
                    "outliers": outliers,
                    "overall_passed": passed,
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "matrix {matrix_path}: {} perturbation(s), threshold={:.4}m",
                    result.outcomes.len(),
                    result.coupling_threshold_m,
                );
                for o in &result.outcomes {
                    println!(
                        "  {}: max_drift={:.4}m ({})",
                        o.name,
                        o.max_drift_m,
                        if o.max_drift_m <= result.coupling_threshold_m {
                            "PASS"
                        } else {
                            "FAIL"
                        }
                    );
                }
                println!("  overall: {}", if passed { "PASS" } else { "FAIL" });
                if !outliers.is_empty() {
                    println!("  coupling outliers: {outliers:?}");
                }
            }
            Ok(())
        }
        Cmd::Describe { format } => {
            let catalog = json!({
                "name": "vrm-runner",
                "version": env!("CARGO_PKG_VERSION"),
                "operations": {
                    "execute-test-plan": {
                        "summary": "Execute a YAML test plan against one renderer adapter; optionally diff against a reference PNG",
                        "input_schema": {
                            "type": "object",
                            "required": ["plan", "adapter_bin", "asset_dir", "output_dir"],
                            "properties": {
                                "plan": { "type": "string" },
                                "adapter_bin": { "type": "string" },
                                "adapter_args": { "type": "array", "items": { "type": "string" } },
                                "asset_dir": { "type": "string" },
                                "output_dir": { "type": "string" },
                                "reference": { "type": "string" },
                                "reference_positions": { "type": "string", "description": "Path to a positions JSON file (DumpBonePositionsResult shape) to diff dumped bone positions against." }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "test_id": { "type": "string" },
                                "renderer": { "type": "string" },
                                "output_png": { "type": "string" },
                                "actual_color_space": { "type": "string" },
                                "diff": { "type": ["object", "null"] },
                                "position_diff": { "type": ["object", "null"], "description": "Present only when reference_positions was provided. Two-threshold position-space diff report (see vrm_diff_engine::positions::PositionDiffReport).", "properties": { "per_joint_max_drift_m": { "type": "number" }, "chain_summed_drift_m": { "type": "number" }, "per_joint_tolerance_m": { "type": "number" }, "chain_max_drift_m": { "type": "number" }, "worst_joint_index": { "type": "integer" }, "passed": { "type": "boolean" } } },
                                "overall_passed": { "type": "boolean" }
                            }
                        }
                    },
                    "plan-test-plan": {
                        "summary": "Cost-preview a test plan without executing",
                        "input_schema": {
                            "type": "object",
                            "required": ["plan"],
                            "properties": {
                                "plan": { "type": "string" }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "test_id": { "type": "string" },
                                "estimated_renders": { "type": "integer" },
                                "estimated_seconds": { "type": "number" },
                                "outputs": { "type": "array", "items": { "type": "string" } }
                            }
                        }
                    },
                    "diff": {
                        "summary": "Diff a render PNG against a reference PNG using a test plan; emit DiffResult JSON. Exit non-zero when overall_passed is false.",
                        "input_schema": {
                            "type": "object",
                            "required": ["plan", "render", "reference"],
                            "properties": {
                                "plan": { "type": "string" },
                                "render": { "type": "string" },
                                "reference": { "type": "string" },
                                "renderer_name": { "type": "string" }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "test_id": { "type": "string" },
                                "renderer": { "type": "string" },
                                "reference_renderer": { "type": "string" },
                                "ssim": { "type": "number" },
                                "ssim_threshold": { "type": "number" },
                                "ssim_passed": { "type": "boolean" },
                                "properties": { "type": "array" }
                            }
                        }
                    },
                    "consensus-diff": {
                        "summary": "N-way cross-renderer consensus diff. Each --render name=path contributes one renderer to the SSIM pool; --render-positions name=path adds per-renderer spring-bone position JSON for position consensus. Outputs the full SSIM matrix, per-renderer agreement counts, outlier names, and (when positions supplied) a position_consensus block. Exit non-zero when any renderer is an outlier.",
                        "input_schema": {
                            "type": "object",
                            "required": ["plan"],
                            "properties": {
                                "plan": { "type": "string" },
                                "renders": {
                                    "type": "array",
                                    "items": { "type": "string", "description": "name=path" },
                                    "minItems": 2
                                },
                                "threshold": { "type": "number" },
                                "render_positions": {
                                    "type": "array",
                                    "items": { "type": "string", "description": "name=path to DumpBonePositionsResult JSON" },
                                    "description": "Per-renderer spring-bone position files for N-way position consensus. Phase 1: first spring chain only."
                                },
                                "position_outlier_threshold_m": {
                                    "type": "number",
                                    "default": 0.010,
                                    "description": "A renderer whose mean pairwise drift vs all peers exceeds the median by this amount (metres) is flagged as a position outlier."
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "test_id": { "type": "string" },
                                "threshold": { "type": "number" },
                                "renderers": { "type": "array", "items": { "type": "string" } },
                                "ssim_matrix": {
                                    "type": "array",
                                    "items": { "type": "array", "items": { "type": "number" } }
                                },
                                "agreement_count": { "type": "array", "items": { "type": "integer" } },
                                "outliers": { "type": "array", "items": { "type": "string" } },
                                "consensus_passed": { "type": "boolean" },
                                "position_consensus": {
                                    "type": "object",
                                    "description": "Present only when --render-positions was supplied.",
                                    "properties": {
                                        "mean_pairwise_drift_m": { "type": "number" },
                                        "outliers": { "type": "array", "items": { "type": "string" } },
                                        "outlier_threshold_m": { "type": "number" }
                                    }
                                },
                                "overall_passed": { "type": "boolean" }
                            }
                        }
                    },
                    "execute-test-batch": {
                        "summary": "Execute a batched corpus through a batch-mode adapter (UniVRM). Builds a JSON manifest, invokes the adapter once for the whole batch, ingests an NDJSON results file.",
                        "input_schema": {
                            "type": "object",
                            "required": ["plans", "adapter_bin", "output_dir"],
                            "properties": {
                                "plans": {
                                    "type": "string",
                                    "description": "directory of .test.yaml files"
                                },
                                "adapter_bin": {
                                    "type": "string",
                                    "description": "path to the adapter launcher"
                                },
                                "output_dir": {
                                    "type": "string",
                                    "description": "PNG and local-manifest output directory"
                                },
                                "renderer_name": {
                                    "type": "string",
                                    "default": "univrm",
                                    "description": "renderer name recorded in local manifest"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "total_tests": { "type": "integer" },
                                "ok_count": { "type": "integer" },
                                "error_count": { "type": "integer" },
                                "local_manifest": {
                                    "type": "string",
                                    "description": "path to local-manifest.json"
                                }
                            }
                        }
                    },
                    "execute-test-plan-matrix": {
                        "summary": "Render a baseline + N parameter-perturbed VRM variants through the same test plan, capturing bone positions for each, then compute per-joint position-delta vectors to detect VMK#162-style parameter-coupling regressions.",
                        "input_schema": {
                            "type": "object",
                            "required": ["matrix", "adapter_bin", "asset_dir", "output_dir", "renderer_name"],
                            "properties": {
                                "matrix": {
                                    "type": "string",
                                    "description": "path to a CouplingMatrix YAML file (base_plan + baseline_asset + perturbations + coupling_threshold_m)"
                                },
                                "adapter_bin": { "type": "string" },
                                "adapter_args": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "extra arguments forwarded to the adapter binary"
                                },
                                "asset_dir": {
                                    "type": "string",
                                    "description": "directory containing the baseline and perturbation VRM files"
                                },
                                "output_dir": {
                                    "type": "string",
                                    "description": "directory for rendered PNGs (one per variant)"
                                },
                                "renderer_name": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "emit structured JSON to stdout"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "matrix_path": { "type": "string" },
                                "baseline_plan": { "type": "string" },
                                "coupling_threshold_m": { "type": "number" },
                                "outcomes": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "name": { "type": "string" },
                                            "per_joint_drifts_m": {
                                                "type": "array",
                                                "items": { "type": "number" },
                                                "description": "per-joint Euclidean distance (metres) between baseline and perturbation positions"
                                            },
                                            "max_drift_m": {
                                                "type": "number",
                                                "description": "max of per_joint_drifts_m; compared against coupling_threshold_m to pass/fail"
                                            }
                                        }
                                    }
                                },
                                "outliers": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "names of perturbations whose max_drift_m exceeded coupling_threshold_m"
                                },
                                "overall_passed": { "type": "boolean" }
                            }
                        }
                    }
                }
            });
            match format {
                DescribeFormat::Json => println!("{}", serde_json::to_string_pretty(&catalog)?),
                DescribeFormat::Text => println!("{catalog:#?}"),
            }
            Ok(())
        }
    }
}
