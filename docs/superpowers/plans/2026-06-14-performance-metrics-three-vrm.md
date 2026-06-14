# Performance Metrics — three-vrm Adapter Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Checkbox steps. All work on branch `feat/perf-metrics-three-vrm`; verify branch first in every task.

**Goal:** Implement `benchmark_plan` / `benchmark_execute` in the three-vrm adapter, returning a `PerfMeasurement` with the metrics three.js can honestly provide — CPU frame-time percentiles + FPS, `draw_calls`, `triangles`, and host JS-heap memory — and make the un-instrumentable structural/geometry sub-fields optional in the Rust contract so partial renderers don't fake data.

**Architecture:** three-vrm runs three.js in Playwright headless Chromium. The adapter (`adapters/three-vrm/`) hand-builds snake_case JSON results; a single browser `state` holds the renderer/scene/camera. A new browser-side `__benchmarkRender` runs warmup + measured frames, timing each `renderer.render()` with `performance.now()` and reading `renderer.info.render.calls/triangles` (with `info.autoReset=false`), plus `performance.memory.usedJSHeapSize`. Node computes percentiles and assembles the `PerfMeasurement`. The Rust `PerfStructural`/`PerfGeometry` types gain optional sub-fields so three-vrm can omit what it can't measure.

**Tech Stack:** TypeScript / three.js / three-vrm / Playwright (headless Chromium, cached locally). Node 26. Rust runner already supports `benchmark-execute`.

**Honesty principle (load-bearing):** three.js does NOT expose per-frame `state_changes`, `texture_bindings`, or `vertices`. We make those three Rust fields `Option` and three-vrm OMITS them (never emits `0` as a stand-in). `draw_calls` and `triangles` are required and three-vrm provides both. The cross-renderer "familiar" diff (`perf-report.sh`) only uses `draw_calls`, so this is fully compatible.

**Contract reminder:** `benchmark_execute` receives `BenchmarkParams` (`session_id`, `width`, `height`, `color_space`, `msaa`, `output_type`, `warmup_frames`, `measured_frames`, optional `animate_root_transform`) and returns a `PerfMeasurement` (NOT a handle scheme). `benchmark_plan` receives the same and returns `{estimated_frames, estimated_seconds, scene_summary}`.

**Design doc:** [`docs/superpowers/specs/2026-06-14-performance-metrics-design.md`](../specs/2026-06-14-performance-metrics-design.md). Prior slices: foundation + VMK plans in this dir.

---

## Task 1: Make un-instrumentable structural/geometry sub-fields optional (Rust)

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs` (`PerfStructural`, `PerfGeometry`, the `benchmark_tests` constructors)
- Modify: `crates/vrm-mock-renderer/src/handlers.rs` (`mock_measurement` wraps the now-optional fields in `Some`)

- [ ] **Step 0: Verify branch** = `feat/perf-metrics-three-vrm`.

- [ ] **Step 1: Change the types.** In `crates/vrm-ops/src/tools.rs`, change `PerfStructural` and `PerfGeometry` so the sub-fields three.js cannot measure are optional (keep `draw_calls` and `triangles` required — every renderer has them):
```rust
/// Hardware-independent structural layer — per-frame means over the measured
/// window. The cross-renderer "familiar" comparison axis. `draw_calls` is
/// always present; `state_changes`/`texture_bindings` are optional because some
/// renderers (e.g. three.js) do not instrument them — they are OMITTED, never
/// reported as zero.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PerfStructural {
    pub draw_calls: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_changes: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture_bindings: Option<f32>,
}

/// Hardware-independent geometry layer — per-frame submission counts.
/// `triangles` is always present; `vertices` is optional (three.js exposes no
/// per-frame vertex counter, so it is omitted rather than faked).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerfGeometry {
    pub triangles: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertices: Option<u64>,
}
```

- [ ] **Step 2: Fix the `benchmark_tests` constructors** in `tools.rs` (the `perf_report_flattens_...` test builds these literals). Wrap the now-optional fields in `Some(...)`:
```rust
                structural: Some(PerfStructural {
                    draw_calls: 1.0,
                    state_changes: Some(0.0),
                    texture_bindings: Some(1.0),
                }),
                geometry: Some(PerfGeometry {
                    triangles: 2,
                    vertices: Some(4),
                }),
```
Add one assertion to that test confirming an omitted optional sub-field is absent from JSON — build a `PerfStructural { draw_calls: 5.0, state_changes: None, texture_bindings: None }`, serialize, and assert the JSON has `draw_calls` but not `state_changes`:
```rust
    #[test]
    fn perf_structural_omits_unmeasured_subfields() {
        let s = PerfStructural { draw_calls: 5.0, state_changes: None, texture_bindings: None };
        let v: serde_json::Value = serde_json::to_value(&s).unwrap();
        assert_eq!(v["draw_calls"], 5.0);
        assert!(v.get("state_changes").is_none());
        assert!(v.get("texture_bindings").is_none());
        let g = PerfGeometry { triangles: 9, vertices: None };
        let gv: serde_json::Value = serde_json::to_value(&g).unwrap();
        assert_eq!(gv["triangles"], 9);
        assert!(gv.get("vertices").is_none());
    }
```

- [ ] **Step 3: Update the mock** in `crates/vrm-mock-renderer/src/handlers.rs` (`mock_measurement`) — it deterministically emits all fields, so wrap the optional ones in `Some`:
```rust
        structural: Some(ops::PerfStructural {
            draw_calls: 1.0,
            state_changes: Some(0.0),
            texture_bindings: Some(1.0),
        }),
        geometry: Some(ops::PerfGeometry {
            triangles: 2,
            vertices: Some(4),
        }),
```

- [ ] **Step 4: Build + test + lint.**
```
cd /Users/arkavo/Projects/vrm-conformance
cargo test -p vrm-ops -p vrm-mock-renderer benchmark 2>&1 | tail -8
cargo test -p vrm-ops perf_structural_omits 2>&1 | tail -4
cargo fmt --all -- --check && cargo clippy -p vrm-ops -p vrm-mock-renderer --all-targets -- -D warnings 2>&1 | tail -1
```
Expected: green. Note: the VMK Swift adapter emits all sub-fields → they deserialize as `Some` (no Swift change needed). Paste results.

- [ ] **Step 5: Commit.**
```bash
git add crates/vrm-ops/src/tools.rs crates/vrm-mock-renderer/src/handlers.rs
git commit -m "feat(ops): optional structural/geometry sub-fields for partial renderers"
```

---

## Task 2: three-vrm benchmark handlers

**Files:**
- Modify: `adapters/three-vrm/src/renderer-host.html` (browser-side `__benchmarkRender` + load-time capture)
- Modify: `adapters/three-vrm/src/browser-session.ts` (`benchmarkExecute`/`benchmarkPlan` methods; `--enable-precise-memory-info` launch flag; load_ms capture)
- Modify: `adapters/three-vrm/src/operations.ts` (two dispatch cases)
- Modify: `adapters/three-vrm/test/contract.test.ts` (a benchmark contract test)

Read `handleRender`/`handleRenderSequence` in `operations.ts`/`browser-session.ts` and `window.__render`/`window.__renderSequence` in `renderer-host.html` first; mirror their session handling, param parsing, and snake_case result construction.

- [ ] **Step 0: Verify branch** = `feat/perf-metrics-three-vrm`.

- [ ] **Step 1: Browser-side `__benchmarkRender`.** Add to `renderer-host.html` (alongside `__render`/`__renderSequence`). It runs warmup + measured frames, times each render, reads `info.render` counters, and samples the JS heap. It probes for GPU timing and reports `clock` accordingly (expected `"cpu"` in headless Chromium):
```javascript
window.__benchmarkRender = function (params) {
  ensureRenderer(params.width, params.height);
  // (mirror __render's color-space setup here)
  const r = state.renderer;
  const warmup = params.warmup_frames | 0;
  const measured = params.measured_frames | 0;
  const animated = !!params.animate_root_transform;
  // (if animated: read translation_start/translation_end like __renderSequence)

  r.info.autoReset = false;

  let firstFrameMs = 0;
  let capturedFirst = false;
  const renderOnce = function (tNorm) {
    // if animated: set root translation = lerp(start, end, tNorm); (mirror __renderSequence)
    r.info.reset();
    const t0 = performance.now();
    if (state.vrm && state.vrm.update) state.vrm.update(1 / 60);
    r.render(state.scene, state.camera);
    const ms = performance.now() - t0;
    if (!capturedFirst) { firstFrameMs = ms; capturedFirst = true; }
    return ms;
  };

  for (let i = 0; i < warmup; i++) renderOnce(0);

  const frameTimes = [];
  let drawCallsSum = 0, trianglesSum = 0;
  for (let i = 0; i < measured; i++) {
    const tNorm = measured > 1 ? i / (measured - 1) : 0;
    frameTimes.push(renderOnce(tNorm));
    drawCallsSum += r.info.render.calls;
    trianglesSum += r.info.render.triangles;
  }
  r.info.autoReset = true;

  const jsHeap = (performance.memory && performance.memory.usedJSHeapSize) || null;
  return {
    frame_times_ms: frameTimes,
    draw_calls_mean: measured ? drawCallsSum / measured : 0,
    triangles_mean: measured ? trianglesSum / measured : 0,
    js_heap_bytes: jsHeap,
    first_frame_ms: firstFrameMs,
    animated: animated,
    gpu_model: (function () {
      try {
        const gl = r.getContext();
        const dbg = gl.getExtension("WEBGL_debug_renderer_info");
        return dbg ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : "headless-chromium";
      } catch (e) { return "headless-chromium"; }
    })(),
  };
};
```

- [ ] **Step 2: Capture load time.** In `browser-session.ts`'s `loadVrm` (whatever loads the asset into the browser), wrap the load with `const t = performance.now(); ... ; this.loadMs = performance.now() - t;` (Node-side `performance.now()` is fine via `perf_hooks` — check if already imported; Node's global `performance` is available in Node 26). Store `private loadMs = 0;` on `BrowserSession`. If the load is split, measure the outermost span in `loadVrm`.

- [ ] **Step 3: `benchmarkPlan` + `benchmarkExecute` on `BrowserSession`.**
```typescript
async benchmarkPlan(p: {
  warmup_frames?: number; measured_frames?: number; width?: number; height?: number;
}): Promise<{ estimated_frames: number; estimated_seconds: number; scene_summary: string }> {
  const warmup = p.warmup_frames ?? 30;
  const measured = p.measured_frames ?? 300;
  const total = warmup + measured;
  return {
    estimated_frames: total,
    estimated_seconds: total / 60.0,
    scene_summary: `three-vrm ${p.width ?? 0}x${p.height ?? 0}`,
  };
}

async benchmarkExecute(p: {
  width?: number; height?: number; color_space?: string;
  warmup_frames?: number; measured_frames?: number; animate_root_transform?: unknown;
}): Promise<unknown> {
  if (!this.page) throw new Error("no session loaded");
  const warmup = p.warmup_frames ?? 30;
  const measured = p.measured_frames ?? 300;
  const raw = await this.page.evaluate(
    (q) => (window as any).__benchmarkRender(q),
    {
      width: p.width ?? 256, height: p.height ?? 256,
      color_space: p.color_space ?? "Linear",
      warmup_frames: warmup, measured_frames: measured,
      animate_root_transform: p.animate_root_transform ?? null,
    },
  ) as {
    frame_times_ms: number[]; draw_calls_mean: number; triangles_mean: number;
    js_heap_bytes: number | null; first_frame_ms: number; animated: boolean; gpu_model: string;
  };

  // percentiles (Node side)
  const sorted = [...raw.frame_times_ms].sort((a, b) => a - b);
  const pct = (q: number) => sorted.length ? sorted[Math.min(sorted.length - 1, Math.floor(q * (sorted.length - 1)))] : 0;
  const mean = sorted.length ? sorted.reduce((a, b) => a + b, 0) / sorted.length : 0;

  const measurement: any = {
    protocol: { warmup_frames: warmup, measured_frames: measured, animated: raw.animated },
    timing: {
      frame_time_ms: { p50: pct(0.5), p95: pct(0.95), p99: pct(0.99) },
      fps_mean: mean > 0 ? 1000 / mean : 0,
      clock: "cpu",
    },
    structural: { draw_calls: raw.draw_calls_mean },          // state_changes / texture_bindings omitted (not instrumented)
    geometry: { triangles: Math.round(raw.triangles_mean) },  // vertices omitted (not instrumented)
    host: {
      os: process.platform, os_version: process.version,
      gpu_vendor: "Google", gpu_model: raw.gpu_model,
      driver_version: "0", build_flags: "",
    },
    capabilities: ["timing", "structural", "geometry"],
  };
  if (raw.js_heap_bytes != null) {
    measurement.resources = {
      peak_memory_bytes: raw.js_heap_bytes, memory_kind: "host",
      load_ms: this.loadMs, first_frame_ms: raw.first_frame_ms,
    };
    measurement.capabilities.push("resources");
  }
  return measurement;
}
```
Notes:
- `triangles` is `u64` in Rust — emit an integer (`Math.round`), not a float, to avoid a u64 parse failure.
- OMIT `state_changes`/`texture_bindings`/`vertices` (Task 1 made them optional). Do NOT emit them as 0.
- `resources` is only added when the JS heap is readable; if not, omit it and don't add `"resources"` to capabilities.

- [ ] **Step 4: Launch flag for precise heap.** In `browser-session.ts` where Chromium launches (`chromium.launch({ ... })`), add `--enable-precise-memory-info` to the `args` array (so `usedJSHeapSize` isn't 1 MB-quantized). If an `args` array doesn't exist yet, add `args: ["--enable-precise-memory-info"]`.

- [ ] **Step 5: Dispatch cases** in `operations.ts` (before `default:`):
```typescript
case "benchmark_plan": {
  const result = await ctx.session.benchmarkPlan(params as any);
  return { ok: true, result };
}
case "benchmark_execute": {
  const result = await ctx.session.benchmarkExecute(params as any);
  return { ok: true, result };
}
```
(Match the exact `DispatchOutcome`/param-cast style of the neighboring cases — read `handleRender`'s case.)

- [ ] **Step 6: Contract test.** In `test/contract.test.ts`, add a test that loads the bundled test `.vrm` (reuse the path/fixture the `render` test uses), sets a camera, runs `benchmark_execute` with small frame counts, and asserts: `result.capabilities` includes `"timing"`,`"structural"`,`"geometry"`; `result.timing.frame_time_ms.p50 >= 0`; `result.structural.draw_calls >= 0`; `result.geometry.triangles >= 0`; and that `result.structural.state_changes === undefined` (omitted). Mirror the multi-step `render` test setup. If the suite can't render in the test env, at minimum add the error-path test (unknown method already covered) plus a `benchmark_plan` test asserting `estimated_frames === warmup+measured`.

- [ ] **Step 7: Build + test.**
```
cd /Users/arkavo/Projects/vrm-conformance/adapters/three-vrm
npm run build 2>&1 | tail -5
npm test 2>&1 | tail -20
```
Expected: TypeScript builds; contract tests pass. If Playwright needs chromium it's already cached. Paste results. If a render-dependent test is flaky/unavailable in this env, report DONE_WITH_CONCERNS and ensure at least build + non-render tests pass.

- [ ] **Step 8: Commit.**
```bash
cd /Users/arkavo/Projects/vrm-conformance
git add adapters/three-vrm/src/renderer-host.html adapters/three-vrm/src/browser-session.ts adapters/three-vrm/src/operations.ts adapters/three-vrm/test/contract.test.ts
git commit -m "feat(three-vrm): benchmark_plan/benchmark_execute (draw_calls, triangles, CPU timing, JS heap)"
```

---

## Task 3: End-to-end through the runner + gated Rust test

**Files:**
- Create: `crates/vrm-runner/tests/benchmark_e2e_three_vrm.rs` (gated like `render_sequence_e2e_three_vrm.rs`)

- [ ] **Step 0: Verify branch** = `feat/perf-metrics-three-vrm`.

- [ ] **Step 1: Build the adapter + runner.**
```
cd /Users/arkavo/Projects/vrm-conformance/adapters/three-vrm && npm run build 2>&1 | tail -3
cd /Users/arkavo/Projects/vrm-conformance && cargo build --release -p vrm-runner -p vrm-asset-generator 2>&1 | tail -2
```
Determine how the runner invokes three-vrm as `--adapter-bin` (read `render_sequence_e2e_three_vrm.rs` / `scripts/*` — it's typically `node adapters/three-vrm/dist/main.js` or a launcher script, possibly via `--adapter-bin node --adapter-args <script>`). Use the SAME invocation the sibling e2e test uses.

- [ ] **Step 2: Run benchmark e2e manually.** Generate an asset and run `vrm-runner benchmark-execute` against three-vrm (mirror the sibling test's adapter invocation), `--renderer-name three-vrm`, small frames. Then `cat` the `<id>_three-vrm.perf.json`. Verify: `capabilities` has `timing`,`structural`,`geometry` (and `resources` if heap readable); `timing.clock == "cpu"`; `structural.draw_calls >= 0` and NO `state_changes` key; `geometry.triangles >= 0` and NO `vertices` key; `host.gpu_model` non-empty. PASTE the perf.json. If the runner reports a deserialize error, fix Task 1/Task 2 and re-run.

- [ ] **Step 3: Gated Rust test.** Read `crates/vrm-runner/tests/render_sequence_e2e_three_vrm.rs` and mirror its gating (binary/node presence check; `#[ignore]` if that's the convention; skip cleanly when three-vrm isn't built). Create `benchmark_e2e_three_vrm.rs` asserting the `PerfReport`: capabilities include timing/structural/geometry; `structural.state_changes` is `None`; `geometry.vertices` is `None`; `structural.draw_calls` present; `timing` is `Some`. Skip cleanly when the adapter is absent so `cargo test --workspace` stays green.

- [ ] **Step 4: Verify.**
```
cd /Users/arkavo/Projects/vrm-conformance
cargo test -p vrm-runner --test benchmark_e2e_three_vrm -- --ignored 2>&1 | tail -12
cargo fmt --all -- --check && cargo clippy -p vrm-runner --tests -- -D warnings 2>&1 | tail -1
```
Paste output.

- [ ] **Step 5: Commit.**
```bash
git add crates/vrm-runner/tests/benchmark_e2e_three_vrm.rs
git commit -m "test(runner): gated three-vrm benchmark e2e (omits unmeasured subfields)"
```

---

## Task 4: Docs

**Files:**
- Modify: `CLAUDE.md` (three-vrm adapter-status line)
- Modify: `docs/superpowers/specs/2026-06-14-performance-metrics-design.md` (capability matrix: three-vrm row implemented; note optional sub-fields)

- [ ] **Step 0: Verify branch.**
- [ ] **Step 1:** In `CLAUDE.md`, the `adapters/three-vrm/` bullet: add "**`benchmark_plan`/`benchmark_execute` real** (CPU frame-time p50/p95/p99 + FPS, `draw_calls`, `triangles`, host JS-heap memory; `state_changes`/`texture_bindings`/`vertices` omitted — three.js does not instrument them)."
- [ ] **Step 2:** In the design spec capability matrix, update the three-vrm row to implemented and add a one-line note that the structural/geometry sub-fields three.js can't measure are omitted (and that the Rust contract made them optional in this slice).
- [ ] **Step 3: Commit.**
```bash
git add CLAUDE.md docs/superpowers/specs/2026-06-14-performance-metrics-design.md
git commit -m "docs(three-vrm): benchmark op real; note optional structural/geometry subfields"
```

---

## Final gate (controller runs)
- `cd adapters/three-vrm && npm run build && npm test` — green.
- `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo test --workspace` — green (gated three-vrm + VMK e2e tests skip cleanly by default).
- The manual e2e `perf.json` shows timing+structural+geometry (+resources), with `state_changes`/`vertices` correctly ABSENT.
