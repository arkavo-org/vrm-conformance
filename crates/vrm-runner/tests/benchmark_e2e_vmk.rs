//! End-to-end benchmark test against the VMK Swift adapter.
//!
//! Ignored by default because it requires:
//!   - Xcode 26 + macOS 26 (VMK's platform floor)
//!   - `swift build` to compile the adapter (or a pre-built release binary)
//!   - A Metal-capable GPU
//!
//! Run locally with:
//!   cargo test -p vrm-runner --test benchmark_e2e_vmk -- --ignored
//!
//! This test validates the Rust↔Swift boundary for the benchmark ops:
//!   - `benchmark_plan` / `benchmark_execute` round-trip through the runner
//!   - All four capabilities (timing, structural, geometry, resources) present
//!   - Numbers are sane (> 0 where spec says > 0)
//!   - Animated path sets `protocol.animated = true`

use camino::Utf8PathBuf;
use vrm_asset_generator::emit::emit_vrm;
use vrm_asset_generator::params::MToonParams;
use vrm_asset_generator::sidecar::build_default_test_plan;
use vrm_ops::tools as ops;
use vrm_runner::benchmark::{run_benchmark, BenchmarkOptions, BenchmarkOutcome};

/// Locate the VMK adapter release binary.
///
/// Mirrors `vmk_bin()` in `render_sequence_e2e_vmk.rs`: walks from
/// CARGO_MANIFEST_DIR to the workspace root, then to the adapter's release
/// output directory. If the binary is absent the test skips rather than
/// failing, so `cargo test --workspace` stays green on machines that don't
/// have a built VMK adapter.
fn vmk_bin() -> Option<Utf8PathBuf> {
    let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap().parent().unwrap();
    let bin = workspace_root
        .join("adapters/vrm-metal-kit")
        .join(".build/release/vrm-metal-kit-adapter");
    if bin.exists() {
        Some(bin)
    } else {
        None
    }
}

#[test]
#[ignore = "requires Xcode 26 + macOS 26 + swift build + Metal GPU"]
fn vmk_benchmark_all_four_capabilities_sane_numbers() {
    // --- gating: skip cleanly if the adapter binary hasn't been built ---
    let Some(adapter_bin) = vmk_bin() else {
        eprintln!("SKIP: VMK adapter binary not found — run `swift build -c release` in adapters/vrm-metal-kit");
        return;
    };

    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = asset_dir.join("out");

    let id = "bench_e2e_vmk";
    let vrm_path = asset_dir.join(format!("{id}.vrm"));

    // Emit a real .vrm so VMK has a glTF mesh to load and render.
    let mtoon = MToonParams::defaults(id);
    emit_vrm(&mtoon, &vrm_path).expect("emit_vrm must succeed");

    let plan = build_default_test_plan(&mtoon, &format!("{id}.vrm"));

    let opts = BenchmarkOptions {
        adapter_bin,
        adapter_args: Vec::new(),
        asset_dir,
        output_dir,
        renderer_name: "vmk-bench".to_string(),
        warmup_frames: 2,
        measured_frames: 8,
        animate: false,
        emit_progress_ndjson: false,
    };

    let outcome = run_benchmark(&plan, &opts).expect("run_benchmark must succeed");

    let report = match outcome {
        BenchmarkOutcome::Report(r) => *r,
        BenchmarkOutcome::Unimplemented { phase } => {
            panic!(
                "VMK benchmark returned Unimplemented (phase={phase:?}) — \
                 benchmark_plan/benchmark_execute must be implemented on this branch"
            );
        }
    };

    // --- identity ---
    assert_eq!(report.test_id, id);
    assert_eq!(report.renderer_name, "vmk-bench");
    assert!(
        report.asset_blake3.starts_with("blake3:"),
        "asset_blake3 must have blake3: prefix, got: {}",
        report.asset_blake3
    );

    let m = &report.measurement;

    // --- protocol ---
    assert_eq!(m.protocol.warmup_frames, 2);
    assert_eq!(m.protocol.measured_frames, 8);
    assert!(
        !m.protocol.animated,
        "animated should be false for non-animated run"
    );

    // --- all four capabilities present ---
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
    assert!(
        m.capabilities.contains(&ops::PerfCapability::Resources),
        "capabilities must include Resources; got: {:?}",
        m.capabilities
    );

    // --- timing ---
    let timing = m
        .timing
        .expect("PerfTiming must be Some when Timing capability declared");
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
        timing.fps_mean > 0.0,
        "fps_mean must be > 0, got {}",
        timing.fps_mean
    );

    // --- structural ---
    let structural = m
        .structural
        .expect("PerfStructural must be Some when Structural capability declared");
    assert!(
        structural.draw_calls > 0.0,
        "draw_calls must be > 0, got {}",
        structural.draw_calls
    );

    // --- geometry ---
    let geometry = m
        .geometry
        .expect("PerfGeometry must be Some when Geometry capability declared");
    assert!(
        geometry.triangles > 0,
        "triangles must be > 0, got {}",
        geometry.triangles
    );
    assert!(
        geometry.vertices > 0,
        "vertices must be > 0, got {}",
        geometry.vertices
    );

    // --- resources ---
    let resources = m
        .resources
        .expect("PerfResources must be Some when Resources capability declared");
    assert!(
        resources.peak_memory_bytes > 0,
        "peak_memory_bytes must be > 0, got {}",
        resources.peak_memory_bytes
    );
    assert_eq!(
        resources.memory_kind,
        ops::PerfMemoryKind::Gpu,
        "memory_kind should be Gpu on VMK"
    );
    assert!(
        resources.load_ms >= 0.0,
        "load_ms must be >= 0, got {}",
        resources.load_ms
    );
    assert!(
        resources.first_frame_ms > 0.0,
        "first_frame_ms must be > 0, got {}",
        resources.first_frame_ms
    );

    // --- host ---
    assert_eq!(m.host.os, "macOS", "host.os must be macOS");
    assert!(
        !m.host.gpu_model.is_empty(),
        "host.gpu_model must be non-empty"
    );
}

#[test]
#[ignore = "requires Xcode 26 + macOS 26 + swift build + Metal GPU"]
fn vmk_benchmark_animated_path_sets_protocol_animated_true() {
    let Some(adapter_bin) = vmk_bin() else {
        eprintln!("SKIP: VMK adapter binary not found");
        return;
    };

    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = asset_dir.join("out");

    let id = "bench_anim_vmk";
    let vrm_path = asset_dir.join(format!("{id}.vrm"));
    let mtoon = MToonParams::defaults(id);
    emit_vrm(&mtoon, &vrm_path).expect("emit_vrm must succeed");
    let plan = build_default_test_plan(&mtoon, &format!("{id}.vrm"));

    let opts = BenchmarkOptions {
        adapter_bin,
        adapter_args: Vec::new(),
        asset_dir,
        output_dir,
        renderer_name: "vmk-bench-anim".to_string(),
        warmup_frames: 2,
        measured_frames: 8,
        animate: true,
        emit_progress_ndjson: false,
    };

    let outcome = run_benchmark(&plan, &opts).expect("run_benchmark (animated) must succeed");

    let report = match outcome {
        BenchmarkOutcome::Report(r) => *r,
        BenchmarkOutcome::Unimplemented { phase } => {
            panic!("VMK benchmark (animated) returned Unimplemented (phase={phase:?})");
        }
    };

    assert!(
        report.measurement.protocol.animated,
        "protocol.animated must be true for animated benchmark run"
    );
    // Capabilities still present on animated path.
    assert!(
        report.measurement.timing.is_some(),
        "timing must be Some on animated path"
    );
    assert!(
        report.measurement.geometry.is_some(),
        "geometry must be Some on animated path"
    );
}
