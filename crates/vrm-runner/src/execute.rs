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

    Ok(ExecuteResult {
        test_id: plan.id.clone(),
        renderer: opts.renderer_name.clone(),
        output_png,
        actual_color_space: render.actual_color_space,
        diff,
        position_diff,
    })
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
        }
    }
}
