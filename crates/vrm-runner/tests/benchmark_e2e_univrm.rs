//! End-to-end benchmark test against the UniVRM PlayMode adapter (batch path).
//!
//! Ignored by default because it requires:
//!   - macOS (UniVRM adapter is macOS-only per the launcher)
//!   - Unity 6 (6000.4.6f1) installed via Unity Hub (or UNITY_BIN env override)
//!   - PlayMode is the launcher default; L3_EDITMODE=1 would force EditMode
//!
//! Run locally with:
//!   cargo test -p vrm-runner --test benchmark_e2e_univrm -- --ignored
//!
//! UniVRM reports all four capabilities (timing, structural, geometry, resources).
//! This test proves the Rust↔C# boundary for benchmark ops through the batch
//! filesystem protocol (RFC-0003):
//!   - `PerfMeasurement` deserialises from the C# `measurement` JSON block
//!   - `capabilities` == [timing, structural, geometry, resources]
//!   - `structural.state_changes` and `structural.texture_bindings` ABSENT
//!     (UniVRM does not instrument them; honesty-omission contract)
//!   - `geometry.vertices` PRESENT (Unity tracks vertex count)
//!   - `timing.clock` == Cpu; sane p50/p95/p99 and fps_mean > 0
//!   - `resources.peak_memory_bytes` > 0; `memory_kind` == Host

use camino::Utf8PathBuf;
use vrm_asset_generator::emit::emit_vrm;
use vrm_asset_generator::params::MToonParams;
use vrm_asset_generator::sidecar::build_default_test_plan;
use vrm_ops::tools as ops;
use vrm_runner::execute_batch::{run, BatchBenchmarkParams, RunOptions};

/// Locate the UniVRM launcher.sh and verify Unity 6 is present.
///
/// Mirrors `univrm_launcher()` in `render_sequence_e2e_univrm.rs` exactly:
/// walks from CARGO_MANIFEST_DIR to the workspace root, checks launcher.sh
/// exists, then verifies the Unity binary is present. Returns `None` (skip)
/// when either is missing so `cargo test --workspace` stays green on machines
/// without a Unity licence.
fn univrm_launcher() -> Option<Utf8PathBuf> {
    let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap().parent().unwrap();
    let launcher = workspace_root.join("adapters/univrm/launcher.sh");
    if !launcher.exists() {
        return None;
    }
    let unity_bin = std::env::var("UNITY_BIN").unwrap_or_else(|_| {
        "/Applications/Unity/Hub/Editor/6000.4.6f1/Unity.app/Contents/MacOS/Unity".into()
    });
    if !std::path::Path::new(&unity_bin).exists() {
        return None;
    }
    Some(launcher)
}

#[test]
#[ignore = "requires Unity 6 + macOS + UniVRM PlayMode launcher"]
fn univrm_benchmark_all_four_capabilities_correct_honesty_omissions() {
    // --- gating: skip cleanly when Unity or launcher is unavailable ---
    let Some(launcher) = univrm_launcher() else {
        eprintln!(
            "SKIP: launcher.sh missing or Unity 6000.4.6f1 not installed — \
             set UNITY_BIN env or install via Hub"
        );
        return;
    };

    let dir = tempfile::tempdir().unwrap();
    let plans_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = plans_dir.join("out");
    std::fs::create_dir_all(output_dir.as_std_path()).unwrap();

    let id = "bench_univrm";
    let vrm_path = plans_dir.join(format!("{id}.vrm"));

    // Emit a real .vrm + write .test.yaml so the discover-pair step finds them.
    let mtoon = MToonParams::defaults(id);
    emit_vrm(&mtoon, &vrm_path).expect("emit_vrm must succeed");

    let plan = build_default_test_plan(&mtoon, &format!("{id}.vrm"));
    let plan_path = plans_dir.join(format!("{id}.test.yaml"));
    let yaml = serde_yml::to_string(&plan).expect("serialize plan");
    std::fs::write(plan_path.as_std_path(), yaml).unwrap();

    let opts = RunOptions {
        plans_dir: plans_dir.clone(),
        adapter_bin: launcher,
        output_dir: output_dir.clone(),
        renderer_name: "univrm".into(),
        benchmark: Some(BatchBenchmarkParams {
            warmup_frames: 2,
            measured_frames: 6,
            animate: false,
        }),
    };

    let summary = run(&opts).expect("run() against UniVRM PlayMode must succeed");
    assert!(
        summary.ok_count >= 1,
        "expected at least 1 ok result; got summary={summary:?}"
    );

    // Read and deserialise the on-disk PerfReport.
    let perf_path = output_dir.join(format!("{id}_univrm.perf.json"));
    assert!(
        perf_path.exists(),
        "perf.json not written at {perf_path} — measurement block missing from results.ndjson?"
    );
    let json = std::fs::read_to_string(perf_path.as_std_path()).expect("read perf.json");
    let report: ops::PerfReport =
        serde_json::from_str(&json).expect("PerfReport must deserialise from perf.json");

    // --- identity ---
    assert_eq!(report.test_id, id);
    assert_eq!(report.renderer_name, "univrm");
    assert!(
        report.asset_blake3.starts_with("blake3:"),
        "asset_blake3 must have blake3: prefix, got: {}",
        report.asset_blake3
    );

    let m = &report.measurement;

    // --- protocol ---
    assert_eq!(m.protocol.warmup_frames, 2);
    assert_eq!(m.protocol.measured_frames, 6);
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
        .as_ref()
        .expect("PerfTiming must be Some when Timing capability declared");
    assert_eq!(
        timing.clock,
        ops::PerfClock::Cpu,
        "UniVRM reports CPU-side timing; clock must be Cpu"
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
        timing.fps_mean > 0.0,
        "fps_mean must be > 0, got {}",
        timing.fps_mean
    );

    // --- structural: draw_calls present; state_changes + texture_bindings ABSENT ---
    //
    // UniVRM does not instrument GPU state-change or texture-binding counters.
    // The honesty-omission contract requires these fields to be None rather
    // than zero-filled — a zero would be an incorrect implicit claim.
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
        "UniVRM does not instrument state_changes — must be None, got {:?}",
        structural.state_changes
    );
    assert!(
        structural.texture_bindings.is_none(),
        "UniVRM does not instrument texture_bindings — must be None, got {:?}",
        structural.texture_bindings
    );

    // --- geometry: triangles AND vertices present (Unity tracks both) ---
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
        geometry.vertices.is_some(),
        "UniVRM reports vertex count — vertices must be Some"
    );
    if let Some(v) = geometry.vertices {
        assert!(v > 0, "vertices must be > 0, got {v}");
    }

    // --- resources ---
    let resources = m
        .resources
        .as_ref()
        .expect("PerfResources must be Some when Resources capability declared");
    assert!(
        resources.peak_memory_bytes > 0,
        "peak_memory_bytes must be > 0, got {}",
        resources.peak_memory_bytes
    );
    assert_eq!(
        resources.memory_kind,
        ops::PerfMemoryKind::Host,
        "UniVRM reports host (managed heap) memory; memory_kind must be Host"
    );
    assert!(
        resources.load_ms >= 0.0,
        "load_ms must be >= 0, got {}",
        resources.load_ms
    );
    assert!(
        resources.first_frame_ms >= 0.0,
        "first_frame_ms must be >= 0, got {}",
        resources.first_frame_ms
    );

    // --- host ---
    assert!(
        !m.host.gpu_model.is_empty(),
        "host.gpu_model must be non-empty"
    );
    assert!(
        !m.host.os.is_empty(),
        "host.os must be non-empty (expected OSXEditor or similar)"
    );
}
