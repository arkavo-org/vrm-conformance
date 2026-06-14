# Performance metrics across implementations — design (observational v1)

- **Date:** 2026-06-14
- **Status:** Design approved; pending spec review → implementation plan
- **Author:** Paul Flynn (with Claude Code)
- **Related:** [`docs/operation-contract.md`](../../operation-contract.md), [`docs/methodology.md`](../../methodology.md), [`rfcs/0004-render-sequence-op.md`](../../../rfcs/0004-render-sequence-op.md), [`docs/findings.md`](../../findings.md)

## Problem

The suite is conformance/quality-focused and captures **no performance data** today. What exists is incidental timing scaffolding, not a designed metric:

- `render_sequence` results carry `duration_seconds` + `frame_hz_achieved` and per-frame `timestamp_seconds`, but these are *logical* values computed from `frame_hz` for temporal alignment — **not wall-clock measurements**.
- Batch `ResultEntry` has an `Option<f32> render_seconds` field, present in the struct but not a contract-level metric.
- The goldens manifest's `SubmissionMetadata` has the right *anchoring* fields (`os`, `os_version`, `gpu_vendor`, `gpu_model`, `driver_version`, `build_flags`) but **no perf fields**.

VMK is the outlier: VRMMetalKit upstream (pinned `0.20.1`) has mature benchmark infra — a `PerformanceTracker` (draw calls, texture/pipeline-state changes, tri/vert counts, phase timing, FPS), a `--perf-test` CLI flag with JSON export and p50/p95/p99 frame-time percentiles, plus `gpudebug` GPU-trace and Instruments — but **none of it is wired back to the runner**. The adapter returns only `{output_path, actual_color_space}`, same as every other adapter.

## Goal & framing

Treat performance as a **conformance axis**: VMK should sit inside an *expected, familiar* performance envelope. Because every adapter runs on a different runtime and (usually) different hardware, an absolute cross-renderer "who's faster" ranking is as methodologically indefensible here as pixel-exact comparison is for toon shading. The metric therefore splits into two layers:

- **Structural layer** — draw calls, state changes, geometry counts, count-based memory. *Hardware-independent and deterministic.* For the same asset, these should be similar across renderers, so this is the cross-renderer **"familiar" comparison axis**. UniVRM (the de-facto golden reference) and three-vrm calibrate the band.
- **Timing layer** — frame-time p50/p95/p99, FPS, peak memory bytes, load/first-frame ms. *Hardware-dependent.* Only interpreted **same-host**; VMK gets a declared reference machine.

**v1 is observational.** Collect, aggregate, and report VMK-vs-peers. No pass/fail gate, no budgets, no perf-diff threshold engine — there is zero baseline data to set defensible thresholds against. Budgets/gating are a deliberate future phase informed by v1 data.

## Non-goals (YAGNI for v1)

- No pass/fail gate, no budgets, no perf-diff threshold engine.
- No Godot timing wiring; no babylon wiring (L3-blocked upstream on VRM 1.0 support).
- No GPU-trace / Instruments artifact capture (the op contract reserves a `blake3:` ref slot for it later).
- No CI gating — benchmarks run locally / on Xcode Cloud, matching how VMK runtime coverage already works (CI builds VMK debug but cannot run macOS-26-linked binaries).
- No historical time-series store — per-run JSON plus a committed report pointer, like the manifest.
- No new asset corpus — benchmarks run over the *existing* test corpus.

## Architecture — dedicated benchmark op

Per the agent-first surface contract, performance measurement is a first-class operation on the uniform surface, defined once in `crates/vrm-ops/` and reachable through both CLI (`--json`) and JSON-RPC stdio. Two new ops:

- `benchmark_plan(params) → BenchmarkPlan` — cheap cost preview: `{ estimated_frames, estimated_seconds, scene_summary }`. Honors the contract rule that expensive ops decouple plan from execute so agents can preview cost before committing.
- `benchmark_execute(params) → PerfReport` — runs the measured loop and returns the report.

**Naming exception (documented):** the existing contract convention is `plan_*` / `execute_*`, and existing ops are verb_noun (`load_vrm`, `render_sequence`). Per maintainer directive, the benchmark ops use **noun_verb** (`benchmark_plan` / `benchmark_execute`) so the plan/execute pair groups under the `benchmark` noun. This is a deliberate exception recorded here and in `docs/operation-contract.md`; it does not change naming of any existing op.

Surface obligations (non-negotiable per the contract):

- Define op types + JSON Schema emission + JSON-RPC dispatch in `crates/vrm-ops/src/tools.rs`; register both ops in the `describe` catalog.
- NDJSON progress events on **stderr** during `benchmark_execute` (frame counter: warmup N/30, measured M/300). Stdout is reserved for the structured `PerfReport`.
- No binary payloads in stdout. Any future trace artifact is passed as a file path + **BLAKE3 content ref** (`blake3:<64-hex>`).
- Adapters lacking support return the standard `Unimplemented` error (`-32000`, `data: { phase: "v1.x" }`).

## The metric — benchmark protocol

Added as a section to `docs/methodology.md`. Pins (so runs are reproducible and comparable):

- **Scene:** fixed, taken from the asset's generated `test.yaml` (camera / lighting / post). `tone_mapping: none` for consistency with the MToon-math methodology pins. MSAA and shadow settings follow the asset's existing test conventions.
- **Frames:** **30 warmup frames discarded** (shader/pipeline compilation, cache warm), then **300 measured steady-state frames** (configurable via params; recorded in `protocol`). VMK's internal baseline used 500 — 300 is the suite default; raise per-run when noise demands it.
- **Excitation:** static scene by default. Optional `animate_root_transform` to exercise spring-bone cost (reuses the swing-sweep excitation pattern), recorded as `protocol.animated = true`.
- **Timing:** per-frame wall-clock → `frame_time_ms { p50, p95, p99 }` + `fps_mean`. GPU+CPU submit-to-complete where the platform exposes it; CPU/main-thread frame time as the documented fallback (e.g. three-vrm reports JS/CPU frame time, flagged in `capabilities`).
- **Structural:** `draw_calls`, `state_changes`, `texture_bindings` — per-frame **means** over the measured window.
- **Geometry:** `triangles`, `vertices` submitted per frame (steady-state).
- **Resources:** `peak_memory_bytes` (+ `memory_kind: gpu | host`), `load_ms` (load_vrm wall-clock), `first_frame_ms` (load → first present).
- **Determinism note (methodology):** timing is non-deterministic and host-bound; structural and geometry counts **are** deterministic and host-independent, and are the cross-renderer comparison layer even though v1 does not gate on them.

### Reference machine

VMK timing is anchored to a single declared host: **Apple M4 Max Mac Studio, 128 GB RAM** (the maintainer's machine; Xcode Cloud runs treated as equivalent only when on matching silicon). Recorded in `docs/methodology.md` and embedded in every `PerfReport.host`. Timing comparisons are valid **same-host only**; structural / geometry / count-based memory are host-independent and carry across renderers.

## PerfReport schema

Each measurement block is nullable, and a `capabilities` list states what was actually captured — cleaner than per-field `Unimplemented` for adapters with partial support (e.g. structural-only). Schema emitted from `crates/vrm-ops/` alongside the op types.

```
PerfReport {
  test_id:        String,
  renderer_name:  String,
  asset_blake3:   String,                 // blake3:<64-hex> of the .vrm
  protocol {
    warmup_frames:   u32,                 // 30
    measured_frames: u32,                 // 300
    animated:        bool,
  },
  timing?: {                              // null when not captured
    frame_time_ms: { p50: f32, p95: f32, p99: f32 },
    fps_mean:      f32,
    clock:         "gpu_cpu" | "cpu",     // documents the fallback used
  },
  structural?: {                          // per-frame means
    draw_calls:       f32,
    state_changes:    f32,
    texture_bindings: f32,
  },
  geometry?: {
    triangles: u64,
    vertices:  u64,
  },
  resources?: {
    peak_memory_bytes: u64,
    memory_kind:       "gpu" | "host",
    load_ms:           f32,
    first_frame_ms:    f32,
  },
  host: {                                 // reuse SubmissionMetadata fields
    os: String, os_version: String,
    gpu_vendor: String, gpu_model: String,
    driver_version: String, build_flags: String,
  },
  capabilities: [ "timing" | "structural" | "geometry" | "resources" ],
}
```

## Adapter capability matrix

| Adapter | timing | structural | geometry | resources | source |
|---|---|---|---|---|---|
| **VMK** (Swift/Metal) | ✅ | ✅ | ✅ | ✅ (gpu) | `PerformanceTracker` driven through the existing `drawOffscreenHeadless` loop; `CACurrentMediaTime` for load/first-frame; Metal allocation for peak memory |
| **three-vrm** (TS/Playwright) | ✅ (cpu) | ✅ | ✅ | host mem | `renderer.info.render.{calls,triangles}`, `renderer.info.memory`, `performance.now()`, `performance.memory.usedJSHeapSize` |
| **UniVRM** (C#/Unity) | — | ✅ | ✅ | host mem | `ProfilerRecorder` ("Draw Calls Count", "SetPass Calls Count", "Triangles Count", "Vertices Count"), `Profiler.GetTotalAllocatedMemoryLong` |
| **mock** (Rust/CPU) | — | ✅ (deterministic) | ✅ | — | CPU-rasterizer counts; provides a no-GPU end-to-end path for the op + schema round-trip |
| **godot** | `Unimplemented` | — | — | — | deferred to a later phase |
| **babylon** | `Unimplemented` | — | — | — | L3-blocked upstream |

Scope decision: VMK is fully instrumented; UniVRM and three-vrm contribute the hardware-independent structural/geometry/memory layer that turns "familiar" into a measured band rather than a guessed budget. The mock renderer implements the structural+geometry path deterministically so the op, schema, and runner aggregation can be exercised without a GPU.

## Runner + reporting

- **Subcommand:** `vrm-runner benchmark-execute --plan <plan.yaml> --adapter-bin <bin> [--adapter-args ...] --asset-dir <dir> --output-dir <dir> --renderer-name <name> [--json]`. Runs `benchmark_plan` (preview, surfaced in progress/JSON) then `benchmark_execute`, and writes a per-test `PerfReport` JSON under `<output_dir>/`. BLAKE3 is centralized in the runner (same pattern as `render_sequence` rehashing).
- **Aggregator:** `scripts/perf-report.sh` (mirrors `scripts/consensus-report.sh`) → `goldens-cache/perf-report.json`: a corpus × renderer matrix of `PerfReport`s **plus a VMK-vs-golden-ref structural delta summary** — the "familiar band," reported not gated. `goldens-cache/` stays gitignored.
- **Findings:** when VMK shows a structural outlier vs UniVRM (e.g. anomalous draw-call count for an asset), that is logged in `docs/findings.md` — findings are the deliverable for cross-renderer divergence.
- **Site:** a perf-table view reading `perf-report.json` is **optional/deferred** in v1 (the data lands first; presentation follows).

## File / crate touch list

- `crates/vrm-ops/src/tools.rs` — `benchmark_plan` / `benchmark_execute` op types, `BenchmarkParams`, `BenchmarkPlan`, `PerfReport` + nested blocks; JSON Schema emission; `describe` registration; JSON-RPC dispatch.
- `docs/operation-contract.md` — document both ops, the `PerfReport` envelope, the noun_verb naming exception, and the reserved `blake3:` trace slot.
- `docs/methodology.md` — benchmark protocol section (frames, scene pins, reference machine, deterministic-vs-timing layer).
- `crates/vrm-runner/` — new `benchmark.rs` (or `execute_benchmark.rs`) + `benchmark-execute` subcommand; per-test report write; BLAKE3 centralization.
- `crates/vrm-mock-renderer/` — implement `benchmark_plan` / `benchmark_execute` (deterministic structural + geometry) for end-to-end coverage.
- `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift` — full `PerfReport` via `PerformanceTracker`.
- `adapters/three-vrm/src/operations.ts` — structural + geometry + host memory + CPU timing.
- `adapters/univrm/` — structural + geometry + host memory (extend the batch result schema).
- `scripts/perf-report.sh` — aggregator → `goldens-cache/perf-report.json`.
- `site/` — optional/deferred perf-table view.

## Open questions / future phases

- **Phase 2 — budgets & gating:** once v1 baselines exist, define a "familiar band" tolerance on structural metrics (VMK within ±X% of golden-ref for the same asset) and an absolute timing budget on the reference machine; add a perf assertion / verdict (mirroring the SSIM threshold + `overall_passed` pattern).
- **Godot timing** and **babylon** instrumentation once those adapters are ready.
- **GPU-trace artifact capture** (VMK `gpudebug` / Instruments) surfaced as a `blake3:` ref for deep-dive triage.
- **Site presentation** of the perf matrix.
