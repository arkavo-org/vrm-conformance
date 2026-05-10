//! Bridges a `vrm_test_plan::TestPlan` plus two rendered PNGs (the
//! produced render and a known-good reference) into a `DiffResult`. SSIM
//! compares render vs reference; property assertions are evaluated on
//! the render alone (they measure absolute renderer behavior, not
//! similarity to a reference).

use anyhow::{Context, Result};
use camino::Utf8Path;
use vrm_diff_engine::property::eval_property;
use vrm_diff_engine::result::DiffResult;
use vrm_diff_engine::ssim::ssim_pngs;
use vrm_test_plan::TestPlan;

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
