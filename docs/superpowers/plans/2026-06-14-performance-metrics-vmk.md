# Performance Metrics — VMK Adapter Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax. All work on branch `feat/perf-metrics-vmk`; verify branch first in every task.

**Goal:** Wire VRMMetalKit's real performance metrics into the VMK adapter so `benchmark_plan` / `benchmark_execute` return a full `PerfMeasurement` (timing + structural + geometry + resources), replacing the foundation-slice's mock-only support.

**Architecture:** The adapter (`adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`) adds two dispatch cases + handlers. `benchmark_execute` attaches `PerformanceTracker` to the session's `VRMRenderer`, runs `warmup_frames` discarded frames then `measured_frames` measured frames through `drawOffscreenHeadless` (mirroring `handleRender`'s scene setup + MSAA path), reads `getPerformanceMetrics()`, samples `device.currentAllocatedSize` for peak memory, and hand-builds the `PerfMeasurement` JSON (no Codable — manual `JSONValue.object`). The runner deserializes that into Rust `PerfMeasurement` and composes the `PerfReport`. v1 stays observational.

**Tech Stack:** Swift / Metal / Xcode 26 (local M4 Max — CI build-only). VRMMetalKit pinned at `39e65f0`. Rust runner already supports `benchmark-execute` (foundation slice, on `main`).

**Verification reality:** `swift test` covers only error paths (unknown session → `-32602`); the real render path needs the GPU + main thread (`MainActor.assumeIsolated`). The load-bearing verification is an **end-to-end run through the Rust runner** against a generated `.vrm` on this Mac, asserting the written `*.perf.json` carries all four capabilities with sane numbers. This mirrors how VMK runtime coverage is validated generally (locally, not CI).

**Design doc:** [`docs/superpowers/specs/2026-06-14-performance-metrics-design.md`](../specs/2026-06-14-performance-metrics-design.md). Foundation plan: [`2026-06-14-performance-metrics-foundation.md`](2026-06-14-performance-metrics-foundation.md).

## Contract the Swift result must satisfy

`benchmark_execute` returns JSON that deserializes into Rust `vrm_ops::tools::PerfMeasurement`. Exact shape (omit optional blocks entirely when not populated; do NOT emit `null`):

```json
{
  "protocol": { "warmup_frames": 30, "measured_frames": 300, "animated": false },
  "timing": { "frame_time_ms": { "p50": 1.2, "p95": 1.9, "p99": 2.4 }, "fps_mean": 740.0, "clock": "gpu_cpu" },
  "structural": { "draw_calls": 4.0, "state_changes": 2.0, "texture_bindings": 6.0 },
  "geometry": { "triangles": 12345, "vertices": 6789 },
  "resources": { "peak_memory_bytes": 12345678, "memory_kind": "gpu", "load_ms": 42.0, "first_frame_ms": 8.0 },
  "host": { "os": "macOS", "os_version": "26.0", "gpu_vendor": "Apple", "gpu_model": "Apple M4 Max", "driver_version": "0", "build_flags": "" },
  "capabilities": ["timing", "structural", "geometry", "resources"]
}
```

Enum wire values are snake_case: `clock` ∈ {`gpu_cpu`,`cpu`}, `memory_kind` ∈ {`gpu`,`host`}, capability strings ∈ {`timing`,`structural`,`geometry`,`resources`}. `benchmark_plan` returns `{ "estimated_frames": N, "estimated_seconds": F, "scene_summary": "..." }`. Integer-valued fields (`triangles`/`vertices`/`peak_memory_bytes`, and `protocol.*`) must serialize as JSON integers — the adapter's existing `.number(Double)` path already does this for `render_sequence`'s `index`/`frame_count` (u32 in Rust), so reuse it; the e2e step will catch any u64-parse failure.

---

## Task 1: VMK benchmark handlers + load timing + unit tests

**Files:**
- Modify: `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift` (dispatch switch; `Session` class; `handleLoadVrm`; add two handlers)
- Modify: `adapters/vrm-metal-kit/Tests/VRMMetalKitAdapterTests/JsonRpcServerTests.swift` (error-path tests)

- [ ] **Step 0: Verify branch.** `cd /Users/arkavo/Projects/vrm-conformance && git branch --show-current` → must be `feat/perf-metrics-vmk`.

- [ ] **Step 1: Capture load time on the session.**

Add a stored property to the `Session` class (near the other `var` state):
```swift
var loadMs: Double = 0
```
In `handleLoadVrm`, wrap the model-load work (the call(s) that produce the `VRMModel` + build the `VRMRenderer`) with a wall-clock measurement and store it on the session before returning success:
```swift
let loadStart = CACurrentMediaTime()
// ... existing model load + renderer construction ...
let loadMs = (CACurrentMediaTime() - loadStart) * 1000.0
// after the Session is created/stored:
session.loadMs = loadMs
```
Read the actual `handleLoadVrm` body first and place the timing around the real load call. If the load is split across helpers, measure the outermost span in `handleLoadVrm`. If genuinely awkward, set `session.loadMs = 0` and note it — but prefer a real measurement.

- [ ] **Step 2: Add dispatch cases.** In the `dispatch(method:params:)` switch in `Operations.swift`, add after the `"render_sequence"` case:
```swift
        case "benchmark_plan":          return handleBenchmarkPlan(params: params)
        case "benchmark_execute":       return handleBenchmarkExecute(params: params)
```

- [ ] **Step 3: Implement `handleBenchmarkPlan`** (no GPU; validate session like `handleRender`):
```swift
private func handleBenchmarkPlan(params: JSONValue?) -> OpOutcome {
    guard case .object(let obj) = params,
          case .string(let sessionId) = obj["session_id"]
    else { return invalidParams("missing session_id") }
    guard lookupSession(sessionId) != nil else {
        return invalidParams("invalid session_id: \(sessionId)")
    }
    let warmup = intField(obj["warmup_frames"]) ?? 30
    let measured = intField(obj["measured_frames"]) ?? 300
    let width = intField(obj["width"]) ?? 0
    let height = intField(obj["height"]) ?? 0
    let total = warmup + measured
    return .ok(.object([
        "estimated_frames": .number(Double(total)),
        "estimated_seconds": .number(Double(total) / 60.0),
        "scene_summary": .string("VMK \(width)x\(height) msaa\(Operations.msaaSampleCount)"),
    ]))
}
```
`intField` helper: if one already exists for extracting an `Int` from a `JSONValue.number`, reuse it; otherwise add a small private helper:
```swift
private func intField(_ v: JSONValue?) -> Int? {
    if case .number(let d)? = v { return Int(exactly: d.rounded()) }
    return nil
}
```
`invalidParams(...)` and `lookupSession(...)` already exist (used by `handleRender`) — match their exact signatures. The unknown-session case must produce `-32602` (confirm `invalidParams` yields code `-32602`; if `handleRender` uses a different helper for invalid session, use that one).

- [ ] **Step 4: Implement `handleBenchmarkExecute`.** Mirror `handleRender`'s scene setup (projection/view from session camera; lights via `setLight`/`disableLight`; `colorPixelFormat` from `color_space` lowercased) and MSAA-4× texture + render-pass-descriptor construction, but: pre-allocate the textures ONCE before the loop and reuse them; do not write PNGs. Drive frames through `drawOffscreenHeadless` exactly as `handleRender` does (inside `MainActor.assumeIsolated`, commit, wait on the completion semaphore). Read `handleRender` and `handleRenderSequence` in the file and reuse their texture/RPD/command-buffer code verbatim — do not invent Metal setup.

Core structure (fill the scene-setup/texture/draw boilerplate from `handleRender`):
```swift
private func handleBenchmarkExecute(params: JSONValue?) -> OpOutcome {
    guard case .object(let obj) = params,
          case .string(let sessionId) = obj["session_id"]
    else { return invalidParams("missing session_id") }
    guard let session = lookupSession(sessionId) else {
        return invalidParams("invalid session_id: \(sessionId)")
    }
    guard let device = self.device, let commandQueue = self.commandQueue else {
        return .error(code: -32002, message: "RenderFailed", data: .object(["reason": .string("no Metal device")]))
    }

    let warmup = intField(obj["warmup_frames"]) ?? 30
    let measured = intField(obj["measured_frames"]) ?? 300
    let width = intField(obj["width"]) ?? 256
    let height = intField(obj["height"]) ?? 256
    var colorSpace = "linear"
    if case .string(let cs)? = obj["color_space"] { colorSpace = cs.lowercased() }
    let animated = (obj["animate_root_transform"].map { if case .null = $0 { return false } else { return true } }) ?? false
    // Parse animate translation_start/translation_end if animated (mirror handleRenderSequence's RootTransformAnimation parsing).

    // --- scene setup: copy from handleRender (projection/view matrices, lights) ---
    // --- allocate MSAA color+depth (.private) + resolve target, ONCE, reuse across frames ---

    session.renderer.performanceTracker = PerformanceTracker()
    defer { session.renderer.performanceTracker = nil }

    func drawOneFrame() -> Double {
        let t0 = CACurrentMediaTime()
        // build MTLRenderPassDescriptor (multisampleResolve, magenta clear) — same as handleRender
        // make command buffer from commandQueue
        let sem = DispatchSemaphore(value: 0)
        // commandBuffer.addCompletedHandler { _ in sem.signal() }
        MainActor.assumeIsolated {
            session.renderer.drawOffscreenHeadless(to: msColorTex, depth: msDepthTex,
                                                   commandBuffer: commandBuffer,
                                                   renderPassDescriptor: rpd)
        }
        // commandBuffer.commit(); sem.wait()
        return (CACurrentMediaTime() - t0) * 1000.0
    }

    // Warmup (discarded). Capture the FIRST warmup frame's wall time as first_frame_ms (cold pipeline).
    var firstFrameMs = 0.0
    for i in 0..<warmup {
        // if animated: set root translation = lerp(start, end, 0) for warmup
        let ms = drawOneFrame()
        if i == 0 { firstFrameMs = ms }
    }
    // Clean the tracker window so measured stats exclude warmup.
    session.renderer.resetPerformanceMetrics()

    var peakMem = 0
    for i in 0..<measured {
        // if animated: t = measured > 1 ? Float(i)/Float(measured-1) : 0; set root translation = lerp(start,end,t)
        //   (mirror handleRenderSequence's root-node translation update + worldTransform refresh)
        _ = drawOneFrame()
        peakMem = max(peakMem, device.currentAllocatedSize)
    }

    let metrics = session.renderer.getPerformanceMetrics()

    var measurement: [String: JSONValue] = [
        "protocol": .object([
            "warmup_frames": .number(Double(warmup)),
            "measured_frames": .number(Double(measured)),
            "animated": .bool(animated),
        ]),
        "host": .object([
            "os": .string("macOS"),
            "os_version": .string(ProcessInfo.processInfo.operatingSystemVersionString),
            "gpu_vendor": .string("Apple"),
            "gpu_model": .string(device.name),
            "driver_version": .string("0"),
            "build_flags": .string(""),
        ]),
    ]
    var capabilities: [JSONValue] = []
    if let m = metrics {
        measurement["timing"] = .object([
            "frame_time_ms": .object([
                "p50": .number(m.frameTimeP50Ms),
                "p95": .number(m.frameTimeP95Ms),
                "p99": .number(m.frameTimeP99Ms),
            ]),
            "fps_mean": .number(m.fps),
            "clock": .string("gpu_cpu"),
        ])
        capabilities.append(.string("timing"))
        measurement["structural"] = .object([
            "draw_calls": .number(Double(m.drawCalls)),
            "state_changes": .number(Double(m.stateChanges)),
            "texture_bindings": .number(Double(m.textureBindings)),
        ])
        capabilities.append(.string("structural"))
        measurement["geometry"] = .object([
            "triangles": .number(Double(m.triangleCount)),
            "vertices": .number(Double(m.vertexCount)),
        ])
        capabilities.append(.string("geometry"))
    }
    measurement["resources"] = .object([
        "peak_memory_bytes": .number(Double(peakMem)),
        "memory_kind": .string("gpu"),
        "load_ms": .number(session.loadMs),
        "first_frame_ms": .number(firstFrameMs),
    ])
    capabilities.append(.string("resources"))
    measurement["capabilities"] = .array(capabilities)

    return .ok(.object(measurement))
}
```
Notes for the implementer:
- Replace the pseudo-comments with the REAL Metal setup copied from `handleRender` (texture descriptors, RPD, command buffer + completion semaphore). Keep the textures allocated once and reused across all warmup+measured frames.
- The `animated` parse: detect presence of a non-null `animate_root_transform` object; if present, parse its `translation_start`/`translation_end` `[f32;3]` arrays the same way `handleRenderSequence` parses its `RootTransformAnimation`, and apply the lerped root translation per measured frame (and per warmup frame at t=0). If wiring the root-translation update is non-trivial, it is acceptable for THIS task to support only the static path and return `-32602` when `animate_root_transform` is present, BUT prefer full support mirroring `handleRenderSequence`. If you reduce scope here, report it as DONE_WITH_CONCERNS.
- `PerformanceTracker`, `VRMRenderer.performanceTracker`, `resetPerformanceMetrics()`, `getPerformanceMetrics()` are public VRMMetalKit API (confirmed). `PerformanceMetrics` fields used: `frameTimeP50Ms`, `frameTimeP95Ms`, `frameTimeP99Ms`, `fps`, `drawCalls`, `stateChanges`, `textureBindings`, `triangleCount`, `vertexCount` — all `public`.
- Import: `PerformanceTracker`/`PerformanceMetrics` come from `import VRMMetalKit` (already imported in Operations.swift).

- [ ] **Step 5: Unit tests (error paths only — no GPU).** In `JsonRpcServerTests.swift`, add tests mirroring the existing framed-request pattern:
  - `benchmark_plan` with an unknown `session_id` → response `error.code == -32602`.
  - `benchmark_execute` with an unknown `session_id` → response `error.code == -32602`.
  Use the existing `frame(...)` / `splitFramedResponses(...)` helpers and the `JsonRpcServer(input:output:log:).run()` harness. Example:
```swift
func testBenchmarkExecuteUnknownSessionInvalidParams() throws {
    let request = #"{"jsonrpc":"2.0","id":70,"method":"benchmark_execute","params":{"session_id":"no-such","width":64,"height":64,"color_space":"linear","msaa":1,"output_type":"Color","warmup_frames":1,"measured_frames":1}}"#
    let writer = MemoryWriter()
    JsonRpcServer(input: MemoryReader(frame(request)), output: writer, log: MemoryWriter()).run()
    let resp = try XCTUnwrap(splitFramedResponses(writer.data).first)
    let err = try XCTUnwrap(resp["error"] as? [String: Any])
    XCTAssertEqual(err["code"] as? Int, -32602)
}
```
  Add the analogous `benchmark_plan` test (id 71).

- [ ] **Step 6: Build + test.**
```
cd /Users/arkavo/Projects/vrm-conformance/adapters/vrm-metal-kit
swift build 2>&1 | tail -5
swift test 2>&1 | tail -15
```
Expected: build succeeds; tests pass (including the two new error-path tests). If `swift build` fails because Xcode/toolchain isn't selected, run `sudo xcode-select -s /Applications/Xcode.app` first (Xcode 26.5 is installed). Paste the build + test summary lines.

- [ ] **Step 7: Commit.**
```bash
cd /Users/arkavo/Projects/vrm-conformance
git add adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift adapters/vrm-metal-kit/Tests/VRMMetalKitAdapterTests/JsonRpcServerTests.swift
git commit -m "feat(vmk): benchmark_plan/benchmark_execute via VRMMetalKit PerformanceTracker"
```

---

## Task 2: End-to-end verification through the runner (+ gated Rust e2e test)

**Files:**
- Create: `crates/vrm-runner/tests/benchmark_e2e_vmk.rs` (gated like the other VMK e2e tests)

This is the real proof: a full cross-process benchmark of VMK against a generated `.vrm`, asserting the `PerfReport` JSON has all four capabilities with sane numbers.

- [ ] **Step 0: Verify branch** = `feat/perf-metrics-vmk`.

- [ ] **Step 1: Build the release adapter + runner.**
```
cd /Users/arkavo/Projects/vrm-conformance
swift build -c release --package-path adapters/vrm-metal-kit 2>&1 | tail -3
cargo build --release -p vrm-runner -p vrm-asset-generator 2>&1 | tail -3
```
The adapter binary is `adapters/vrm-metal-kit/.build/release/vrm-metal-kit-adapter` (executable target `vrm-metal-kit-adapter`). Confirm it exists.

- [ ] **Step 2: Generate an asset and run the benchmark manually.**
```
TMP=$(mktemp -d)
cargo run --release -q -p vrm-asset-generator -- emit-default --id bench_vmk --output-dir "$TMP"
cargo run --release -q -p vrm-runner -- benchmark-execute \
  --plan "$TMP/bench_vmk.test.yaml" \
  --adapter-bin adapters/vrm-metal-kit/.build/release/vrm-metal-kit-adapter \
  --asset-dir "$TMP" \
  --output-dir "$TMP/out" \
  --renderer-name vrm-metal-kit \
  --warmup-frames 5 --measured-frames 30 \
  --json
cat "$TMP/out/bench_vmk_vrm-metal-kit.perf.json"
```
Expected: a `PerfReport` with `capabilities` containing all four of `timing`,`structural`,`geometry`,`resources`; `timing.frame_time_ms.p50/p95/p99` > 0; `structural.draw_calls` > 0; `geometry.triangles` > 0; `resources.peak_memory_bytes` > 0; `host.gpu_model` non-empty (e.g. "Apple M4 Max"). Paste the JSON. If `benchmark_execute` errors (e.g. a u64-parse failure on triangles/vertices, or a snake_case mismatch), FIX it in Task 1's handler and re-run — this step is the contract validator.

- [ ] **Step 3: Add a gated Rust integration test.** Read `crates/vrm-runner/tests/render_sequence_e2e_vmk.rs` and mirror its gating EXACTLY (how it locates/skips when the VMK adapter binary or Xcode isn't available, e.g. an env guard or a `build.rs`/binary-presence check). Create `crates/vrm-runner/tests/benchmark_e2e_vmk.rs` that: locates the release VMK adapter binary the same way, generates/loads a small asset (reuse the helper the sibling test uses), runs `vrm_runner::benchmark::run_benchmark` (or shells the subcommand — match the sibling's approach), and asserts the resulting `PerfReport` has the four capabilities and positive timing/structural/geometry values. The test MUST skip cleanly (not fail) when the adapter binary is absent, so `cargo test --workspace` stays green on machines without the built adapter. Paste the gating snippet you mirrored.

- [ ] **Step 4: Verify the gated test.**
```
cd /Users/arkavo/Projects/vrm-conformance
cargo test -p vrm-runner --test benchmark_e2e_vmk 2>&1 | tail -15
```
Expected: passes (adapter built in Step 1) OR skips cleanly with a clear message. Also confirm `cargo test --workspace` still green when the adapter is absent is preserved by the gating (you can simulate by temporarily pointing the gate at a nonexistent path if the sibling test supports it — otherwise reason about the gate). Paste output.

- [ ] **Step 5: Commit.**
```bash
git add crates/vrm-runner/tests/benchmark_e2e_vmk.rs
git commit -m "test(runner): gated VMK benchmark e2e (all four capabilities)"
```

---

## Task 3: Docs — mark VMK benchmark real

**Files:**
- Modify: `CLAUDE.md` (VMK adapter-status line)
- Modify: `docs/superpowers/specs/2026-06-14-performance-metrics-design.md` (capability matrix note)
- Modify: `docs/findings.md` IF a cross-renderer divergence is observed (only if Task 2 surfaced one)

- [ ] **Step 0: Verify branch** = `feat/perf-metrics-vmk`.

- [ ] **Step 1: Update `CLAUDE.md`.** In the "Adapter status" section, the `adapters/vrm-metal-kit/` bullet, add a sentence noting the benchmark op is real: e.g. append "**`benchmark_plan`/`benchmark_execute` real** (full `PerfReport` — timing+structural+geometry+resources — via VRMMetalKit `PerformanceTracker` + `device.currentAllocatedSize`; runtime coverage local-only)." Match the file's existing phrasing for the other "real" ops on that line.

- [ ] **Step 2: Update the design spec capability matrix.** In `docs/superpowers/specs/2026-06-14-performance-metrics-design.md`, the adapter capability matrix row for VMK currently describes the intent; add a short "(implemented <date>)" note or change tense to reflect it's now real. Keep it factual; do not rewrite the section.

- [ ] **Step 3: (Conditional) findings.** Only if the Task 2 e2e run revealed something divergent or surprising (e.g. anomalous draw-call count vs expectations) — add a `docs/findings.md` entry per that file's format. If nothing surprising, SKIP this step and say so.

- [ ] **Step 4: Commit.**
```bash
git add CLAUDE.md docs/superpowers/specs/2026-06-14-performance-metrics-design.md docs/findings.md
git commit -m "docs(vmk): benchmark op real in VMK adapter"
```
(Drop `docs/findings.md` from the add if Step 3 was skipped.)

---

## Final gate (controller runs)

- `cd adapters/vrm-metal-kit && swift build && swift test` — green.
- `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` — clean (the new Rust e2e test must be clippy-clean).
- `cargo test --workspace` — green (the gated VMK test passes or skips cleanly).
- The manual e2e `perf.json` from Task 2 Step 2 shows all four capabilities with sane numbers.
