//! Drives an adapter through the benchmark op sequence and writes a PerfReport.
//!
//! Scene setup mirrors `execute_plan` (load_vrm -> set_camera -> set_lighting ->
//! set_post_processing) so the measured scene matches the conformance render,
//! then runs `benchmark_plan` (cost preview, logged not gating) +
//! `benchmark_execute`. The runner owns identity (test_id / renderer_name /
//! asset_blake3); the adapter owns the measurement. v1 is observational -
//! there is no pass/fail.

use crate::adapter::{Adapter, AdapterError};
use crate::plan_to_ops::{
    benchmark_params, camera_params, lighting_params, post_processing_params,
};
use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use vrm_ops::tools as ops;
use vrm_test_plan::TestPlan;

pub struct BenchmarkOptions {
    pub adapter_bin: Utf8PathBuf,
    pub adapter_args: Vec<String>,
    pub asset_dir: Utf8PathBuf,
    pub output_dir: Utf8PathBuf,
    pub renderer_name: String,
    pub warmup_frames: u32,
    pub measured_frames: u32,
    pub animate: bool,
    pub emit_progress_ndjson: bool,
}

/// Outcome of a benchmark run: a full report, or Unimplemented when the
/// adapter does not support the op (so callers distinguish "not capable"
/// from "crashed").
pub enum BenchmarkOutcome {
    Report(Box<ops::PerfReport>),
    Unimplemented { phase: Option<String> },
}

/// Compose the on-disk report from runner-owned identity + the adapter's
/// measurement. Pure - unit-testable without a subprocess.
pub fn compose_report(
    test_id: &str,
    renderer_name: &str,
    asset_blake3: &str,
    measurement: ops::PerfMeasurement,
) -> ops::PerfReport {
    ops::PerfReport {
        test_id: test_id.to_string(),
        renderer_name: renderer_name.to_string(),
        asset_blake3: asset_blake3.to_string(),
        measurement,
    }
}

/// BLAKE3 of a file's bytes, prefixed `blake3:` per the content-addressing
/// convention. Anchors a PerfReport to the exact asset measured.
pub fn asset_blake3(path: &Utf8Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(format!("blake3:{}", blake3::hash(&bytes).to_hex()))
}

/// Where the per-test report is written.
pub fn report_path(output_dir: &Utf8Path, test_id: &str, renderer_name: &str) -> Utf8PathBuf {
    output_dir.join(format!("{test_id}_{renderer_name}.perf.json"))
}

/// Extract the `phase` string from a `-32000 Unimplemented` error envelope.
fn unimplemented_phase(e: &vrm_ops::RpcError) -> Option<String> {
    e.data
        .as_ref()
        .and_then(|d| d.get("phase"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Emit one NDJSON progress line to stderr when `opts.emit_progress_ndjson`
/// is true. Shape matches the `execute_plan` progress lines from
/// `execute.rs` (same `event`/`phase`/`test_id` keys) with `op` set to
/// `"benchmark"` so downstream consumers can distinguish the two paths.
fn progress(opts: &BenchmarkOptions, phase: &str, test_id: &str) {
    if opts.emit_progress_ndjson {
        let line = serde_json::json!({
            "event": "progress",
            "op": "benchmark",
            "phase": phase,
            "test_id": test_id,
        });
        eprintln!("{}", serde_json::to_string(&line).unwrap_or_default());
    }
}

pub fn run_benchmark(plan: &TestPlan, opts: &BenchmarkOptions) -> Result<BenchmarkOutcome> {
    let asset_path = opts.asset_dir.join(&plan.asset);
    if !asset_path.exists() {
        anyhow::bail!("asset not found: {asset_path}");
    }
    let asset_hash = asset_blake3(&asset_path)?;

    progress(opts, "spawn", &plan.id);
    let mut adapter = Adapter::spawn(&opts.adapter_bin, &opts.adapter_args)
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    progress(opts, "load_vrm", &plan.id);
    let load: ops::LoadVrmResult = adapter
        .call(
            "load_vrm",
            ops::LoadVrmParams {
                path: asset_path.to_string(),
                augment_colliders: None,
            },
        )
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
    let session_id = load.session_id;

    progress(opts, "set_camera", &plan.id);
    let _: ops::UnitResult = adapter
        .call("set_camera", camera_params(&session_id, &plan.camera))
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
    progress(opts, "set_lighting", &plan.id);
    let _: ops::UnitResult = adapter
        .call("set_lighting", lighting_params(&session_id, &plan.lighting))
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
    progress(opts, "set_post_processing", &plan.id);
    let _: ops::UnitResult = adapter
        .call(
            "set_post_processing",
            post_processing_params(&session_id, &plan.post_processing),
        )
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    let bench_params = benchmark_params(
        &session_id,
        &plan.output,
        opts.warmup_frames,
        opts.measured_frames,
        opts.animate,
    );

    // Cost preview. A -32000 here means the adapter cannot benchmark at all
    // (the contract requires -32000 on both benchmark ops) — route it to the
    // same Unimplemented outcome rather than a hard error.
    progress(opts, "benchmark_plan", &plan.id);
    let preview: std::result::Result<ops::BenchmarkPlanResult, AdapterError> =
        adapter.call("benchmark_plan", bench_params.clone());

    let outcome = match preview {
        Err(AdapterError::Rpc(ref e)) if e.code == -32000 => BenchmarkOutcome::Unimplemented {
            phase: unimplemented_phase(e),
        },
        Err(e) => return Err(anyhow::anyhow!("adapter error: {e}")),
        Ok(_preview) => {
            progress(opts, "benchmark_execute", &plan.id);
            let measured: std::result::Result<ops::PerfMeasurement, AdapterError> =
                adapter.call("benchmark_execute", bench_params);
            match measured {
                Ok(m) => BenchmarkOutcome::Report(Box::new(compose_report(
                    &plan.id,
                    &opts.renderer_name,
                    &asset_hash,
                    m,
                ))),
                Err(AdapterError::Rpc(ref e)) if e.code == -32000 => {
                    BenchmarkOutcome::Unimplemented {
                        phase: unimplemented_phase(e),
                    }
                }
                Err(e) => return Err(anyhow::anyhow!("adapter error: {e}")),
            }
        }
    };

    progress(opts, "dispose", &plan.id);
    let _: ops::UnitResult = adapter
        .call("dispose", ops::DisposeParams { session_id })
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
    adapter
        .shutdown()
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    if let BenchmarkOutcome::Report(ref report) = outcome {
        std::fs::create_dir_all(&opts.output_dir)?;
        let out = report_path(&opts.output_dir, &plan.id, &opts.renderer_name);
        std::fs::write(&out, serde_json::to_string_pretty(report.as_ref())?)?;
    }

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vrm_ops::tools as ops;

    fn sample_measurement() -> ops::PerfMeasurement {
        ops::PerfMeasurement {
            protocol: ops::PerfProtocol {
                warmup_frames: 30,
                measured_frames: 300,
                animated: false,
            },
            timing: None,
            structural: Some(ops::PerfStructural {
                draw_calls: 1.0,
                state_changes: Some(0.0),
                texture_bindings: Some(1.0),
            }),
            geometry: Some(ops::PerfGeometry {
                triangles: 2,
                vertices: Some(4),
            }),
            resources: None,
            host: ops::PerfHost {
                os: "mock".into(),
                os_version: "0".into(),
                gpu_vendor: "none".into(),
                gpu_model: "cpu".into(),
                driver_version: "0".into(),
                build_flags: String::new(),
            },
            capabilities: vec![
                ops::PerfCapability::Structural,
                ops::PerfCapability::Geometry,
            ],
        }
    }

    #[test]
    fn compose_report_sets_identity_from_runner() {
        let report = compose_report("mtoon_00", "mock", "blake3:ab", sample_measurement());
        assert_eq!(report.test_id, "mtoon_00");
        assert_eq!(report.renderer_name, "mock");
        assert_eq!(report.asset_blake3, "blake3:ab");
        assert_eq!(report.measurement.geometry.unwrap().triangles, 2);
    }

    #[test]
    fn report_path_format() {
        let p = report_path(Utf8Path::new("/out"), "mtoon_00", "mock");
        assert_eq!(p.as_str(), "/out/mtoon_00_mock.perf.json");
    }

    #[test]
    fn asset_blake3_is_prefixed_and_stable() {
        let dir = tempfile::tempdir().unwrap();
        let path = camino::Utf8PathBuf::from_path_buf(dir.path().join("a.vrm")).unwrap();
        std::fs::write(&path, b"hello").unwrap();
        let h1 = asset_blake3(&path).unwrap();
        let h2 = asset_blake3(&path).unwrap();
        assert!(h1.starts_with("blake3:"));
        assert_eq!(h1, h2);
    }
}
