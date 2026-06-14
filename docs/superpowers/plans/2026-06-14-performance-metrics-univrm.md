# Performance Metrics — UniVRM Adapter Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Checkbox steps. All work on branch `feat/perf-metrics-univrm`; verify branch first in every task.

**Goal:** Add benchmark support to the UniVRM adapter via the RFC-0003 batch protocol, returning a `PerfMeasurement` per test (CPU frame-time percentiles + FPS, `draw_calls`, `triangles`, `vertices`, host memory). Unity runs the warmup+measured loop in PlayMode per entry; the runner collects the measurement and writes the same `<test_id>_<renderer>.perf.json` as the other adapters.

**Architecture:** UniVRM uses a batched filesystem one-shot (runner writes `manifest.json` → `launcher.sh` runs Unity once → Unity writes `results.ndjson`), NOT per-op JSON-RPC. So a new `--benchmark` flag on `execute-test-batch` makes the runner stamp benchmark params onto each manifest entry; the Unity PlayMode `BatchRunner` runs a warmup+measured render loop per benchmark entry and emits a `perf` block on the result entry; the runner parses it into `PerfMeasurement`, composes a `PerfReport` (reusing `benchmark::compose_report`/`report_path`/`asset_blake3`), and writes the per-test perf.json. One Unity invocation benchmarks the whole plans dir.

**Tech Stack:** Rust runner (`execute_batch.rs`, `cli.rs`) + C# (Unity 6000.4.6f1, UniVRM v0.131.0, Built-in RP, `JsonUtility`). Unity Personal license active locally (verify-able e2e). The per-op `benchmark`/`PerfMeasurement` Rust types are reused unchanged.

**Honesty principle (load-bearing) + JsonUtility constraint:** `JsonUtility` cannot omit value-type fields (a `float` serializes as `0`, never absent/null). To avoid faking `state_changes:0`/`texture_bindings:0`, the C# `PerfStructuralDto` **declares only the fields UniVRM measures** (`draw_calls`). Absent C# fields → absent JSON keys → Rust `PerfStructural` reads them as `None` (they were made `Option` in the three-vrm slice). UniVRM CAN measure `vertices` (`UnityStats.vertices`), so `PerfGeometryDto` declares both `triangles` and `vertices`. NEVER emit a 0 stand-in for an un-measured metric.

**Contract:** `measurement` JSON must deserialize into Rust `vrm_ops::tools::PerfMeasurement`. Capabilities for UniVRM: `["timing","structural","geometry","resources"]`. `clock:"cpu"`, `memory_kind:"host"`.

**Design doc + prior slices:** `docs/superpowers/specs/2026-06-14-performance-metrics-design.md`; foundation/VMK/three-vrm plans in this dir.

---

## Task 1: Runner batch wiring (Rust)

**Files:**
- Modify: `crates/vrm-runner/src/execute_batch.rs` (`BatchTestEntry`, `ResultEntry`, `RunOptions`, `run`, `build_manifest`)
- Modify: `crates/vrm-runner/src/cli.rs` (`Cmd::ExecuteTestBatch` args + handler)

- [ ] **Step 0: Verify branch** = `feat/perf-metrics-univrm`.

- [ ] **Step 1: Add benchmark types + fields** in `execute_batch.rs`:
```rust
/// Per-entry benchmark request carried in the batch manifest. Present only when
/// `execute-test-batch --benchmark` was used; Unity runs a warmup+measured
/// render loop for the entry and returns a `measurement` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchBenchmarkParams {
    pub warmup_frames: u32,
    pub measured_frames: u32,
    pub animate: bool,
}
```
Add to `BatchTestEntry` (after `render_sequence`):
```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark: Option<BatchBenchmarkParams>,
```
Add to `ResultEntry` (after `frame_hz_achieved`), reusing the op type:
```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurement: Option<vrm_ops::tools::PerfMeasurement>,
```
(Confirm `execute_batch.rs` has `use vrm_ops::tools as ops;` or add it; reference the type as `ops::PerfMeasurement` if that's the file convention.)

- [ ] **Step 2: Thread benchmark through `RunOptions` + `build_manifest`.** Add to `RunOptions`:
```rust
    pub benchmark: Option<BatchBenchmarkParams>,
```
In `build_manifest` (where each `BatchTestEntry` is constructed), set `benchmark: opts.benchmark.clone()` on every entry (when `Some`, all entries get the same warmup/measured/animate). Every existing `RunOptions { ... }` construction site must add `benchmark: None` (find them: the `ExecuteTestBatch` handler in cli.rs, and any tests).

- [ ] **Step 3: Per-test PerfReport write in `run()`.** After the BLAKE3 rehash loop and before the spring-positions loop, add a loop that writes a perf.json for each Ok entry carrying a measurement. Build a `test_id → .vrm path` map from the discovered `pairs` for `asset_blake3`:
```rust
    // Benchmark: write a PerfReport per entry that returned a measurement.
    {
        use std::collections::HashMap;
        let vrm_by_id: HashMap<&str, &Utf8PathBuf> =
            pairs.iter().map(|(p, v)| (p.id.as_str(), v)).collect();
        for entry in parsed.entries.iter() {
            if entry.status != ResultStatus::Ok {
                continue;
            }
            let Some(measurement) = entry.measurement.clone() else {
                continue;
            };
            let hash = vrm_by_id
                .get(entry.test_id.as_str())
                .and_then(|p| crate::benchmark::asset_blake3(p).ok())
                .unwrap_or_else(|| "blake3:unknown".to_string());
            let report = crate::benchmark::compose_report(
                &entry.test_id,
                &opts.renderer_name,
                &hash,
                measurement,
            );
            let path = crate::benchmark::report_path(
                &opts.output_dir,
                &entry.test_id,
                &opts.renderer_name,
            );
            match serde_json::to_string_pretty(&report) {
                Ok(json) => {
                    if let Err(e) = std::fs::write(path.as_std_path(), json) {
                        tracing::warn!("failed to write perf report {path}: {e}");
                    }
                }
                Err(e) => tracing::warn!("failed to serialize perf report for {}: {e}", entry.test_id),
            }
        }
    }
```
(Match the real field/variable names in `run()`: the parsed-results collection (`parsed.entries` here — confirm the actual binding name), `pairs`, `ResultStatus::Ok`, `opts.renderer_name`, `opts.output_dir`. Adapt to the real names you read.)

- [ ] **Step 4: CLI flags.** Add to `Cmd::ExecuteTestBatch` in `cli.rs`:
```rust
        /// Benchmark each plan instead of (or in addition to) a single render:
        /// Unity runs warmup+measured frames per entry and the runner writes a
        /// PerfReport JSON per test. Observational — no pass/fail.
        #[arg(long)]
        benchmark: bool,
        #[arg(long, default_value_t = 30)]
        warmup_frames: u32,
        #[arg(long, default_value_t = 300)]
        measured_frames: u32,
        #[arg(long)]
        animate: bool,
```
In the `ExecuteTestBatch` handler, build the `RunOptions.benchmark`:
```rust
            let benchmark = if benchmark {
                Some(crate::execute_batch::BatchBenchmarkParams {
                    warmup_frames, measured_frames, animate,
                })
            } else {
                None
            };
```
and set `benchmark` on the `RunOptions` you pass to `execute_batch::run`. (Destructure the new args in the match arm; mind the name clash — the bool arg is `benchmark` and the local is also `benchmark`; rename the local to `benchmark_opts` if the shadow is awkward.)

- [ ] **Step 5: Tests.** In `execute_batch.rs` tests (or a new `#[cfg(test)] mod`), add:
  - A manifest-serialization test: a `BatchTestEntry` with `benchmark: Some(BatchBenchmarkParams{warmup_frames:5,measured_frames:10,animate:false})` serializes to JSON containing `"benchmark":{"warmup_frames":5,...}`; and with `None` the `benchmark` key is absent.
  - A `ResultEntry` deserialization test: a results line with a `measurement` block (a minimal `PerfMeasurement` JSON: `protocol`+`host`+`capabilities`+`structural{draw_calls}`) parses into `Some`, and `structural.state_changes == None`; a line WITHOUT `measurement` parses into `None`.
  Use the real `ResultEntry`/`BatchTestEntry` shapes.

- [ ] **Step 6: Build + test + lint.**
```
cd /Users/arkavo/Projects/vrm-conformance
cargo test -p vrm-runner execute_batch 2>&1 | tail -10
cargo build -p vrm-runner 2>&1 | tail -2
cargo fmt --all -- --check && cargo clippy -p vrm-runner --all-targets -- -D warnings 2>&1 | tail -1
```
Paste results.

- [ ] **Step 7: Commit.**
```bash
git add crates/vrm-runner/src/execute_batch.rs crates/vrm-runner/src/cli.rs
git commit -m "feat(runner): --benchmark for execute-test-batch (collects per-entry PerfMeasurement)"
```

---

## Task 2: UniVRM C# benchmark (DTOs + PlayMode loop)

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Manifest.cs` (`TestEntryDto.benchmark`; `EntryDto.perf`; new `PerfMeasurementDto` + nested DTOs)
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/PlayMode/BatchRunner.cs` (benchmark loop; load timing)

Read `BatchRunner.cs` (`RenderOneCo` / `RenderSequenceCo`) and `Capture.cs` and `Manifest.cs` first; mirror their coroutine + render + DTO style.

- [ ] **Step 0: Verify branch** = `feat/perf-metrics-univrm`.

- [ ] **Step 1: DTOs in `Manifest.cs`.** All `[Serializable]`, public fields, snake_case names. DECLARE ONLY MEASURABLE FIELDS (so JsonUtility omits the rest):
```csharp
[Serializable] public class BenchmarkDto {
    public int warmup_frames;
    public int measured_frames;
    public bool animate;
}
[Serializable] public class PerfFrameTimeDto { public float p50; public float p95; public float p99; }
[Serializable] public class PerfTimingDto {
    public PerfFrameTimeDto frame_time_ms;
    public float fps_mean;
    public string clock;           // "cpu"
}
[Serializable] public class PerfStructuralDto {
    public float draw_calls;       // ONLY draw_calls — state_changes/texture_bindings NOT declared (Unity can't measure them)
}
[Serializable] public class PerfGeometryDto {
    public long triangles;
    public long vertices;          // UnityStats.vertices IS available
}
[Serializable] public class PerfResourcesDto {
    public long peak_memory_bytes;
    public string memory_kind;     // "host"
    public float load_ms;
    public float first_frame_ms;
}
[Serializable] public class PerfHostDto {
    public string os; public string os_version;
    public string gpu_vendor; public string gpu_model;
    public string driver_version; public string build_flags;
}
[Serializable] public class PerfProtocolDto {
    public int warmup_frames; public int measured_frames; public bool animated;
}
[Serializable] public class PerfMeasurementDto {
    public PerfProtocolDto protocol;
    public PerfTimingDto timing;
    public PerfStructuralDto structural;
    public PerfGeometryDto geometry;
    public PerfResourcesDto resources;
    public PerfHostDto host;
    public string[] capabilities;
}
```
Add `public BenchmarkDto benchmark;` to `TestEntryDto` and `public PerfMeasurementDto perf;` to `EntryDto`.
IMPORTANT (Rust contract): the Rust `ResultEntry.measurement` field is named `measurement`, but the C# `EntryDto` field is `perf`. THESE MUST MATCH. Rename the C# field to `public PerfMeasurementDto measurement;` (matching Rust) — OR add `#[serde(rename = "perf")]` on the Rust side. Pick ONE: name the C# field `measurement` to match the Rust `ResultEntry.measurement` field (simplest). Use `measurement` in C#.

- [ ] **Step 2: Benchmark loop in `BatchRunner.cs`.** In the per-entry coroutine, when `t.benchmark != null && t.benchmark.measured_frames > 0`, run a benchmark instead of the single `Capture.Render`. Mirror `RenderSequenceCo`'s coroutine/render structure. Pseudocode (adapt to real APIs in the file):
```csharp
// after scene setup + Settle (+ animate setup if t.benchmark.animate), with `cam` ready:
var warmup = t.benchmark.warmup_frames;
var measured = t.benchmark.measured_frames;
var frameTimesMs = new System.Collections.Generic.List<float>(measured);
long peakMem = 0;
float firstFrameMs = 0f; bool capturedFirst = false;
long drawSum = 0, triSum = 0, vertSum = 0;

// warmup (discarded)
for (int i = 0; i < warmup; i++) {
    // if animate: set root translation lerp(start,end, measured>1? i/(measured-1):0) — mirror AnimateRootTransform
    var sw = System.Diagnostics.Stopwatch.StartNew();
    cam.Render();
    sw.Stop();
    float ms = (float)sw.Elapsed.TotalMilliseconds;
    if (!capturedFirst) { firstFrameMs = ms; capturedFirst = true; }
    yield return null;
}
// measured
for (int i = 0; i < measured; i++) {
    // if animate: set root translation lerp per frame
    var sw = System.Diagnostics.Stopwatch.StartNew();
    cam.Render();
    sw.Stop();
    frameTimesMs.Add((float)sw.Elapsed.TotalMilliseconds);
#if UNITY_EDITOR
    drawSum += UnityEditor.UnityStats.drawCalls;
    triSum  += UnityEditor.UnityStats.triangles;
    vertSum += UnityEditor.UnityStats.vertices;
#endif
    peakMem = System.Math.Max(peakMem, UnityEngine.Profiling.Profiler.GetTotalAllocatedMemoryLong());
    yield return null;
}
if (!capturedFirst && measured > 0) firstFrameMs = frameTimesMs.Count > 0 ? frameTimesMs[0] : 0f;

frameTimesMs.Sort();
float Pct(float q) => frameTimesMs.Count == 0 ? 0f
    : frameTimesMs[System.Math.Min(frameTimesMs.Count - 1, (int)(q * (frameTimesMs.Count - 1)))];
float meanMs = 0f; foreach (var v in frameTimesMs) meanMs += v;
meanMs = frameTimesMs.Count > 0 ? meanMs / frameTimesMs.Count : 0f;

var measurement = new Manifest.PerfMeasurementDto {
    protocol = new Manifest.PerfProtocolDto { warmup_frames = warmup, measured_frames = measured, animated = t.benchmark.animate },
    timing = new Manifest.PerfTimingDto {
        frame_time_ms = new Manifest.PerfFrameTimeDto { p50 = Pct(0.5f), p95 = Pct(0.95f), p99 = Pct(0.99f) },
        fps_mean = meanMs > 0 ? 1000f / meanMs : 0f,
        clock = "cpu",
    },
    structural = new Manifest.PerfStructuralDto { draw_calls = measured > 0 ? (float)drawSum / measured : 0f },
    geometry = new Manifest.PerfGeometryDto { triangles = measured > 0 ? triSum / measured : 0, vertices = measured > 0 ? vertSum / measured : 0 },
    resources = new Manifest.PerfResourcesDto { peak_memory_bytes = peakMem, memory_kind = "host", load_ms = loadMs, first_frame_ms = firstFrameMs },
    host = new Manifest.PerfHostDto {
        os = "macOS", os_version = SystemInfo.operatingSystem,
        gpu_vendor = SystemInfo.graphicsDeviceVendor, gpu_model = SystemInfo.graphicsDeviceName,
        driver_version = SystemInfo.graphicsDeviceVersion, build_flags = "",
    },
    capabilities = new string[] { "timing", "structural", "geometry", "resources" },
};
entry.measurement = measurement;
entry.status = "ok";
// For a benchmark entry we still set output_path/actual_color_space if the test framework expects them — render one PNG (optional) or leave existing single-render result. Keep status "ok".
```
Notes:
- `loadMs`: wrap the `Vrm10.LoadPathAsync(...)` call in the entry coroutine with a `Stopwatch` and store its `Elapsed.TotalMilliseconds` as `loadMs` for use above. (Mirror where load happens; measure that span.)
- `UnityStats` is `UnityEditor`-only → keep `#if UNITY_EDITOR`. The batch always runs in the Editor (batchmode), so this is fine; outside the guard `drawSum/triSum/vertSum` stay 0 (acceptable fallback, but in practice the guard is always active here).
- Set `entry.measurement` (the new DTO field) on the EntryDto; leave the other EntryDto fields as the existing code sets them (a benchmark entry can still produce a PNG via one `Capture.Render`, or you may skip the golden PNG — but ensure `entry.status="ok"` and `entry.output_path` is set to something valid or the existing single render is still performed once before/after the loop. Simplest: still call the normal single `Capture.Render` once to populate output_path, THEN run the benchmark loop and attach `measurement`.)
- Animate: mirror `PhysicsDriver.AnimateRootTransform` / the sequence loop's root-transform interpolation for the per-frame lerp. If wiring the lerp is non-trivial, support static-only and set `animated=false` regardless, reporting DONE_WITH_CONCERNS — but prefer full support.

- [ ] **Step 3: (No Unity unit test required here.)** Unity C# changes are validated by the e2e run in Task 3 (Unity compiles on launch; a compile error fails the batch). If `BatchRunner.cs` has a sibling EditMode path that also needs the benchmark branch, note it — but the PlayMode path is what `launcher.sh` uses by default, so implement there.

- [ ] **Step 4: Commit.**
```bash
cd /Users/arkavo/Projects/vrm-conformance
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Manifest.cs adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/PlayMode/BatchRunner.cs
git commit -m "feat(univrm): benchmark loop in PlayMode batch (draw_calls, triangles, vertices, CPU timing, memory)"
```

---

## Task 3: End-to-end through Unity + gated Rust test

**Files:**
- Create: `crates/vrm-runner/tests/benchmark_e2e_univrm.rs` (gated like `render_sequence_e2e_univrm.rs`)

This is the real proof — a licensed Unity PlayMode batch run. Unity startup + PlayMode is SLOW (several minutes); use long timeouts and be patient.

- [ ] **Step 0: Verify branch** = `feat/perf-metrics-univrm`.

- [ ] **Step 1: Build the runner + generate a tiny plans dir.**
```
cd /Users/arkavo/Projects/vrm-conformance
cargo build --release -p vrm-runner -p vrm-asset-generator 2>&1 | tail -2
TMP=$(mktemp -d)
cargo run --release -q -p vrm-asset-generator -- emit-default --id bench_univrm --output-dir "$TMP"
ls "$TMP"   # expect bench_univrm.vrm + bench_univrm.test.yaml
```

- [ ] **Step 2: Run the benchmark batch via launcher.sh (licensed Unity, PlayMode).**
```
cd /Users/arkavo/Projects/vrm-conformance
cargo run --release -q -p vrm-runner -- execute-test-batch \
  --plans "$TMP" \
  --adapter-bin adapters/univrm/launcher.sh \
  --output-dir "$TMP/out" \
  --renderer-name univrm \
  --benchmark --warmup-frames 3 --measured-frames 10
echo "=== perf.json ==="
cat "$TMP/out/bench_univrm_univrm.perf.json"
echo "=== results.ndjson (measurement line) ==="
tail -5 "$TMP/out/results.ndjson"
```
This launches Unity in batchmode/PlayMode — allow up to ~10 minutes. REQUIRED — verify + PASTE the perf.json:
- `capabilities` = `["timing","structural","geometry","resources"]`.
- `timing.clock` == "cpu"; `frame_time_ms.p50/p95/p99` present; `fps_mean` > 0.
- `structural.draw_calls` present; NO `state_changes`/`texture_bindings` keys.
- `geometry.triangles` AND `geometry.vertices` present (both > 0).
- `resources.peak_memory_bytes` > 0; `memory_kind` == "host"; `load_ms` >= 0; `first_frame_ms` >= 0.
- `host.gpu_model` non-empty (Apple GPU via Metal); top-level `test_id`/`renderer_name`/`asset_blake3` correct.
If Unity errors (C# compile error, license, deserialize mismatch on the runner side), STOP and report BLOCKED with the Unity log tail (`adapters/univrm/last-run.log` or the runner's captured output) — fix Task 1/Task 2 and re-run. If the runner can't parse `measurement` (serde error), the C# field name or shape is wrong (remember: C# field must be `measurement`, matching Rust).

- [ ] **Step 3: Gated Rust test.** Read `crates/vrm-runner/tests/render_sequence_e2e_univrm.rs` and mirror its gating (how it locates launcher.sh / Unity and SKIPS cleanly when unavailable; `#[ignore]` convention — Unity tests are surely `#[ignore]`). Create `benchmark_e2e_univrm.rs` that runs `execute-test-batch --benchmark` (or the `execute_batch::run` API with `RunOptions.benchmark = Some(...)`) against a generated plan and asserts the resulting per-test `PerfReport`: capabilities include timing/structural/geometry/resources; `structural.state_changes == None` and `texture_bindings == None`; `geometry.vertices.is_some()`; `timing` Some with `clock == Cpu`. Skip cleanly when Unity/launcher is unavailable. Paste the gating snippet.

- [ ] **Step 4: Verify.**
```
cd /Users/arkavo/Projects/vrm-conformance
cargo test -p vrm-runner --test benchmark_e2e_univrm -- --ignored 2>&1 | tail -15   # (matches sibling's ignore convention; SLOW — Unity)
cargo fmt --all -- --check && cargo clippy -p vrm-runner --tests -- -D warnings 2>&1 | tail -1
```
Paste output.

- [ ] **Step 5: Commit.**
```bash
git add crates/vrm-runner/tests/benchmark_e2e_univrm.rs
git commit -m "test(runner): gated UniVRM benchmark e2e (PlayMode batch, full geometry)"
```

---

## Task 4: Docs

**Files:**
- Modify: `CLAUDE.md` (univrm adapter-status line)
- Modify: `docs/superpowers/specs/2026-06-14-performance-metrics-design.md` (capability matrix: univrm row implemented)

- [ ] **Step 0: Verify branch.**
- [ ] **Step 1:** In `CLAUDE.md`, the `adapters/univrm/` bullet: add "**`benchmark` via `execute-test-batch --benchmark` real** (PlayMode batch warmup+measured loop; CPU frame-time p50/p95/p99 + FPS, `draw_calls`, `triangles`, `vertices`, host memory; `state_changes`/`texture_bindings` omitted — Unity doesn't expose them; UnityStats EditMode-gated)."
- [ ] **Step 2:** In the design spec capability matrix, update the UniVRM row to implemented (2026-06-14): timing(cpu)+structural(draw_calls)+geometry(triangles+vertices)+resources(host); note it flows through the batch protocol (not per-op), via the `--benchmark` flag.
- [ ] **Step 3: Commit.**
```bash
git add CLAUDE.md docs/superpowers/specs/2026-06-14-performance-metrics-design.md
git commit -m "docs(univrm): benchmark via execute-test-batch --benchmark"
```

---

## Final gate (controller runs)
- `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo test --workspace` — green (gated UniVRM/VMK/three-vrm e2e skip by default).
- The manual e2e `perf.json` (Task 3 Step 2) shows all four capabilities, with `state_changes`/`texture_bindings` ABSENT and `geometry.vertices` PRESENT.
- (Cross-renderer sanity: compare `draw_calls`/`triangles` against VMK + three-vrm for the default asset — they should be in a familiar range; log a `docs/findings.md` entry only if there's a genuine divergence.)
