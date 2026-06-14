//! End-to-end benchmark test against the three-vrm adapter.
//!
//! Ignored by default because it requires:
//!   - Node.js on PATH
//!   - `npm install` + `npm run build` already run in `adapters/three-vrm/`
//!   - Playwright Chromium installed (`npx playwright install chromium`)
//!
//! Run locally with:
//!   cd adapters/three-vrm && npm run build && cd ../..
//!   cargo test -p vrm-runner --test benchmark_e2e_three_vrm -- --ignored
//!
//! This test proves the Rust↔three-vrm boundary for the benchmark ops:
//!   - `benchmark_plan` / `benchmark_execute` round-trip through the runner
//!   - `capabilities` includes timing, structural, geometry (resources when heap readable)
//!   - Unmeasurable sub-fields (`state_changes`, `texture_bindings`, `vertices`)
//!     are correctly ABSENT — three.js does not instrument them

use camino::Utf8PathBuf;
use vrm_asset_generator::emit::emit_vrm;
use vrm_asset_generator::params::MToonParams;
use vrm_asset_generator::sidecar::build_default_test_plan;
use vrm_ops::tools as ops;
use vrm_runner::benchmark::{run_benchmark, BenchmarkOptions, BenchmarkOutcome};

/// Locate the three-vrm adapter entrypoint.
///
/// Mirrors `three_vrm_bin()` in `render_sequence_e2e_three_vrm.rs`: resolves
/// `adapters/three-vrm/dist/main.js` from CARGO_MANIFEST_DIR and returns
/// `("node", [<path_to_main.js>])`. Returns `None` when the build hasn't been
/// run so `cargo test --workspace` can skip cleanly on machines without it.
fn three_vrm_entrypoint() -> Option<(Utf8PathBuf, Vec<String>)> {
    let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap().parent().unwrap();
    let dist_main = workspace_root.join("adapters/three-vrm/dist/main.js");
    if !dist_main.exists() {
        return None;
    }
    // Also gate on node being available — without it the adapter can't run.
    let ok = std::process::Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        return None;
    }
    Some((Utf8PathBuf::from("node"), vec![dist_main.to_string()]))
}

#[test]
#[ignore = "requires node + three-vrm dist build + playwright chromium"]
fn three_vrm_benchmark_capabilities_and_absent_subfields() {
    // --- gating: skip cleanly when adapter or node is unavailable ---
    let Some((adapter_bin, adapter_args)) = three_vrm_entrypoint() else {
        eprintln!(
            "SKIP: three-vrm dist/main.js not found or node not on PATH — \
             run `cd adapters/three-vrm && npm run build` first"
        );
        return;
    };

    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = asset_dir.join("out");

    let id = "bench_e2e_three_vrm";
    let vrm_path = asset_dir.join(format!("{id}.vrm"));

    let mtoon = MToonParams::defaults(id);
    emit_vrm(&mtoon, &vrm_path).expect("emit_vrm must succeed");

    let plan = build_default_test_plan(&mtoon, &format!("{id}.vrm"));

    let opts = BenchmarkOptions {
        adapter_bin,
        adapter_args,
        asset_dir,
        output_dir,
        renderer_name: "three-vrm-bench".to_string(),
        warmup_frames: 3,
        measured_frames: 10,
        animate: false,
        emit_progress_ndjson: false,
    };

    let outcome = run_benchmark(&plan, &opts).expect("run_benchmark must succeed");

    let report = match outcome {
        BenchmarkOutcome::Report(r) => *r,
        BenchmarkOutcome::Unimplemented { phase } => {
            panic!(
                "three-vrm benchmark returned Unimplemented (phase={phase:?}) — \
                 benchmark_plan/benchmark_execute must be implemented on this branch"
            );
        }
    };

    // --- identity ---
    assert_eq!(report.test_id, id);
    assert_eq!(report.renderer_name, "three-vrm-bench");
    assert!(
        report.asset_blake3.starts_with("blake3:"),
        "asset_blake3 must have blake3: prefix, got: {}",
        report.asset_blake3
    );

    let m = &report.measurement;

    // --- protocol ---
    assert_eq!(m.protocol.warmup_frames, 3);
    assert_eq!(m.protocol.measured_frames, 10);
    assert!(
        !m.protocol.animated,
        "animated should be false for non-animated run"
    );

    // --- capabilities: timing, structural, geometry required ---
    assert!(
        m.capabilities.contains(&ops::PerfCapability::Timing),
        "capabilities must include Timing; got: {:?}",
        m.capabilities
    );
    assert!(
        m.capabilities.contains(&ops::PerfCapability::Structural),
        "capabilities must include Structural; got: {:?}",
        m.capabilities
    );
    assert!(
        m.capabilities.contains(&ops::PerfCapability::Geometry),
        "capabilities must include Geometry; got: {:?}",
        m.capabilities
    );

    // --- timing ---
    let timing = m
        .timing
        .as_ref()
        .expect("PerfTiming must be Some when Timing capability declared");
    assert_eq!(
        timing.clock,
        ops::PerfClock::Cpu,
        "three-vrm reports CPU-side timing; clock must be Cpu"
    );
    assert!(
        timing.frame_time_ms.p50 >= 0.0,
        "p50 must be >= 0, got {}",
        timing.frame_time_ms.p50
    );
    assert!(
        timing.frame_time_ms.p95 >= 0.0,
        "p95 must be >= 0, got {}",
        timing.frame_time_ms.p95
    );
    assert!(
        timing.frame_time_ms.p99 >= 0.0,
        "p99 must be >= 0, got {}",
        timing.frame_time_ms.p99
    );
    assert!(
        timing.fps_mean >= 0.0,
        "fps_mean must be >= 0, got {}",
        timing.fps_mean
    );

    // --- structural: draw_calls present; state_changes + texture_bindings ABSENT ---
    let structural = m
        .structural
        .as_ref()
        .expect("PerfStructural must be Some when Structural capability declared");
    assert!(
        structural.draw_calls >= 0.0,
        "draw_calls must be >= 0, got {}",
        structural.draw_calls
    );
    assert!(
        structural.state_changes.is_none(),
        "three.js does not instrument state_changes — must be None, got {:?}",
        structural.state_changes
    );
    assert!(
        structural.texture_bindings.is_none(),
        "three.js does not instrument texture_bindings — must be None, got {:?}",
        structural.texture_bindings
    );

    // --- geometry: triangles present; vertices ABSENT ---
    let geometry = m
        .geometry
        .as_ref()
        .expect("PerfGeometry must be Some when Geometry capability declared");
    assert!(
        geometry.triangles > 0,
        "triangles must be > 0, got {}",
        geometry.triangles
    );
    assert!(
        geometry.vertices.is_none(),
        "three.js exposes no per-frame vertex counter — vertices must be None, got {:?}",
        geometry.vertices
    );

    // --- host ---
    assert!(
        !m.host.gpu_model.is_empty(),
        "host.gpu_model must be non-empty"
    );
}
