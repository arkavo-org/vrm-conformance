use crate::execute::{execute_plan, load_plan, ExecuteOptions};
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
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
            };
            let result = execute_plan(&plan_value, &opts)?;
            if emit_json {
                let mut summary = json!({
                    "ok": true,
                    "test_id": result.test_id,
                    "renderer": result.renderer,
                    "output_png": result.output_png,
                    "actual_color_space": format!("{:?}", result.actual_color_space)
                });
                if let Some(diff) = &result.diff {
                    summary["diff"] = serde_json::to_value(diff)?;
                    summary["overall_passed"] = serde_json::Value::Bool(diff.overall_passed());
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
            json: emit_json,
        } => {
            use vrm_diff_engine::consensus::{consensus_diff, RendererRender};

            let plan_value = load_plan(&plan)?;
            let effective_threshold = threshold.unwrap_or(plan_value.diff.threshold);

            let render_refs: Vec<RendererRender<'_>> = renders
                .iter()
                .map(|(name, path)| RendererRender {
                    name: name.clone(),
                    png_path: path.as_path(),
                })
                .collect();

            let result = consensus_diff(&plan_value.id, &render_refs, effective_threshold)?;

            if emit_json {
                println!("{}", serde_json::to_string(&result)?);
            } else {
                println!(
                    "{}: {} renderer(s), threshold={:.4}",
                    result.test_id,
                    result.renderers.len(),
                    result.threshold,
                );
                for (i, name) in result.renderers.iter().enumerate() {
                    println!(
                        "  {name}: agreement={}/{}",
                        result.agreement_count[i],
                        result.renderers.len() - 1
                    );
                }
                if result.consensus_passed {
                    println!("  consensus: PASS");
                } else {
                    println!("  consensus: FAIL  outliers={:?}", result.outliers);
                }
            }

            if !result.consensus_passed {
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
                                "reference": { "type": "string" }
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
                        "summary": "N-way cross-renderer consensus diff. Each --render name=path contributes one renderer to the pool; outputs the full SSIM matrix, per-renderer agreement counts, and outlier names. Exit non-zero when any renderer is an outlier (disagrees with a peer at threshold).",
                        "input_schema": {
                            "type": "object",
                            "required": ["plan", "renders"],
                            "properties": {
                                "plan": { "type": "string" },
                                "renders": {
                                    "type": "array",
                                    "items": { "type": "string", "description": "name=path" },
                                    "minItems": 2
                                },
                                "threshold": { "type": "number" }
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
                                "consensus_passed": { "type": "boolean" }
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
