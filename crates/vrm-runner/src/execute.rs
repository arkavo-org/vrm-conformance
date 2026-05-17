//! Execute one test plan against one adapter, producing a PNG.

use crate::adapter::Adapter;
use crate::plan_to_ops::*;
use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use serde_json::json;
use vrm_ops::tools as ops;
use vrm_test_plan::TestPlan;

#[derive(Debug, Clone)]
pub struct ExecuteOptions {
    pub adapter_bin: Utf8PathBuf,
    pub adapter_args: Vec<String>,
    pub asset_dir: Utf8PathBuf,
    pub output_dir: Utf8PathBuf,
    pub renderer_name: String,
    pub emit_progress_ndjson: bool,
    /// If provided, diff the produced render against this reference PNG and
    /// include the result in `ExecuteResult::diff`.
    pub reference: Option<Utf8PathBuf>,
    /// If provided, dump bone positions after render and diff against
    /// this JSON reference file (same shape as `DumpBonePositionsResult`).
    pub reference_positions: Option<Utf8PathBuf>,
    /// If provided, the runner loads this `.vrma` and calls
    /// `apply_vrma_at_time(t)` before render. None disables VRMA.
    pub vrma_path: Option<Utf8PathBuf>,
    /// Sample time for `apply_vrma_at_time`. Ignored when `vrma_path` is
    /// None. Defaults to 0.0 when VRMA is set but no time is specified.
    pub apply_at_time: f32,
    /// JSON fixture path with reference humanoid pose + expressions +
    /// lookAt state. When set, the runner runs pose_diff against this
    /// reference and includes the report in `ExecuteResult::pose_diff`.
    pub reference_pose_json: Option<Utf8PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ExecuteResult {
    pub test_id: String,
    pub renderer: String,
    pub output_png: Utf8PathBuf,
    pub actual_color_space: ops::ColorSpace,
    /// Populated only when `ExecuteOptions::reference` was set.
    pub diff: Option<vrm_diff_engine::result::DiffResult>,
    /// Populated only when `ExecuteOptions::reference_positions` was set.
    pub position_diff: Option<vrm_diff_engine::positions::PositionDiffReport>,
    /// Populated only when `ExecuteOptions::reference_pose_json` was set.
    pub pose_diff: Option<vrm_diff_engine::pose_diff::PoseDiffReport>,
}

pub fn execute_plan(plan: &TestPlan, opts: &ExecuteOptions) -> Result<ExecuteResult> {
    let asset_path = opts.asset_dir.join(&plan.asset);
    if !asset_path.exists() {
        anyhow::bail!("asset not found: {asset_path}");
    }

    progress(opts, "spawn", &plan.id, json!({}));
    let mut adapter = Adapter::spawn(&opts.adapter_bin, &opts.adapter_args)
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    progress(opts, "load_vrm", &plan.id, json!({ "asset": asset_path }));
    let load: ops::LoadVrmResult = adapter
        .call(
            "load_vrm",
            ops::LoadVrmParams {
                path: asset_path.to_string(),
            },
        )
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
    let session_id = load.session_id;

    progress(opts, "set_camera", &plan.id, json!({}));
    let _: ops::UnitResult = adapter
        .call("set_camera", camera_params(&session_id, &plan.camera))
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    progress(opts, "set_lighting", &plan.id, json!({}));
    let _: ops::UnitResult = adapter
        .call("set_lighting", lighting_params(&session_id, &plan.lighting))
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    progress(opts, "set_post_processing", &plan.id, json!({}));
    let _: ops::UnitResult = adapter
        .call(
            "set_post_processing",
            post_processing_params(&session_id, &plan.post_processing),
        )
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    // VRMA op sequence: load .vrma, apply at time, dump pose state.
    // Runs after set_post_processing and before physics so the dumps
    // capture the pure VRMA-applied pose, uncontaminated by physics.
    let (humanoid_dump, expression_dump, look_at_dump) = if let Some(vrma) =
        plan.animation.as_ref().and_then(|a| a.vrma.as_ref())
    {
        progress(opts, "load_vrma", &plan.id, json!({ "vrma_path": &vrma.path }));
        let loaded: ops::LoadVrmaResult = adapter
            .call("load_vrma", load_vrma_params(&opts.asset_dir, vrma))
            .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

        progress(
            opts,
            "apply_vrma_at_time",
            &plan.id,
            json!({ "time_seconds": vrma.apply_at_time }),
        );
        let _: ops::ApplyVrmaAtTimeResult = adapter
            .call(
                "apply_vrma_at_time",
                apply_vrma_at_time_params(
                    &session_id,
                    loaded.vrma_handle,
                    0,
                    vrma.apply_at_time,
                ),
            )
            .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

        progress(opts, "dump_humanoid_pose", &plan.id, json!({}));
        let humanoid: ops::DumpHumanoidPoseResult = adapter
            .call(
                "dump_humanoid_pose",
                ops::DumpHumanoidPoseParams {
                    session_id: session_id.clone(),
                },
            )
            .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

        progress(opts, "dump_expression_weights", &plan.id, json!({}));
        let expressions: ops::DumpExpressionWeightsResult = adapter
            .call(
                "dump_expression_weights",
                ops::DumpExpressionWeightsParams {
                    session_id: session_id.clone(),
                },
            )
            .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

        progress(opts, "dump_look_at_state", &plan.id, json!({}));
        let look_at: ops::DumpLookAtStateResult = adapter
            .call(
                "dump_look_at_state",
                ops::DumpLookAtStateParams {
                    session_id: session_id.clone(),
                },
            )
            .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

        (Some(humanoid), Some(expressions), Some(look_at))
    } else {
        (None, None, None)
    };

    if let (Some(h), Some(e), Some(la)) = (&humanoid_dump, &expression_dump, &look_at_dump) {
        let dumps_path = opts
            .output_dir
            .join(format!("{}_{}.pose.json", plan.id, opts.renderer_name));
        if let Some(parent) = dumps_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let payload = json!({
            "humanoid": h,
            "expressions": e,
            "look_at": la,
        });
        std::fs::write(&dumps_path, serde_json::to_string_pretty(&payload)?)?;
    }

    if let Some(physics) = &plan.physics {
        progress(
            opts,
            "reset_physics",
            &plan.id,
            json!({ "settle_steps": physics.settle_steps }),
        );
        let _: ops::UnitResult = adapter
            .call(
                "reset_physics",
                ops::ResetPhysicsParams {
                    session_id: session_id.clone(),
                    settle_steps: physics.settle_steps,
                },
            )
            .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
    }

    if let Some(animation) = &plan.animation {
        if let Some(root) = &animation.root_transform {
            progress(
                opts,
                "animate_root_transform",
                &plan.id,
                json!({
                    "duration_seconds": root.duration_seconds,
                    "fps": root.fps,
                }),
            );
            let _: ops::UnitResult = adapter
                .call(
                    "animate_root_transform",
                    animate_root_transform_params(&session_id, root),
                )
                .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
        }
    }

    let png = opts
        .output_dir
        .join(format!("{}_{}.png", plan.id, opts.renderer_name));
    if let Some(parent) = png.parent() {
        std::fs::create_dir_all(parent)?;
    }
    progress(opts, "render", &plan.id, json!({ "output": png }));
    let render: ops::RenderResult = adapter
        .call(
            "render",
            render_params(&session_id, &plan.output, png.to_string()),
        )
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    let position_dump: Option<ops::DumpBonePositionsResult> = if opts.reference_positions.is_some()
    {
        progress(opts, "dump_bone_positions", &plan.id, json!({}));
        let r: ops::DumpBonePositionsResult = adapter
            .call(
                "dump_bone_positions",
                ops::DumpBonePositionsParams {
                    session_id: session_id.clone(),
                    spring_index: None,
                },
            )
            .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
        Some(r)
    } else {
        None
    };

    progress(opts, "dispose", &plan.id, json!({}));
    let _: ops::UnitResult = adapter
        .call("dispose", ops::DisposeParams { session_id })
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
    adapter
        .shutdown()
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    let output_png = Utf8PathBuf::from(render.output_path);

    let diff = if let Some(reference) = &opts.reference {
        progress(opts, "diff", &plan.id, json!({ "reference": reference }));
        Some(crate::diff::diff_one(
            plan,
            &output_png,
            reference,
            &opts.renderer_name,
        )?)
    } else {
        None
    };

    let position_diff =
        if let (Some(ref_path), Some(dump)) = (&opts.reference_positions, position_dump.as_ref()) {
            progress(
                opts,
                "position_diff",
                &plan.id,
                json!({ "reference_positions": ref_path }),
            );
            Some(crate::diff::diff_positions_one(plan, dump, ref_path)?)
        } else {
            None
        };

    let pose_diff = if let Some(ref_path) = &opts.reference_pose_json {
        // The actual pose dump was written during the VRMA op sequence
        // earlier in execute_plan (when animation.vrma was set).
        let actual_path = opts
            .output_dir
            .join(format!("{}_{}.pose.json", plan.id, opts.renderer_name));
        if !actual_path.exists() {
            anyhow::bail!(
                "reference_pose_json set but no pose.json captured at {actual_path}\
                 — does the test plan have animation.vrma?"
            );
        }
        progress(
            opts,
            "pose_diff",
            &plan.id,
            json!({ "reference_pose_json": ref_path }),
        );
        Some(crate::diff::diff_pose_one(plan, &actual_path, ref_path)?)
    } else {
        None
    };

    Ok(ExecuteResult {
        test_id: plan.id.clone(),
        renderer: opts.renderer_name.clone(),
        output_png,
        actual_color_space: render.actual_color_space,
        diff,
        position_diff,
        pose_diff,
    })
}

/// Run one plan through the adapter and always return the bone positions dump.
/// Used by matrix mode which needs positions for every render regardless of
/// whether a reference_positions file was provided.
pub fn execute_plan_capturing_positions(
    plan: &TestPlan,
    opts: &ExecuteOptions,
) -> Result<ops::DumpBonePositionsResult> {
    let asset_path = opts.asset_dir.join(&plan.asset);
    if !asset_path.exists() {
        anyhow::bail!("asset not found: {asset_path}");
    }

    progress(opts, "spawn", &plan.id, json!({}));
    let mut adapter = Adapter::spawn(&opts.adapter_bin, &opts.adapter_args)
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    progress(opts, "load_vrm", &plan.id, json!({ "asset": asset_path }));
    let load: ops::LoadVrmResult = adapter
        .call(
            "load_vrm",
            ops::LoadVrmParams {
                path: asset_path.to_string(),
            },
        )
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
    let session_id = load.session_id;

    progress(opts, "set_camera", &plan.id, json!({}));
    let _: ops::UnitResult = adapter
        .call("set_camera", camera_params(&session_id, &plan.camera))
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    progress(opts, "set_lighting", &plan.id, json!({}));
    let _: ops::UnitResult = adapter
        .call("set_lighting", lighting_params(&session_id, &plan.lighting))
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    progress(opts, "set_post_processing", &plan.id, json!({}));
    let _: ops::UnitResult = adapter
        .call(
            "set_post_processing",
            post_processing_params(&session_id, &plan.post_processing),
        )
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    if let Some(physics) = &plan.physics {
        progress(
            opts,
            "reset_physics",
            &plan.id,
            json!({ "settle_steps": physics.settle_steps }),
        );
        let _: ops::UnitResult = adapter
            .call(
                "reset_physics",
                ops::ResetPhysicsParams {
                    session_id: session_id.clone(),
                    settle_steps: physics.settle_steps,
                },
            )
            .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
    }

    if let Some(animation) = &plan.animation {
        if let Some(root) = &animation.root_transform {
            progress(
                opts,
                "animate_root_transform",
                &plan.id,
                json!({
                    "duration_seconds": root.duration_seconds,
                    "fps": root.fps,
                }),
            );
            let _: ops::UnitResult = adapter
                .call(
                    "animate_root_transform",
                    animate_root_transform_params(&session_id, root),
                )
                .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
        }
    }

    let png = opts
        .output_dir
        .join(format!("{}_{}.png", plan.id, opts.renderer_name));
    if let Some(parent) = png.parent() {
        std::fs::create_dir_all(parent)?;
    }
    progress(opts, "render", &plan.id, json!({ "output": png }));
    let _: ops::RenderResult = adapter
        .call(
            "render",
            render_params(&session_id, &plan.output, png.to_string()),
        )
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    progress(opts, "dump_bone_positions", &plan.id, json!({}));
    let dump: ops::DumpBonePositionsResult = adapter
        .call(
            "dump_bone_positions",
            ops::DumpBonePositionsParams {
                session_id: session_id.clone(),
                spring_index: None,
            },
        )
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    progress(opts, "dispose", &plan.id, json!({}));
    let _: ops::UnitResult = adapter
        .call("dispose", ops::DisposeParams { session_id })
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
    adapter
        .shutdown()
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    Ok(dump)
}

pub fn load_plan(path: &Utf8Path) -> Result<TestPlan> {
    let s = std::fs::read_to_string(path.as_std_path())?;
    Ok(serde_yml::from_str(&s)?)
}

fn progress(opts: &ExecuteOptions, phase: &str, test_id: &str, extra: serde_json::Value) {
    if opts.emit_progress_ndjson {
        let mut o = json!({
            "event": "progress",
            "op": "execute_plan",
            "phase": phase,
            "test_id": test_id,
        });
        if let Some(obj) = o.as_object_mut() {
            if let Some(extra_obj) = extra.as_object() {
                for (k, v) in extra_obj {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
        eprintln!("{}", serde_json::to_string(&o).unwrap_or_default());
    }
}

#[cfg(test)]
mod reference_positions_tests {
    use super::*;

    #[test]
    fn execute_options_and_result_carry_position_fields() {
        // Structural test: asserts these fields exist on the types.
        // Compile success IS the assertion.
        fn _structural(opts: &ExecuteOptions, r: &ExecuteResult) {
            let _: &Option<Utf8PathBuf> = &opts.reference_positions;
            let _: &Option<vrm_diff_engine::positions::PositionDiffReport> = &r.position_diff;
            let _: &Option<Utf8PathBuf> = &opts.vrma_path;
            let _: f32 = opts.apply_at_time;
            let _: &Option<Utf8PathBuf> = &opts.reference_pose_json;
            let _: &Option<vrm_diff_engine::pose_diff::PoseDiffReport> = &r.pose_diff;
        }
    }
}
