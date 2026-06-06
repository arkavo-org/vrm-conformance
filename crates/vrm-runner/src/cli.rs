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
        /// Optional `.vrma` file path. When set, the runner calls
        /// `load_vrma` and `apply_vrma_at_time(--apply-at-time)` before
        /// render, then dumps humanoid pose + expression weights +
        /// lookAt state to `<output_dir>/<id>_<renderer>.pose.json`.
        #[arg(long = "vrma", value_name = "PATH")]
        vrma_path: Option<Utf8PathBuf>,
        /// Sample time (seconds) for `apply_vrma_at_time`. Ignored when
        /// `--vrma` is not set.
        #[arg(long = "apply-at-time", default_value_t = 0.0)]
        apply_at_time: f32,
        /// Optional reference pose JSON fixture (with `humanoid`,
        /// `expressions`, `look_at` top-level keys matching the runner's
        /// pose.json shape). When set + `--vrma` set, the runner runs
        /// pose_diff and includes the report in JSON output.
        #[arg(long = "reference-pose-json", value_name = "PATH")]
        reference_pose_json: Option<Utf8PathBuf>,
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
    /// Diff a render against a reference using a test plan.
    ///
    /// Single-frame mode (legacy): `--render <png> --reference <png>`.
    /// Sequence mode: `--render-frames <dir> --reference-frames <dir>` —
    ///   the dir contains per-frame PNGs (e.g. `0000.png`, `0001.png`, …)
    ///   in sorted-filename order. Mutually exclusive with single-frame
    ///   flags. The plan's `render_sequence.temporal_ssim_threshold`
    ///   (when set) overrides `diff.threshold` for the sequence diff
    ///   pass criterion.
    Diff {
        #[arg(long)]
        plan: Utf8PathBuf,
        /// Single-frame candidate PNG. Mutually exclusive with --render-frames.
        #[arg(long, conflicts_with = "render_frames")]
        render: Option<Utf8PathBuf>,
        /// Single-frame reference PNG. Mutually exclusive with --reference-frames.
        #[arg(long, conflicts_with = "reference_frames")]
        reference: Option<Utf8PathBuf>,
        /// Sequence-mode candidate frames directory. PNG files inside are
        /// loaded in lexicographic order (use zero-padded names like
        /// 0000.png, 0001.png). Mutually exclusive with --render.
        #[arg(long, requires = "reference_frames")]
        render_frames: Option<Utf8PathBuf>,
        /// Sequence-mode reference frames directory.
        #[arg(long, requires = "render_frames")]
        reference_frames: Option<Utf8PathBuf>,
        #[arg(long, default_value = "vrm-metal-kit")]
        renderer_name: String,
        #[arg(long)]
        json: bool,
    },
    /// Cross-variant SSIM: assert the false-variant and true-variant renders of
    /// the SAME renderer DIFFER. Reads `max_ssim` from the plan's `cross_variant`
    /// block. Exits non-zero when the renders do NOT diverge (ssim > max_ssim).
    /// Used by the doubleSided back-face-culling spec test.
    CrossVariantDiff {
        #[arg(long)]
        plan: Utf8PathBuf,
        #[arg(long)]
        render_false: Utf8PathBuf,
        #[arg(long)]
        render_true: Utf8PathBuf,
        #[arg(long, default_value = "univrm")]
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
    ///
    /// Sequence mode: use `--render-frames <name>=<dir>` instead of
    /// `--render`. Each directory must contain PNG frames in lexicographic
    /// order (e.g. `0000.png`, `0001.png`, …). `--render` and
    /// `--render-frames` are mutually exclusive (enforced at runtime).
    ConsensusDiff {
        #[arg(long)]
        plan: Utf8PathBuf,
        /// One `name=path` pair per renderer. Repeat the flag. The same
        /// PNG dimensions are required across all renderers.
        #[arg(long = "render", value_parser = parse_named_path)]
        renders: Vec<(String, Utf8PathBuf)>,
        /// Sequence mode: one `name=dir` pair per renderer (path = directory
        /// of PNG frames in lexicographic order). Mutually exclusive with
        /// `--render` (enforced at runtime, not by clap).
        #[arg(long = "render-frames", value_parser = parse_named_path)]
        render_frames: Vec<(String, Utf8PathBuf)>,
        /// Override the plan's `diff.threshold` (single-frame) or
        /// `render_sequence.temporal_ssim_threshold` (sequence). Optional.
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
    /// Check spring-bone non-penetration against world-fixed colliders.
    ///
    /// Loads a per-frame positions JSON (produced by the runner when
    /// `capture_positions = true` in the plan's `render_sequence` block) and
    /// a test plan carrying a `ccd_colliders` list, then reports the deepest
    /// penetration depth across all frames, springs, and joints.
    ///
    /// Exits non-zero when the report's `passed` field is false (i.e., the
    /// maximum penetration depth exceeds `--epsilon`).
    PenetrationDiff {
        /// Path to the `*_positions.json` file written by the runner.
        #[arg(long)]
        positions: Utf8PathBuf,
        /// Path to the test plan (`.test.yaml`) carrying `ccd_colliders`.
        #[arg(long)]
        plan: Utf8PathBuf,
        /// Penetration tolerance in metres. A joint whose signed distance to
        /// the nearest collider surface is more negative than this threshold
        /// is considered a violation. Default 2 mm (spec floor).
        #[arg(long, default_value_t = 0.002)]
        epsilon: f32,
        /// Emit a JSON object to stdout (mirrors the `PenetrationReport`
        /// shape from `vrm_diff_engine::penetration`).
        #[arg(long)]
        json: bool,
    },
    /// Print the operation catalog.
    Describe {
        #[arg(long, value_enum, default_value_t = DescribeFormat::Json)]
        format: DescribeFormat,
    },
}

/// Load PNG frame paths from a directory in lexicographic order.
fn load_frames_dir(dir: &camino::Utf8Path) -> anyhow::Result<Vec<camino::Utf8PathBuf>> {
    let mut frames: Vec<camino::Utf8PathBuf> = Vec::new();
    for entry in std::fs::read_dir(dir.as_std_path())
        .map_err(|e| anyhow::anyhow!("failed to read frames dir {dir}: {e}"))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("png") {
            let utf8 = camino::Utf8PathBuf::try_from(path)
                .map_err(|e| anyhow::anyhow!("non-utf8 path: {e}"))?;
            frames.push(utf8);
        }
    }
    frames.sort();
    if frames.is_empty() {
        anyhow::bail!("no PNG frames found in {dir}");
    }
    Ok(frames)
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
            vrma_path,
            apply_at_time,
            reference_pose_json,
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
                vrma_path,
                apply_at_time,
                reference_pose_json,
                augment_colliders: None,
            };
            let result = execute_plan(&plan_value, &opts)?;
            if emit_json {
                let overall_passed = {
                    let ssim_pass = result
                        .diff
                        .as_ref()
                        .map(|d| d.overall_passed())
                        .unwrap_or(true);
                    let position_pass = result
                        .position_diff
                        .as_ref()
                        .map(|p| p.passed)
                        .unwrap_or(true);
                    let pose_pass = result
                        .pose_diff
                        .as_ref()
                        .map(|p| p.overall_passed)
                        .unwrap_or(true);
                    ssim_pass && position_pass && pose_pass
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
                if let Some(pose_diff) = &result.pose_diff {
                    summary["pose_diff"] = serde_json::to_value(pose_diff)?;
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
            render_frames,
            reference_frames,
            renderer_name,
            json: emit_json,
        } => {
            let plan_value = load_plan(&plan)?;

            if let (Some(rf), Some(rfr)) = (render_frames, reference_frames) {
                // Sequence-mode diff
                use vrm_diff_engine::temporal::temporal_diff;
                let cand_frames = load_frames_dir(&rf)?;
                let refr_frames = load_frames_dir(&rfr)?;

                let threshold = plan_value
                    .render_sequence
                    .as_ref()
                    .and_then(|rs| rs.temporal_ssim_threshold)
                    .unwrap_or(0.90);

                let cand_refs: Vec<&camino::Utf8Path> =
                    cand_frames.iter().map(|p| p.as_path()).collect();
                let refr_refs: Vec<&camino::Utf8Path> =
                    refr_frames.iter().map(|p| p.as_path()).collect();

                let result = temporal_diff(&cand_refs, &refr_refs, threshold)?;

                // Save fields before moving result into the envelope.
                let passed = result.passed;
                let mean_ssim = result.mean_ssim;
                let min_ssim = result.min_ssim;
                let temporal_ssim_threshold = result.temporal_ssim_threshold;
                let frame_count_compared = result.frame_count_compared;
                let frame_count_match = result.frame_count_match;

                if emit_json {
                    #[derive(serde::Serialize)]
                    struct SequenceDiffEnvelope<'a> {
                        test_id: &'a str,
                        renderer: &'a str,
                        temporal_diff: vrm_diff_engine::temporal::TemporalDiffResult,
                    }
                    let envelope = SequenceDiffEnvelope {
                        test_id: &plan_value.id,
                        renderer: &renderer_name,
                        temporal_diff: result,
                    };
                    println!("{}", serde_json::to_string(&envelope)?);
                } else {
                    println!(
                        "{}: temporal mean SSIM={:.4} min={:.4} (threshold {:.4}, {}), {} frame(s) compared{}",
                        plan_value.id,
                        mean_ssim,
                        min_ssim,
                        temporal_ssim_threshold,
                        if passed { "PASS" } else { "FAIL" },
                        frame_count_compared,
                        if frame_count_match { "" } else { " (LENGTH MISMATCH)" },
                    );
                }

                if !passed {
                    std::process::exit(1);
                }
                Ok(())
            } else if let (Some(rp), Some(rr)) = (render, reference) {
                // Existing single-frame logic — keep as-is
                use crate::diff::diff_one;
                let result = diff_one(&plan_value, &rp, &rr, &renderer_name)?;
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
            } else {
                anyhow::bail!(
                    "diff: provide either --render+--reference (single-frame) \
                     or --render-frames+--reference-frames (sequence)"
                )
            }
        }
        Cmd::CrossVariantDiff {
            plan,
            render_false,
            render_true,
            renderer_name,
            json: emit_json,
        } => {
            use vrm_diff_engine::cross_variant::cross_variant_diff;
            let plan_value = load_plan(&plan)?;
            let cv = plan_value.cross_variant.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "plan '{}' has no cross_variant block; not a cross-variant spec test",
                    plan_value.id
                )
            })?;
            let result = cross_variant_diff(&render_false, &render_true, cv.max_ssim as f64)?;

            if emit_json {
                #[derive(serde::Serialize)]
                struct CrossVariantEnvelope<'a> {
                    test_id: &'a str,
                    renderer: &'a str,
                    sibling_id: &'a str,
                    cross_variant: vrm_diff_engine::cross_variant::CrossVariantResult,
                }
                let envelope = CrossVariantEnvelope {
                    test_id: &plan_value.id,
                    renderer: &renderer_name,
                    sibling_id: &cv.sibling_id,
                    cross_variant: result.clone(),
                };
                println!("{}", serde_json::to_string(&envelope)?);
            } else {
                println!(
                    "{}: cross-variant SSIM={:.4} (max {:.4}, {}) vs {}",
                    plan_value.id,
                    result.ssim,
                    result.max_ssim,
                    if result.passed { "PASS" } else { "FAIL" },
                    cv.sibling_id,
                );
            }

            if !result.passed {
                std::process::exit(1);
            }
            Ok(())
        }
        Cmd::ConsensusDiff {
            plan,
            renders,
            render_frames,
            threshold,
            render_positions,
            position_outlier_threshold_m,
            json: emit_json,
        } => {
            use vrm_diff_engine::consensus::{
                consensus_diff, position_consensus, sequence_consensus_diff, RendererRender,
                RendererSequence,
            };
            use vrm_ops::tools::DumpBonePositionsResult;

            if !render_frames.is_empty() && !renders.is_empty() {
                anyhow::bail!(
                    "consensus-diff: --render and --render-frames are mutually exclusive"
                );
            }

            let plan_value = load_plan(&plan)?;

            // ── Sequence mode ────────────────────────────────────────────────
            if !render_frames.is_empty() {
                let effective_threshold: f64 = threshold
                    .map(|t| t as f64)
                    .or_else(|| {
                        plan_value
                            .render_sequence
                            .as_ref()
                            .and_then(|rs| rs.temporal_ssim_threshold)
                    })
                    .unwrap_or(0.90);

                let mut renderer_seqs: Vec<RendererSequence> = Vec::new();
                for (name, dir) in &render_frames {
                    let frame_paths = load_frames_dir(dir)?;
                    renderer_seqs.push(RendererSequence {
                        name: name.clone(),
                        frame_paths,
                    });
                }

                let result =
                    sequence_consensus_diff(&plan_value.id, &renderer_seqs, effective_threshold)?;
                let passed = result.overall_passed();

                if emit_json {
                    println!("{}", serde_json::to_string(&result)?);
                } else {
                    println!(
                        "{}: temporal consensus {} ({} renderers, {} outliers)",
                        result.test_id,
                        if passed { "PASS" } else { "FAIL" },
                        result.renderers.len(),
                        result.outliers.len(),
                    );
                    for (i, name) in result.renderers.iter().enumerate() {
                        println!(
                            "  {name}: agreement={}/{}  frames={}",
                            result.agreement_count[i],
                            result.renderers.len() - 1,
                            result.frame_counts[i],
                        );
                    }
                    if !result.outliers.is_empty() {
                        println!("  outliers: {:?}", result.outliers);
                    }
                }

                if !passed {
                    std::process::exit(1);
                }
                return Ok(());
            }

            // ── Single-frame mode (unchanged) ────────────────────────────────
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
        Cmd::PenetrationDiff {
            positions,
            plan,
            epsilon,
            json: emit_json,
        } => {
            use crate::penetration_diff::run_penetration_diff;
            let report = run_penetration_diff(&positions, &plan, epsilon)?;
            let passed = report.passed;

            if emit_json {
                println!("{}", serde_json::to_string(&report)?);
            } else {
                println!(
                    "penetration-diff: max_depth={:.4}m epsilon={:.4}m worst_frame={} worst_spring={} worst_joint={} ({})",
                    report.max_penetration_depth_m,
                    report.epsilon_m,
                    report.worst_frame_index,
                    report.worst_spring,
                    report.worst_joint,
                    if passed { "PASS" } else { "FAIL" },
                );
            }

            if !passed {
                std::process::exit(1);
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
                                "reference_positions": { "type": "string", "description": "Path to a positions JSON file (DumpBonePositionsResult shape) to diff dumped bone positions against." },
                                "vrma": { "type": "string", "description": "Path to a .vrma file. When set, the runner loads it and applies it at --apply-at-time before render, dumping pose/expressions/lookAt to <output_dir>/<id>_<renderer>.pose.json." },
                                "apply_at_time": { "type": "number", "default": 0.0, "description": "Sample time (seconds) for apply_vrma_at_time. Ignored when --vrma is not set." },
                                "reference_pose_json": { "type": "string", "description": "Path to a reference pose JSON fixture. When set with --vrma, the runner runs pose_diff and includes the report in JSON output." }
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
                                "pose_diff": { "type": ["object", "null"], "description": "Present only when --vrma and --reference-pose-json were provided. Pose-space diff report comparing dumped pose against reference fixture." },
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
                        "summary": "Single-frame or sequence-mode diff against a reference. Single-frame mode: --render <png> --reference <png>. Sequence mode: --render-frames <dir> --reference-frames <dir> (mutually exclusive). Exit non-zero when overall_passed / passed is false.",
                        "input_schema": {
                            "type": "object",
                            "required": ["plan"],
                            "properties": {
                                "plan": { "type": "string" },
                                "render": {
                                    "type": "string",
                                    "description": "Single-frame candidate PNG. Mutually exclusive with render_frames."
                                },
                                "reference": {
                                    "type": "string",
                                    "description": "Single-frame reference PNG. Mutually exclusive with reference_frames."
                                },
                                "render_frames": {
                                    "type": "string",
                                    "description": "Sequence-mode candidate frames directory. PNG files loaded in lexicographic order. Mutually exclusive with render."
                                },
                                "reference_frames": {
                                    "type": "string",
                                    "description": "Sequence-mode reference frames directory. Must be provided together with render_frames."
                                },
                                "renderer_name": { "type": "string" }
                            }
                        },
                        "output_schema": {
                            "oneOf": [
                                {
                                    "description": "Single-frame mode output (DiffResult)",
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
                                },
                                {
                                    "description": "Sequence mode output (TemporalDiffResult envelope)",
                                    "type": "object",
                                    "properties": {
                                        "test_id": { "type": "string" },
                                        "renderer": { "type": "string" },
                                        "temporal_diff": {
                                            "type": "object",
                                            "properties": {
                                                "frame_count": { "type": "integer" },
                                                "frame_count_compared": { "type": "integer" },
                                                "per_frame": { "type": "array" },
                                                "mean_ssim": { "type": "number" },
                                                "p95_ssim": { "type": "number" },
                                                "min_ssim": { "type": "number" },
                                                "worst_frame_index": { "type": "integer" },
                                                "frame_count_match": { "type": "boolean" },
                                                "temporal_ssim_threshold": { "type": "number" },
                                                "passed": { "type": "boolean" }
                                            }
                                        }
                                    }
                                }
                            ]
                        }
                    },
                    "consensus-diff": {
                        "summary": "N-way cross-renderer consensus diff. Single-frame mode: each --render name=path contributes one renderer to the SSIM pool. Sequence mode: each --render-frames name=dir contributes a directory of PNG frames; temporal_diff is run per pair and aggregated to a mean SSIM matrix. --render and --render-frames are mutually exclusive. --render-positions name=path adds per-renderer spring-bone position JSON for position consensus (single-frame mode only). Outputs the full matrix, per-renderer agreement counts, outlier names. Exit non-zero when any renderer is an outlier.",
                        "input_schema": {
                            "type": "object",
                            "required": ["plan"],
                            "properties": {
                                "plan": { "type": "string" },
                                "renders": {
                                    "type": "array",
                                    "items": { "type": "string", "description": "name=path (single PNG)" },
                                    "description": "Single-frame mode. Mutually exclusive with render_frames.",
                                    "minItems": 2
                                },
                                "render_frames": {
                                    "type": "array",
                                    "items": { "type": "string", "description": "name=dir (directory of PNG frames in lexicographic order)" },
                                    "description": "Sequence mode. Mutually exclusive with renders.",
                                    "minItems": 2
                                },
                                "threshold": {
                                    "type": "number",
                                    "description": "Override plan diff.threshold (single-frame) or render_sequence.temporal_ssim_threshold (sequence). Default 0.90 for sequence mode."
                                },
                                "render_positions": {
                                    "type": "array",
                                    "items": { "type": "string", "description": "name=path to DumpBonePositionsResult JSON" },
                                    "description": "Per-renderer spring-bone position files for N-way position consensus (single-frame mode only). Phase 1: first spring chain only."
                                },
                                "position_outlier_threshold_m": {
                                    "type": "number",
                                    "default": 0.010,
                                    "description": "A renderer whose mean pairwise drift vs all peers exceeds the median by this amount (metres) is flagged as a position outlier."
                                }
                            }
                        },
                        "output_schema": {
                            "oneOf": [
                                {
                                    "description": "Single-frame mode output (ConsensusResult + optional position_consensus)",
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
                                },
                                {
                                    "description": "Sequence mode output (SequenceConsensusResult)",
                                    "type": "object",
                                    "properties": {
                                        "test_id": { "type": "string" },
                                        "threshold": { "type": "number" },
                                        "renderers": { "type": "array", "items": { "type": "string" } },
                                        "mean_ssim_matrix": {
                                            "type": "array",
                                            "items": { "type": "array", "items": { "type": "number" } },
                                            "description": "Symmetric N×N temporal mean SSIM per renderer pair. Diagonal 1.0. Length mismatches become 0.0 (hard disagreement)."
                                        },
                                        "agreement_count": { "type": "array", "items": { "type": "integer" } },
                                        "outliers": { "type": "array", "items": { "type": "string" } },
                                        "consensus_passed": { "type": "boolean" },
                                        "frame_counts": {
                                            "type": "array",
                                            "items": { "type": "integer" },
                                            "description": "Number of frames each renderer contributed, in renderers[] order."
                                        }
                                    }
                                }
                            ]
                        }
                    },
                    "cross-variant-diff": {
                        "summary": "Assert two renders of the SAME renderer DIFFER (inverted SSIM). Reads max_ssim from the plan's cross_variant block; passes iff ssim(--render-false, --render-true) <= max_ssim. Exits non-zero when the renders do not diverge. Used by the doubleSided back-face-culling spec test.",
                        "input_schema": {
                            "type": "object",
                            "required": ["plan", "render_false", "render_true"],
                            "properties": {
                                "plan": { "type": "string", "description": "Test plan (.test.yaml) carrying the cross_variant block; max_ssim is read from it." },
                                "render_false": { "type": "string", "description": "PNG render of the doubleSided=false variant." },
                                "render_true": { "type": "string", "description": "PNG render of the doubleSided=true variant." },
                                "renderer_name": { "type": "string", "default": "univrm" }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "test_id": { "type": "string" },
                                "renderer": { "type": "string" },
                                "sibling_id": { "type": "string" },
                                "cross_variant": {
                                    "type": "object",
                                    "properties": {
                                        "ssim": { "type": "number" },
                                        "max_ssim": { "type": "number" },
                                        "passed": { "type": "boolean" }
                                    }
                                }
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
