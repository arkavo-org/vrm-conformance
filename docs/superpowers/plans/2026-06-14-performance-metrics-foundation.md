# Performance Metrics — Foundation Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `benchmark_plan` / `benchmark_execute` op to the uniform op surface that carries a `PerfReport` across renderers, implement it deterministically in the mock renderer, drive it from a new `vrm-runner benchmark-execute` subcommand, and aggregate results — an end-to-end, GPU-free performance-measurement path with no pass/fail gate (observational v1).

**Architecture:** Op types live in `crates/vrm-ops/src/tools.rs` (plain serde structs + roundtrip tests, the established pattern). The adapter returns a `PerfMeasurement` (the measurement + host); the runner owns identity (`test_id` / `renderer_name` / `asset_blake3`) and composes the on-disk `PerfReport`, mirroring how BLAKE3 is centralized in the runner for `render_sequence`. The mock renderer fills deterministic structural + geometry counts (no GPU, no timing). The runner subcommand drives `load_vrm → set_camera → set_lighting → set_post_processing → benchmark_plan → benchmark_execute → dispose`. A bash aggregator merges per-test `*.perf.json` into `goldens-cache/perf-report.json` with a VMK-vs-golden-ref structural delta.

**Tech Stack:** Rust 1.88 (workspace), `serde` / `serde_json`, `blake3`, `clap` (runner CLI), `jq` + bash (aggregator). No GPU, no Swift/TS/C# in this slice — those are follow-on plans.

**Scope boundary (this plan):** Rust foundation only. The VMK, three-vrm, and UniVRM adapter wiring are each a separate follow-on plan (`docs/superpowers/specs/2026-06-14-performance-metrics-design.md` §"Adapter capability matrix"). This slice ships working software: you can benchmark the mock end-to-end and `cargo test --workspace` is green.

**Design doc:** [`docs/superpowers/specs/2026-06-14-performance-metrics-design.md`](../specs/2026-06-14-performance-metrics-design.md)

---

## File Structure

| File | Responsibility | Change |
|---|---|---|
| `crates/vrm-ops/src/tools.rs` | Benchmark op param/result types + `PerfReport` schema + roundtrip tests | Modify (append types + test module) |
| `crates/vrm-mock-renderer/src/handlers.rs` | Deterministic `benchmark_plan` / `benchmark_execute` handlers | Modify (append handlers + tests) |
| `crates/vrm-mock-renderer/src/main.rs` | Dispatch arms for the two new methods | Modify (`dispatch` match) |
| `crates/vrm-runner/src/plan_to_ops.rs` | `benchmark_params(...)` plan→ops mapping | Modify (append `pub fn`) |
| `crates/vrm-runner/src/benchmark.rs` | Drive the benchmark op sequence; compose + write `PerfReport` | Create |
| `crates/vrm-runner/src/lib.rs` | Register `benchmark` module | Modify |
| `crates/vrm-runner/src/cli.rs` | `BenchmarkExecute` subcommand + handler | Modify |
| `scripts/perf-report.sh` | Aggregate `*.perf.json` → `goldens-cache/perf-report.json` | Create |
| `scripts/smoke.sh` | Add a mock benchmark E2E step | Modify |
| `docs/operation-contract.md` | Document the two ops + `PerfReport` + naming exception | Modify |
| `docs/methodology.md` | Benchmark protocol + reference machine | Modify |

---

## Task 1: Benchmark op types in `vrm-ops`

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs` (append after `RenderSequenceResult`, before the first `#[cfg(test)]` mod at line 504)
- Test: `crates/vrm-ops/src/tools.rs` (new `#[cfg(test)] mod benchmark_tests`)

- [ ] **Step 1: Write the failing tests**

Append this test module to `crates/vrm-ops/src/tools.rs` (after the existing `mod tests` block at the end of the file):

```rust
#[cfg(test)]
mod benchmark_tests {
    use super::*;

    #[test]
    fn benchmark_params_defaults_frame_counts_and_omits_animation() {
        let j = r#"{"session_id":"s","width":64,"height":64,
            "color_space":"Linear","msaa":1,"output_type":"Color"}"#;
        let p: BenchmarkParams = serde_json::from_str(j).unwrap();
        assert_eq!(p.warmup_frames, 30);
        assert_eq!(p.measured_frames, 300);
        assert!(p.animate_root_transform.is_none());
    }

    #[test]
    fn perf_enums_serialize_snake_case() {
        assert_eq!(
            serde_json::to_value(PerfCapability::Structural).unwrap(),
            "structural"
        );
        assert_eq!(serde_json::to_value(PerfClock::GpuCpu).unwrap(), "gpu_cpu");
        assert_eq!(serde_json::to_value(PerfMemoryKind::Host).unwrap(), "host");
    }

    #[test]
    fn benchmark_plan_result_roundtrip() {
        let r = BenchmarkPlanResult {
            estimated_frames: 330,
            estimated_seconds: 5.5,
            scene_summary: "static 64x64".into(),
        };
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(serde_json::from_str::<BenchmarkPlanResult>(&s).unwrap(), r);
    }

    #[test]
    fn perf_report_flattens_identity_and_measurement_and_omits_empty_blocks() {
        let report = PerfReport {
            test_id: "mtoon_00".into(),
            renderer_name: "mock".into(),
            asset_blake3: "blake3:ab".into(),
            measurement: PerfMeasurement {
                protocol: PerfProtocol {
                    warmup_frames: 30,
                    measured_frames: 300,
                    animated: false,
                },
                timing: None,
                structural: Some(PerfStructural {
                    draw_calls: 1.0,
                    state_changes: 0.0,
                    texture_bindings: 1.0,
                }),
                geometry: Some(PerfGeometry {
                    triangles: 2,
                    vertices: 4,
                }),
                resources: None,
                host: PerfHost {
                    os: "mock".into(),
                    os_version: "0".into(),
                    gpu_vendor: "none".into(),
                    gpu_model: "cpu".into(),
                    driver_version: "0".into(),
                    build_flags: String::new(),
                },
                capabilities: vec![PerfCapability::Structural, PerfCapability::Geometry],
            },
        };
        let v: serde_json::Value = serde_json::to_value(&report).unwrap();
        // identity is flattened to the top level
        assert_eq!(v["test_id"], "mtoon_00");
        assert_eq!(v["renderer_name"], "mock");
        assert_eq!(v["asset_blake3"], "blake3:ab");
        // measurement merged at top level (no `measurement` wrapper key)
        assert!(v.get("measurement").is_none());
        assert_eq!(v["protocol"]["measured_frames"], 300);
        assert_eq!(v["structural"]["draw_calls"], 1.0);
        assert_eq!(v["geometry"]["triangles"], 2);
        assert_eq!(v["capabilities"][0], "structural");
        // unpopulated blocks are omitted entirely
        assert!(v.get("timing").is_none());
        assert!(v.get("resources").is_none());
        // round-trips
        let back: PerfReport = serde_json::from_value(v).unwrap();
        assert_eq!(back, report);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vrm-ops benchmark_tests`
Expected: FAIL to compile — `cannot find type BenchmarkParams in this scope` (and the other new types).

- [ ] **Step 3: Implement the types**

Insert this block in `crates/vrm-ops/src/tools.rs` immediately after the `RenderSequenceResult` struct (ends at line 502) and before `#[cfg(test)] mod ccd_capture_positions_tests` (line 504):

```rust
// ---- Benchmark op (performance metrics, observational v1) ----
// Design: docs/superpowers/specs/2026-06-14-performance-metrics-design.md
//
// NAMING EXCEPTION: the contract's plan/execute convention is `plan_*` /
// `execute_*`. The benchmark ops use noun_verb (`benchmark_plan` /
// `benchmark_execute`) by maintainer directive so the pair groups under the
// `benchmark` noun. Documented in docs/operation-contract.md.

fn default_warmup_frames() -> u32 {
    30
}

fn default_measured_frames() -> u32 {
    300
}

/// Params for both `benchmark_plan` (cheap cost preview, no rendering) and
/// `benchmark_execute` (the measured run). The adapter renders
/// `warmup_frames` discarded frames to warm shader/pipeline caches, then
/// `measured_frames` steady-state frames over which it aggregates
/// timing/structural/geometry/resource metrics.
///
/// `animate_root_transform`, when present, drives a linear root translation
/// across the measured window so spring-bone cost is exercised; absent means
/// a static scene. See `docs/methodology.md`, "Benchmark protocol".
///
/// Adapters that cannot benchmark MUST return `-32000 Unimplemented` with
/// `data: { phase: "perf-v1" }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkParams {
    pub session_id: String,
    pub width: u32,
    pub height: u32,
    pub color_space: ColorSpace,
    pub msaa: u8,
    pub output_type: OutputType,
    #[serde(default = "default_warmup_frames")]
    pub warmup_frames: u32,
    #[serde(default = "default_measured_frames")]
    pub measured_frames: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate_root_transform: Option<RootTransformAnimation>,
}

/// Result of `benchmark_plan`: a cost preview, no rendering performed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkPlanResult {
    pub estimated_frames: u32,
    pub estimated_seconds: f32,
    pub scene_summary: String,
}

/// Which measurement blocks an adapter actually populated. Cleaner than a
/// per-field Unimplemented for partial adapters (e.g. structural-only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerfCapability {
    Timing,
    Structural,
    Geometry,
    Resources,
}

/// Benchmark protocol echoed back so the report is self-describing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerfProtocol {
    pub warmup_frames: u32,
    pub measured_frames: u32,
    pub animated: bool,
}

/// Which wall clock the timing layer used. `Cpu` is the documented fallback
/// for runtimes that cannot measure GPU submit-to-complete (e.g. browser).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerfClock {
    GpuCpu,
    Cpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FrameTimePercentiles {
    pub p50: f32,
    pub p95: f32,
    pub p99: f32,
}

/// Hardware-dependent timing layer — only comparable on a matching host.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PerfTiming {
    pub frame_time_ms: FrameTimePercentiles,
    pub fps_mean: f32,
    pub clock: PerfClock,
}

/// Hardware-independent structural layer — per-frame means over the measured
/// window. The cross-renderer "familiar" comparison axis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PerfStructural {
    pub draw_calls: f32,
    pub state_changes: f32,
    pub texture_bindings: f32,
}

/// Hardware-independent geometry layer — per-frame submission counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerfGeometry {
    pub triangles: u64,
    pub vertices: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerfMemoryKind {
    Gpu,
    Host,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PerfResources {
    pub peak_memory_bytes: u64,
    pub memory_kind: PerfMemoryKind,
    pub load_ms: f32,
    pub first_frame_ms: f32,
}

/// Host/hardware anchor. Mirrors the goldens manifest's `SubmissionMetadata`
/// fields so timing numbers are interpretable and comparable only against a
/// matching host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerfHost {
    pub os: String,
    pub os_version: String,
    pub gpu_vendor: String,
    pub gpu_model: String,
    pub driver_version: String,
    pub build_flags: String,
}

/// What `benchmark_execute` returns — the measurement plus host, minus the
/// runner-owned identity (test_id / renderer_name / asset_blake3, which the
/// runner adds when composing the on-disk `PerfReport`). This split mirrors
/// the contract's "BLAKE3 centralized in the runner" decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerfMeasurement {
    pub protocol: PerfProtocol,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing: Option<PerfTiming>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural: Option<PerfStructural>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<PerfGeometry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<PerfResources>,
    pub host: PerfHost,
    pub capabilities: Vec<PerfCapability>,
}

/// The on-disk report the runner writes: runner-owned identity + the adapter's
/// measurement flattened into a single JSON object (the schema in the design
/// doc). Written to `<output_dir>/<test_id>_<renderer>.perf.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerfReport {
    pub test_id: String,
    pub renderer_name: String,
    pub asset_blake3: String,
    #[serde(flatten)]
    pub measurement: PerfMeasurement,
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vrm-ops benchmark_tests`
Expected: PASS (4 tests).

- [ ] **Step 5: Lint + format**

Run: `cargo fmt -p vrm-ops && cargo clippy -p vrm-ops --all-targets -- -D warnings`
Expected: no diff from fmt, zero clippy warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-ops/src/tools.rs
git commit -m "feat(ops): benchmark_plan/benchmark_execute op types + PerfReport schema"
```

---

## Task 2: Deterministic benchmark in the mock renderer

**Files:**
- Modify: `crates/vrm-mock-renderer/src/handlers.rs` (append handlers; extend `#[cfg(test)] mod tests`)
- Modify: `crates/vrm-mock-renderer/src/main.rs` (`dispatch` match, after the `render_sequence` arm at line 94)
- Test: `crates/vrm-mock-renderer/src/handlers.rs` (existing `mod tests`)

- [ ] **Step 1: Write the failing tests**

Append these tests inside the existing `#[cfg(test)] mod tests` block in `crates/vrm-mock-renderer/src/handlers.rs` (before its closing `}` at line 594):

```rust
    #[test]
    fn benchmark_execute_returns_deterministic_structural_and_geometry() {
        let (mut registry, session_id) = make_test_session();
        let params = ops::BenchmarkParams {
            session_id,
            width: 64,
            height: 64,
            color_space: ops::ColorSpace::Linear,
            msaa: 1,
            output_type: ops::OutputType::Color,
            warmup_frames: 30,
            measured_frames: 300,
            animate_root_transform: None,
        };
        let m = benchmark_execute(&mut registry, params.clone()).unwrap();
        assert_eq!(
            m.capabilities,
            vec![
                ops::PerfCapability::Structural,
                ops::PerfCapability::Geometry
            ]
        );
        assert!(m.timing.is_none(), "mock reports no timing");
        assert!(m.resources.is_none(), "mock reports no resources");
        assert_eq!(m.geometry.unwrap().triangles, 2);
        assert_eq!(m.protocol.measured_frames, 300);
        assert!(!m.protocol.animated);
        // Determinism: identical params → identical measurement.
        let m2 = benchmark_execute(&mut registry, params).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn benchmark_execute_reflects_animated_flag() {
        let (mut registry, session_id) = make_test_session();
        let params = ops::BenchmarkParams {
            session_id,
            width: 8,
            height: 8,
            color_space: ops::ColorSpace::Linear,
            msaa: 1,
            output_type: ops::OutputType::Color,
            warmup_frames: 1,
            measured_frames: 1,
            animate_root_transform: Some(ops::RootTransformAnimation {
                translation_start: [0.0, 0.0, 0.0],
                translation_end: [0.0, 0.1, 0.0],
            }),
        };
        let m = benchmark_execute(&mut registry, params).unwrap();
        assert!(m.protocol.animated);
    }

    #[test]
    fn benchmark_execute_on_unknown_session_is_invalid_params() {
        let mut registry = SessionRegistry::default();
        let params = ops::BenchmarkParams {
            session_id: "ghost".into(),
            width: 8,
            height: 8,
            color_space: ops::ColorSpace::Linear,
            msaa: 1,
            output_type: ops::OutputType::Color,
            warmup_frames: 1,
            measured_frames: 1,
            animate_root_transform: None,
        };
        let err = benchmark_execute(&mut registry, params).unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[test]
    fn benchmark_plan_estimates_total_frames() {
        let (mut registry, session_id) = make_test_session();
        let params = ops::BenchmarkParams {
            session_id,
            width: 64,
            height: 64,
            color_space: ops::ColorSpace::Linear,
            msaa: 1,
            output_type: ops::OutputType::Color,
            warmup_frames: 30,
            measured_frames: 300,
            animate_root_transform: None,
        };
        let plan = benchmark_plan(&mut registry, params).unwrap();
        assert_eq!(plan.estimated_frames, 330);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vrm-mock-renderer benchmark`
Expected: FAIL to compile — `cannot find function benchmark_execute` / `benchmark_plan`.

- [ ] **Step 3: Implement the handlers**

Append to `crates/vrm-mock-renderer/src/handlers.rs` (after the `dump_look_at_state` handler, before the `render_sequence` handler at line 242):

```rust
/// Deterministic benchmark measurement for the mock. No GPU and no real
/// scene graph, so structural/geometry counts are fixed synthetic values
/// (one draw call over a 2-triangle quad). Timing and resources are omitted —
/// CPU-only timing is not a conformance signal. Determinism makes this a
/// stable E2E + schema round-trip fixture (mirrors `synthetic_spring_chain`).
fn mock_measurement(params: &ops::BenchmarkParams) -> ops::PerfMeasurement {
    ops::PerfMeasurement {
        protocol: ops::PerfProtocol {
            warmup_frames: params.warmup_frames,
            measured_frames: params.measured_frames,
            animated: params.animate_root_transform.is_some(),
        },
        timing: None,
        structural: Some(ops::PerfStructural {
            draw_calls: 1.0,
            state_changes: 0.0,
            texture_bindings: 1.0,
        }),
        geometry: Some(ops::PerfGeometry {
            triangles: 2,
            vertices: 4,
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

pub fn benchmark_plan(
    registry: &mut SessionRegistry,
    params: ops::BenchmarkParams,
) -> Result<ops::BenchmarkPlanResult, RpcError> {
    let _session = registry
        .get(&params.session_id)
        .ok_or_else(|| invalid_session(&params.session_id))?;
    let total = params.warmup_frames + params.measured_frames;
    Ok(ops::BenchmarkPlanResult {
        estimated_frames: total,
        // Rough preview only; the mock does not really pace at 60 Hz.
        estimated_seconds: total as f32 / 60.0,
        scene_summary: format!(
            "mock deterministic {}x{} msaa{}",
            params.width, params.height, params.msaa
        ),
    })
}

pub fn benchmark_execute(
    registry: &mut SessionRegistry,
    params: ops::BenchmarkParams,
) -> Result<ops::PerfMeasurement, RpcError> {
    let _session = registry
        .get(&params.session_id)
        .ok_or_else(|| invalid_session(&params.session_id))?;
    Ok(mock_measurement(&params))
}
```

- [ ] **Step 4: Wire the dispatch arms**

In `crates/vrm-mock-renderer/src/main.rs`, add these two arms to the `dispatch` match, immediately after the `"render_sequence"` arm (line 94):

```rust
        "benchmark_plan" => json_result(handlers::benchmark_plan(registry, deser(params)?)),
        "benchmark_execute" => json_result(handlers::benchmark_execute(registry, deser(params)?)),
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p vrm-mock-renderer benchmark`
Expected: PASS (4 tests).

- [ ] **Step 6: Lint + format + full crate test**

Run: `cargo fmt -p vrm-mock-renderer && cargo clippy -p vrm-mock-renderer --all-targets -- -D warnings && cargo test -p vrm-mock-renderer`
Expected: clean, all tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-mock-renderer/src/handlers.rs crates/vrm-mock-renderer/src/main.rs
git commit -m "feat(mock): deterministic benchmark_plan/benchmark_execute handlers"
```

---

## Task 3: `benchmark_params` plan→ops mapping

**Files:**
- Modify: `crates/vrm-runner/src/plan_to_ops.rs` (append `pub fn` after `render_params`, ends ~line 78)
- Test: `crates/vrm-runner/src/plan_to_ops.rs` (add/extend a `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test**

Append (or create) a test module at the end of `crates/vrm-runner/src/plan_to_ops.rs`. If a `#[cfg(test)] mod tests` already exists, add the test inside it; otherwise add the whole block:

```rust
#[cfg(test)]
mod benchmark_params_tests {
    use super::*;

    fn sample_output() -> plan::Output {
        plan::Output {
            width: 256,
            height: 256,
            color_space: plan::ColorSpace::Linear,
            msaa: 4,
        }
    }

    #[test]
    fn benchmark_params_maps_output_and_frames() {
        let p = benchmark_params("sess-1", &sample_output(), 30, 300, false);
        assert_eq!(p.session_id, "sess-1");
        assert_eq!(p.width, 256);
        assert_eq!(p.height, 256);
        assert_eq!(p.msaa, 4);
        assert_eq!(p.color_space, ops::ColorSpace::Linear);
        assert_eq!(p.warmup_frames, 30);
        assert_eq!(p.measured_frames, 300);
        assert!(p.animate_root_transform.is_none());
    }

    #[test]
    fn benchmark_params_sets_animation_when_requested() {
        let p = benchmark_params("s", &sample_output(), 1, 1, true);
        assert!(p.animate_root_transform.is_some());
    }
}
```

> NOTE: `plan::Output` field names above are taken verbatim from `render_params` in this same file (it reads `p.width`, `p.height`, `p.color_space`, `p.msaa`). If the struct gained fields, construct it with `..Default::default()` only if `plan::Output` derives `Default`; otherwise fill the real fields — do not invent names.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vrm-runner benchmark_params`
Expected: FAIL to compile — `cannot find function benchmark_params`.

- [ ] **Step 3: Implement the mapping**

Append to `crates/vrm-runner/src/plan_to_ops.rs` (after `render_params`):

```rust
/// Map a plan's output block to `BenchmarkParams`. Mirrors `render_params`'
/// color-space mapping so the benchmarked scene matches the conformance
/// render. `animate` selects a small vertical root excitation so spring-bone
/// cost is exercised; otherwise the scene is static.
pub fn benchmark_params(
    session_id: &str,
    p: &plan::Output,
    warmup_frames: u32,
    measured_frames: u32,
    animate: bool,
) -> ops::BenchmarkParams {
    let color_space = match p.color_space {
        plan::ColorSpace::Linear => ops::ColorSpace::Linear,
        plan::ColorSpace::Srgb => ops::ColorSpace::Srgb,
    };
    ops::BenchmarkParams {
        session_id: session_id.into(),
        width: p.width,
        height: p.height,
        color_space,
        msaa: p.msaa,
        output_type: ops::OutputType::Color,
        warmup_frames,
        measured_frames,
        animate_root_transform: if animate {
            Some(ops::RootTransformAnimation {
                translation_start: [0.0, 0.0, 0.0],
                translation_end: [0.0, 0.1, 0.0],
            })
        } else {
            None
        },
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p vrm-runner benchmark_params`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-runner/src/plan_to_ops.rs
git commit -m "feat(runner): benchmark_params plan->ops mapping"
```

---

## Task 4: Runner `benchmark` module (drive + compose + write)

**Files:**
- Create: `crates/vrm-runner/src/benchmark.rs`
- Modify: `crates/vrm-runner/src/lib.rs` (register module)
- Test: `crates/vrm-runner/src/benchmark.rs` (unit tests for the pure helpers)

- [ ] **Step 1: Write the failing tests**

Create `crates/vrm-runner/src/benchmark.rs` with ONLY the imports + the two pure helpers' tests first (the implementation comes in Step 3). Put this at the bottom of the file:

```rust
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
                state_changes: 0.0,
                texture_bindings: 1.0,
            }),
            geometry: Some(ops::PerfGeometry {
                triangles: 2,
                vertices: 4,
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
```

> `tempfile` is already a dev-dependency in this workspace (used by other runner tests). If `cargo test` reports it missing for `vrm-runner`, add `tempfile` under `[dev-dependencies]` in `crates/vrm-runner/Cargo.toml` and commit that with this task.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vrm-runner --lib benchmark`
Expected: FAIL to compile — `benchmark` module not declared / `compose_report` not found.

- [ ] **Step 3: Implement the module**

Put this ABOVE the `#[cfg(test)] mod tests` block in `crates/vrm-runner/src/benchmark.rs`:

```rust
//! Drives an adapter through the benchmark op sequence and writes a PerfReport.
//!
//! Scene setup mirrors `execute_plan` (load_vrm → set_camera → set_lighting →
//! set_post_processing) so the measured scene matches the conformance render,
//! then runs `benchmark_plan` (cost preview, logged not gating) +
//! `benchmark_execute`. The runner owns identity (test_id / renderer_name /
//! asset_blake3); the adapter owns the measurement. v1 is observational —
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
}

/// Outcome of a benchmark run: a full report, or Unimplemented when the
/// adapter does not support the op (so callers distinguish "not capable"
/// from "crashed").
pub enum BenchmarkOutcome {
    Report(ops::PerfReport),
    Unimplemented { phase: Option<String> },
}

/// Compose the on-disk report from runner-owned identity + the adapter's
/// measurement. Pure — unit-testable without a subprocess.
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

pub fn run_benchmark(plan: &TestPlan, opts: &BenchmarkOptions) -> Result<BenchmarkOutcome> {
    let asset_path = opts.asset_dir.join(&plan.asset);
    if !asset_path.exists() {
        anyhow::bail!("asset not found: {asset_path}");
    }
    let asset_hash = asset_blake3(&asset_path)?;

    let mut adapter = Adapter::spawn(&opts.adapter_bin, &opts.adapter_args)
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

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

    let _: ops::UnitResult = adapter
        .call("set_camera", camera_params(&session_id, &plan.camera))
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
    let _: ops::UnitResult = adapter
        .call("set_lighting", lighting_params(&session_id, &plan.lighting))
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
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

    // Cost preview — surfaced for logging, not a gate.
    let _preview: ops::BenchmarkPlanResult = adapter
        .call("benchmark_plan", bench_params.clone())
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    let measured: std::result::Result<ops::PerfMeasurement, AdapterError> =
        adapter.call("benchmark_execute", bench_params);

    let outcome = match measured {
        Ok(m) => BenchmarkOutcome::Report(compose_report(
            &plan.id,
            &opts.renderer_name,
            &asset_hash,
            m,
        )),
        Err(AdapterError::Rpc(ref e)) if e.code == -32000 => {
            let phase = e
                .data
                .as_ref()
                .and_then(|d| d.get("phase"))
                .and_then(|v| v.as_str())
                .map(String::from);
            BenchmarkOutcome::Unimplemented { phase }
        }
        Err(e) => return Err(anyhow::anyhow!("adapter error: {e}")),
    };

    let _: ops::UnitResult = adapter
        .call("dispose", ops::DisposeParams { session_id })
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
    adapter
        .shutdown()
        .map_err(|e| anyhow::anyhow!("adapter shutdown: {e}"))?;

    if let BenchmarkOutcome::Report(ref report) = outcome {
        std::fs::create_dir_all(&opts.output_dir)?;
        let out = report_path(&opts.output_dir, &plan.id, &opts.renderer_name);
        std::fs::write(&out, serde_json::to_string_pretty(report)?)?;
    }

    Ok(outcome)
}
```

> The `BenchmarkParams` must derive `Clone` (it does — Task 1) so `bench_params.clone()` works for the preview call. Confirm `vrm_test_plan` is already a dependency of `vrm-runner` (it is — `plan_to_ops.rs` uses `plan::Camera` etc.). The `TestPlan` import path matches what `execute.rs` uses; if `execute.rs` imports it differently (e.g. `use vrm_test_plan as plan;`), follow that exact path.

- [ ] **Step 4: Register the module**

Add to `crates/vrm-runner/src/lib.rs`:

```rust
pub mod benchmark;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p vrm-runner --lib benchmark`
Expected: PASS (2 tests).

- [ ] **Step 6: Lint + format**

Run: `cargo fmt -p vrm-runner && cargo clippy -p vrm-runner --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-runner/src/benchmark.rs crates/vrm-runner/src/lib.rs crates/vrm-runner/Cargo.toml
git commit -m "feat(runner): benchmark module drives op sequence, composes+writes PerfReport"
```

---

## Task 5: `benchmark-execute` CLI subcommand

**Files:**
- Modify: `crates/vrm-runner/src/cli.rs` (add `Cmd::BenchmarkExecute` variant ~after `ConsensusDiff`; add handler arm in `pub fn run` ~after the `ExecuteTestPlan` arm at line 291)
- Test: `crates/vrm-runner/src/cli.rs` (clap parse test in the file's test module)

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod` in `crates/vrm-runner/src/cli.rs` (if none exists, create `#[cfg(test)] mod cli_tests { use super::*; ... }` at end of file):

```rust
    #[test]
    fn benchmark_execute_subcommand_parses_with_defaults() {
        let cli = Cli::try_parse_from([
            "vrm-runner",
            "benchmark-execute",
            "--plan",
            "p.yaml",
            "--adapter-bin",
            "mock",
            "--asset-dir",
            "assets",
            "--output-dir",
            "out",
        ])
        .expect("should parse");
        match cli.command {
            Cmd::BenchmarkExecute {
                warmup_frames,
                measured_frames,
                animate,
                renderer_name,
                ..
            } => {
                assert_eq!(warmup_frames, 30);
                assert_eq!(measured_frames, 300);
                assert!(!animate);
                assert_eq!(renderer_name, "vrm-metal-kit");
            }
            _ => panic!("expected BenchmarkExecute"),
        }
    }
```

> Confirm the top-level parser type is named `Cli` with a `command: Cmd` field (it is — `pub fn run(cli: Cli)` matches `cli.command`). If `Cli` is not `pub` or not `#[derive(Parser)]`-visible to tests, the existing test module already references it; follow that.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vrm-runner benchmark_execute_subcommand`
Expected: FAIL to compile — `no variant BenchmarkExecute`.

- [ ] **Step 3: Add the subcommand variant**

In `crates/vrm-runner/src/cli.rs`, add to the `Cmd` enum (place it after the `ConsensusDiff { ... }` variant):

```rust
    /// Benchmark a plan's scene against one adapter and write a PerfReport
    /// JSON to `<output_dir>/<test_id>_<renderer>.perf.json`. Observational —
    /// no pass/fail. Adapters that don't support benchmarking report
    /// Unimplemented and no file is written.
    BenchmarkExecute {
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
        /// Discarded warm-up frames before measurement (cache/pipeline warm).
        #[arg(long, default_value_t = 30)]
        warmup_frames: u32,
        /// Measured steady-state frames the metrics aggregate over.
        #[arg(long, default_value_t = 300)]
        measured_frames: u32,
        /// Drive a small root-transform animation so spring-bone cost is
        /// exercised (otherwise the scene is static).
        #[arg(long)]
        animate: bool,
        #[arg(long)]
        json: bool,
    },
```

- [ ] **Step 4: Add the handler arm**

In `pub fn run(cli: Cli)` in `crates/vrm-runner/src/cli.rs`, add this arm (after the `Cmd::ExecuteTestPlan { .. } => { ... }` block):

```rust
        Cmd::BenchmarkExecute {
            plan,
            adapter_bin,
            adapter_args,
            asset_dir,
            output_dir,
            renderer_name,
            warmup_frames,
            measured_frames,
            animate,
            json: emit_json,
        } => {
            let plan_value = load_plan(&plan)?;
            let opts = crate::benchmark::BenchmarkOptions {
                adapter_bin,
                adapter_args,
                asset_dir,
                output_dir,
                renderer_name: renderer_name.clone(),
                warmup_frames,
                measured_frames,
                animate,
            };
            let outcome = crate::benchmark::run_benchmark(&plan_value, &opts)?;
            match outcome {
                crate::benchmark::BenchmarkOutcome::Report(report) => {
                    if emit_json {
                        println!("{}", serde_json::to_string_pretty(&report)?);
                    } else {
                        let caps: Vec<String> = report
                            .measurement
                            .capabilities
                            .iter()
                            .map(|c| format!("{c:?}"))
                            .collect();
                        println!(
                            "benchmark {} [{}]: captured {}",
                            report.test_id,
                            renderer_name,
                            caps.join(", ")
                        );
                    }
                }
                crate::benchmark::BenchmarkOutcome::Unimplemented { phase } => {
                    eprintln!(
                        "benchmark: adapter '{}' does not implement benchmark_execute (phase: {})",
                        renderer_name,
                        phase.as_deref().unwrap_or("unknown")
                    );
                }
            }
            Ok(())
        }
```

> `load_plan(&plan)?` is the same plan loader the `ExecuteTestPlan` arm uses (cli.rs:305). `PerfCapability` has no `Display`, so the text branch uses `{c:?}` (Debug) — that prints `Structural`/`Geometry`, which is fine for a human summary.

- [ ] **Step 5: If there is a `describe` catalog, register the subcommand**

`cli.rs` carries a machine-readable command catalog (search for the `"execute-test-plan":` JSON literal, ~line 876). If present, add a sibling entry so the agent surface stays complete:

```rust
                    "benchmark-execute": {
                        "summary": "Benchmark a plan's scene against one adapter; writes a PerfReport JSON (observational, no gate)."
                    },
```

If no such catalog exists in `cli.rs`, skip this step.

- [ ] **Step 6: Run the test + build**

Run: `cargo test -p vrm-runner benchmark_execute_subcommand && cargo build -p vrm-runner`
Expected: PASS; binary builds.

- [ ] **Step 7: Lint + format**

Run: `cargo fmt -p vrm-runner && cargo clippy -p vrm-runner --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add crates/vrm-runner/src/cli.rs
git commit -m "feat(runner): benchmark-execute subcommand"
```

---

## Task 6: End-to-end against the mock (real subprocess)

**Files:**
- Modify: `scripts/smoke.sh` (add a benchmark step using the built mock)

This proves the whole path across a real process boundary (the unit tests stop at the module). It reuses the smoke harness, which already generates an asset and builds the mock.

- [ ] **Step 1: Locate the smoke asset + mock build**

Run: `grep -n "vrm-mock-renderer\|emit-default\|execute-test-plan\|OUT\b\|cargo run -p vrm-runner" scripts/smoke.sh | head -30`
Expected: shows where the mock binary path and the generated asset dir / plan are defined (variable names like `$OUT`, `$ASSET_DIR`, `$PLAN`, `$MOCK`). Use those exact variables in Step 2 — do not hardcode paths.

- [ ] **Step 2: Add the benchmark step**

After the existing `execute-test-plan` / `diff` step in `scripts/smoke.sh`, add (substituting the real variable names found in Step 1):

```bash
echo "==> Benchmark (mock, observational)"
cargo run -q -p vrm-runner -- benchmark-execute \
    --plan "$PLAN" \
    --adapter-bin "$MOCK" \
    --asset-dir "$ASSET_DIR" \
    --output-dir "$OUT" \
    --renderer-name mock \
    --warmup-frames 2 \
    --measured-frames 4 \
    --json

PERF_JSON="$OUT/${SMOKE_ID:-smoke}_mock.perf.json"
if [ ! -f "$PERF_JSON" ]; then
    echo "smoke: expected perf report at $PERF_JSON" >&2
    exit 1
fi
# Assert the report carries the structural + geometry capabilities.
grep -q '"structural"' "$PERF_JSON" || { echo "smoke: perf report missing structural block" >&2; exit 1; }
grep -q '"triangles"' "$PERF_JSON" || { echo "smoke: perf report missing geometry block" >&2; exit 1; }
echo "    perf report OK: $PERF_JSON"
```

> The per-test report filename is `<test_id>_<renderer>.perf.json` (see `benchmark::report_path`). `$SMOKE_ID` must match the `--id` used by the smoke `emit-default` call; if smoke uses a different variable for the test id, use that one.

- [ ] **Step 3: Run smoke end-to-end**

Run: `scripts/smoke.sh`
Expected: completes with `perf report OK: ...` printed and exit 0.

- [ ] **Step 4: Commit**

```bash
git add scripts/smoke.sh
git commit -m "test(smoke): mock benchmark E2E asserts PerfReport written"
```

---

## Task 7: Aggregator script `scripts/perf-report.sh`

**Files:**
- Create: `scripts/perf-report.sh`

Mirrors `scripts/consensus-report.sh` structure (ROOT detection, `set -euo pipefail`, `jq`, `REPORT_OUT` override). Merges all `*.perf.json` under a directory into a single matrix and computes a VMK-vs-golden-ref structural delta.

- [ ] **Step 1: Write the script**

Create `scripts/perf-report.sh`:

```bash
#!/usr/bin/env bash
#
# Aggregate per-test PerfReport JSON files (written by
# `vrm-runner benchmark-execute`) into a single corpus-wide report at
# goldens-cache/perf-report.json, plus a VMK-vs-golden-ref structural delta
# summary on stdout. Observational — no pass/fail.
#
# Usage:
#   scripts/perf-report.sh <perf-json-dir> [reference-renderer]
#
#   <perf-json-dir>      Directory containing <id>_<renderer>.perf.json files.
#   [reference-renderer] Renderer name used as the "familiar" baseline for the
#                        structural delta. Default: univrm (golden reference).
#
# Env:
#   REPORT_OUT  Override output path. Default: goldens-cache/perf-report.json
#   SUBJECT     Renderer to compare against the reference. Default: vrm-metal-kit

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
cd "$ROOT"

PERF_DIR="${1:-}"
REFERENCE="${2:-univrm}"
SUBJECT="${SUBJECT:-vrm-metal-kit}"
REPORT_OUT="${REPORT_OUT:-$ROOT/goldens-cache/perf-report.json}"

if [ -z "$PERF_DIR" ] || [ ! -d "$PERF_DIR" ]; then
    echo "perf-report: pass a directory of *.perf.json files" >&2
    echo "             usage: scripts/perf-report.sh <perf-json-dir> [reference-renderer]" >&2
    exit 2
fi

command -v jq >/dev/null 2>&1 || { echo "perf-report: jq is required" >&2; exit 2; }

mkdir -p "$(dirname "$REPORT_OUT")"

# Collect every report into a JSON array under `reports`.
mapfile -t FILES < <(find "$PERF_DIR" -name '*.perf.json' | sort)
if [ "${#FILES[@]}" -eq 0 ]; then
    echo "perf-report: no *.perf.json files under $PERF_DIR" >&2
    exit 1
fi

jq -s '{ reports: . }' "${FILES[@]}" > "$REPORT_OUT.tmp"

# Compute the structural delta: for each test_id present for BOTH subject and
# reference, percentage difference of draw_calls (subject vs reference).
jq --arg subject "$SUBJECT" --arg reference "$REFERENCE" '
  .reports as $r
  | [ $r[] | select(.renderer_name == $subject) ] as $subj
  | [ $r[] | select(.renderer_name == $reference) ] as $ref
  | .structural_delta = [
      $subj[] as $s
      | ($ref[] | select(.test_id == $s.test_id)) as $b
      | select($s.structural != null and $b.structural != null
               and $b.structural.draw_calls != 0)
      | {
          test_id: $s.test_id,
          subject: $subject,
          reference: $reference,
          subject_draw_calls: $s.structural.draw_calls,
          reference_draw_calls: $b.structural.draw_calls,
          draw_calls_pct: ((($s.structural.draw_calls - $b.structural.draw_calls)
                            / $b.structural.draw_calls) * 100)
        }
    ]
' "$REPORT_OUT.tmp" > "$REPORT_OUT"
rm -f "$REPORT_OUT.tmp"

echo "==> perf-report written: $REPORT_OUT"
echo "    reports: ${#FILES[@]}"
echo "    structural delta ($SUBJECT vs $REFERENCE, draw_calls %):"
jq -r '.structural_delta[] | "      \(.test_id): \(.draw_calls_pct | . * 100 | round / 100)%"' "$REPORT_OUT" \
    || echo "      (no overlapping test_ids between $SUBJECT and $REFERENCE)"
```

- [ ] **Step 2: Make it executable**

Run: `chmod +x scripts/perf-report.sh`

- [ ] **Step 3: Verify with a fixture**

Run:

```bash
TMP=$(mktemp -d)
cat > "$TMP/a_mock.perf.json" <<'JSON'
{"test_id":"a","renderer_name":"mock","asset_blake3":"blake3:1",
 "protocol":{"warmup_frames":2,"measured_frames":4,"animated":false},
 "structural":{"draw_calls":2.0,"state_changes":0.0,"texture_bindings":1.0},
 "geometry":{"triangles":2,"vertices":4},
 "host":{"os":"mock","os_version":"0","gpu_vendor":"none","gpu_model":"cpu","driver_version":"0","build_flags":""},
 "capabilities":["structural","geometry"]}
JSON
cat > "$TMP/a_univrm.perf.json" <<'JSON'
{"test_id":"a","renderer_name":"univrm","asset_blake3":"blake3:1",
 "protocol":{"warmup_frames":2,"measured_frames":4,"animated":false},
 "structural":{"draw_calls":1.0,"state_changes":0.0,"texture_bindings":1.0},
 "geometry":{"triangles":2,"vertices":4},
 "host":{"os":"linux","os_version":"6","gpu_vendor":"nv","gpu_model":"x","driver_version":"1","build_flags":""},
 "capabilities":["structural","geometry"]}
JSON
REPORT_OUT="$TMP/out.json" SUBJECT=mock scripts/perf-report.sh "$TMP" univrm
jq -e '.structural_delta[0].draw_calls_pct == 100' "$TMP/out.json"
```

Expected: prints `==> perf-report written`, a `a: 100%` delta line, and the final `jq -e` exits 0 (mock has 2 draw calls vs univrm's 1 → +100%).

- [ ] **Step 4: Commit**

```bash
git add scripts/perf-report.sh
git commit -m "feat(scripts): perf-report.sh aggregates PerfReports + structural delta"
```

---

## Task 8: Documentation

**Files:**
- Modify: `docs/operation-contract.md`
- Modify: `docs/methodology.md`

- [ ] **Step 1: Document the ops in `docs/operation-contract.md`**

Add a section (place it alongside the other op definitions; match the file's existing heading style):

```markdown
### `benchmark_plan` / `benchmark_execute` (performance metrics, v1 — observational)

Measures per-renderer performance over a plan's scene. Two ops:

- `benchmark_plan(BenchmarkParams) → BenchmarkPlanResult` — cheap cost preview
  (`estimated_frames`, `estimated_seconds`, `scene_summary`); renders nothing.
- `benchmark_execute(BenchmarkParams) → PerfMeasurement` — renders
  `warmup_frames` discarded frames, then `measured_frames` steady-state frames,
  aggregating metrics.

**Naming exception:** these use noun_verb (`benchmark_*`) rather than the
contract's usual `plan_*` / `execute_*` prefix, by maintainer directive, so the
pair groups under the `benchmark` noun.

**Identity is runner-owned.** `benchmark_execute` returns a `PerfMeasurement`
(metrics + `host` + `capabilities`). The runner composes the on-disk
`PerfReport` by adding `test_id`, `renderer_name`, and `asset_blake3`
(`blake3:<hex>` of the `.vrm`), flattened with the measurement — mirroring the
"BLAKE3 centralized in the runner" rule.

**Capabilities, not per-field Unimplemented.** Each measurement block
(`timing` / `structural` / `geometry` / `resources`) is nullable; the
`capabilities` array lists what was populated. A structural-only adapter omits
`timing`. Adapters with no benchmark support at all return
`-32000 Unimplemented` with `data: { phase: "perf-v1" }`.

`timing` is hardware-dependent (compare same-host only); `structural` /
`geometry` are hardware-independent and are the cross-renderer comparison axis.
See `docs/methodology.md`, "Benchmark protocol".
```

- [ ] **Step 2: Document the protocol in `docs/methodology.md`**

Add a section:

```markdown
## Benchmark protocol (performance metrics, observational v1)

Performance is a conformance axis: a renderer should sit inside an expected,
familiar envelope. Because adapters run on different runtimes/hardware, metrics
split into two layers — **structural** (draw calls, state changes, geometry,
count-based memory: hardware-independent, the cross-renderer "familiar" axis)
and **timing** (frame-time percentiles, FPS, byte-valued memory:
hardware-dependent, same-host only).

Pins:

- **Scene:** the plan's `output` block (width/height/color_space/msaa) and the
  plan's camera/lighting/post. `tone_mapping: none` consistent with the
  MToon-math pins.
- **Frames:** 30 warmup frames discarded (shader/pipeline compile, cache warm),
  then 300 measured steady-state frames (both configurable;
  `--warmup-frames` / `--measured-frames`). VMK's internal baseline used 500;
  300 is the suite default.
- **Excitation:** static by default; `--animate` drives a small root
  translation so spring-bone cost is exercised (`protocol.animated = true`).
- **Determinism:** structural and geometry counts are deterministic and
  host-independent — the comparison layer. Timing is non-deterministic and
  host-bound.

### Reference machine (timing anchor)

VMK timing is anchored to a single declared host: **Apple M4 Max Mac Studio,
128 GB RAM**. Xcode Cloud runs count as equivalent only on matching silicon.
Timing numbers are interpreted same-host only; structural/geometry/count-based
memory carry across renderers.

v1 is **observational**: collect and report (`scripts/perf-report.sh` →
`goldens-cache/perf-report.json`, with a VMK-vs-golden-ref structural delta).
No pass/fail gate — budgets are a deliberate later phase informed by v1 data.
```

- [ ] **Step 3: Verify the docs reference real identifiers**

Run: `grep -n "benchmark_plan\|benchmark_execute\|PerfMeasurement\|PerfReport\|perf-v1\|M4 Max" docs/operation-contract.md docs/methodology.md`
Expected: matches in both files; identifiers match the Rust types from Task 1 and the `phase: "perf-v1"` label from Task 2's design (note: the mock returns Unimplemented only for truly-unsupported adapters — the mock DOES support it, so this label is for future adapters).

- [ ] **Step 4: Commit**

```bash
git add docs/operation-contract.md docs/methodology.md
git commit -m "docs: benchmark ops contract + benchmark protocol/methodology"
```

---

## Task 9: Workspace gate

- [ ] **Step 1: Full workspace build + test**

Run: `cargo build --workspace && cargo test --workspace`
Expected: all green.

- [ ] **Step 2: Format + clippy gate (CI parity)**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: no diff, zero warnings.

- [ ] **Step 3: Final commit (only if Step 1/2 produced fmt fixes)**

```bash
git add -A
git commit -m "chore: fmt/clippy pass for benchmark foundation" || echo "nothing to commit"
```

---

## Follow-on plans (NOT this slice)

Each is a separate spec→plan→implement cycle, gated on its toolchain:

1. **VMK adapter** — full `PerfReport` (timing + structural + geometry +
   resources) via VRMMetalKit's `PerformanceTracker` driven through the
   existing `drawOffscreenHeadless` loop; `phase: "perf-v1"` removed once real.
2. **three-vrm adapter** — structural + geometry + host memory + CPU timing via
   `renderer.info.render.*`, `performance.now()`, `performance.memory`.
3. **UniVRM adapter** — structural + geometry + host memory via
   `ProfilerRecorder` + `Profiler.GetTotalAllocatedMemoryLong`; extend the
   batch result schema.
4. **Phase 2 — budgets & gating** — once baselines exist: a familiar-band
   tolerance on structural metrics and an absolute timing budget on the
   reference machine, with a perf verdict mirroring the SSIM `overall_passed`
   pattern.
```
