# adapter-univrm — design spec

- **Status:** Approved (brainstorming complete, implementation pending)
- **Date:** 2026-05-12
- **Companion RFC:** [`rfcs/0003-engine-idiom-divergence.md`](../../../rfcs/0003-engine-idiom-divergence.md)

## Summary

Add UniVRM (Unity-based, the VRM consortium reference implementation) as the fourth renderer adapter in `vrm-conformance`. UniVRM's role is **offline oracle** — when three-vrm and vrm-metal-kit disagree on MToon shading or outline behavior, UniVRM's output is the closest thing the suite can get to "what the spec actually means." With three renderers shipped (three-vrm, vrm-metal-kit, godot-vrm) the consensus diff can flag outliers; UniVRM extends consensus to four-renderer and adds a ground-truth axis the other three lack.

The adapter ships as a **batched one-shot** invocation: the Rust runner builds a JSON manifest of test_ids, invokes Unity Editor in `-batchmode` once, the C# entry point iterates the manifest, renders each test, writes PNGs + per-test results to disk, and exits. No persistent IPC, no in-process JSON-RPC, no Rust shim crate.

## Motivation

After run 7 (`docs/findings.md`), the corpus has three real renderers but they only agree to a corpus mean SSIM of 0.79 (`three-vrm vs vrm-metal-kit`), 0.87 (`godot-vrm vs vrm-metal-kit`), 0.70 (`godot-vrm vs three-vrm`). When two diverge, the third can suggest an outlier, but none of the three is canonically "correct." For example, `pixiv/three-vrm#1839` (outline floods entire mesh) showed three different failure modes across the three renderers, leaving the suite unable to call which (if any) matched the MToon-1.0 spec.

UniVRM is the implementation the VRMC_materials_mtoon-1.0 spec was written against. Adding it as a fourth renderer gives the suite ground-truth disambiguation it currently lacks. The project's existing scope (Phase 1 plan, `README.md:256`) names UniVRM as a target renderer.

## Detailed design

### Adapter shape: batched one-shot

The Rust runner builds a JSON manifest of test_ids and invokes Unity Editor in batchmode once. The C# entry point iterates the manifest, renders each test, writes PNGs + a per-test results file, and exits. No persistent IPC, no in-process JSON-RPC.

Rationale (full argument carried verbatim from the brainstorming pushback that established this decision):

> For a full corpus re-render: 1 × ~15s startup + 80 × render-time = ~3 minutes total. For an incremental new test: 1 × ~15s startup + 1 × render-time = ~20 seconds. For a single ad-hoc "what does UniVRM say about this test" query: same 20 seconds. This is essentially the persistent design's performance without the persistent design's IPC complexity. The trick is that "persistence" is bought at the batch level (one Unity per batch invocation) rather than at the process level (one Unity alive forever).

**Why not persistent JSON-RPC over TCP (godot-vrm style):**

> godot-vrm shipped its persistent-IPC shim because Godot's headless mode is idiomatically long-running and natively supports it; the TCP loopback in that adapter is solving Unity-class stdout-pollution problems that Godot doesn't have. UniVRM faces the opposite engine constraint: Unity batch mode is idiomatic for "run, do work, exit," and Unity's stdout — polluted by Editor logs, package import chatter, third-party `Debug.Log` calls, and UniVRM's own load-time logging — makes in-process JSON-RPC fragile in a way that produces intermittent CI failures hard to root-cause. The adapter shape matches the engine, not the prior adapter. Code reuse across adapters lives in shared crates (JSON-RPC framing, PNG metadata, manifest schema) and does not require identical adapter shapes.

**Why not direct stdio JSON-RPC (vrm-metal-kit style):**

Unity's stdout pollution is not a small inconvenience that careful stream redirection fixes. Unity Editor writes to stdout from many sources: the Editor itself, package import, AssetDatabase operations, Burst compilation, IL2CPP, and any user code (including UniVRM) that calls `Debug.Log`. Reliably reserving stdout for framed JSON-RPC inside a process that fundamentally wants to log to stdout is fragile in a way that will produce intermittent CI failures that are very hard to root-cause. The `-logFile -` flag helps but doesn't fully solve it because user code can still hit stdout directly.

**What batched one-shot loses (acceptable for UniVRM's role):**

1. **No interactive MCP for UniVRM.** A future critic/agent can't say "stay loaded, now try shadingShift=-0.7." It has to spawn a new batch. Acceptable: UniVRM is the offline oracle, not a hot-loop iterative renderer; interactive iteration belongs on the renderers under active dev.
2. **Adapter shape diverges from godot-vrm.** Documented in [`RFC-0003`](../../../rfcs/0003-engine-idiom-divergence.md): engine idioms differ, and adapter shape matches the engine, not the prior adapter.

### Architecture

```
       Rust runner                                  Unity Editor (batchmode)
   ┌─────────────────┐                          ┌──────────────────────────┐
   │ vrm-runner      │   spawn + CLI args       │ Conformance.RunBatch     │
   │ execute-test-   │ ───────────────────────▶ │ (C# static method)       │
   │ batch           │                          │                          │
   │                 │                          │ ┌──────────────────────┐ │
   │ build           │                          │ │ for each test_id:    │ │
   │ manifest.json ──┼──► manifest.json ───────▶│ │   load .vrm          │ │
   │                 │                          │ │   configure camera   │ │
   │                 │                          │ │   configure lighting │ │
   │                 │                          │ │   configure mtoon    │ │
   │                 │                          │ │   (settle physics)   │ │
   │                 │                          │ │   render to RT       │ │
   │                 │   ◀──── PNG files ───────┤ │   PNG to disk        │ │
   │                 │   ◀── results.ndjson ────┤ │   per-test metadata  │ │
   │                 │                          │ │                      │ │
   │ ingest results  │                          │ └──────────────────────┘ │
   │ + write goldens │                          │ exit                     │
   └─────────────────┘                          └──────────────────────────┘
```

Three units, each with one purpose:

1. **`vrm-runner execute-test-batch` subcommand (Rust)** — builds the manifest, invokes the adapter, ingests results, writes per-renderer manifest under `goldens-cache/univrm/`.
2. **`adapters/univrm/launcher.sh`** — thin shell wrapper that resolves the Unity binary (`UNITY_BIN` env or default install path), the project path, and forwards CLI args to `Unity -batchmode -projectPath ... -executeMethod Conformance.RunBatch -- manifest.json results.ndjson`. Exists so `--adapter-bin` points at one path regardless of where Unity lives on the dev machine.
3. **`adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs`** — the C# batch entry point. Reads manifest, iterates test_ids, owns Unity scene setup, drives UniVRM's loader, configures camera/lighting/MToon, steps spring-bone physics, renders, writes outputs.

Communication is filesystem-as-protocol. No stdio, no TCP, no IPC.

### Environment

- **Unity:** 6000.4.6f1 (Unity 6 LTS; pinned via `ProjectSettings/ProjectVersion.txt`). Originally specced as 2022.3 LTS; bumped to Unity 6 at L3-execution time because the dev machine had Unity 6 installed and UniVRM v0.131.0 supports Unity 2021.3+.
- **UniVRM:** v0.131+ pinned via `Packages/manifest.json` UPM git URL (`https://github.com/vrm-c/UniVRM.git?path=/Assets/VRM10#v0.131.2`-style ref; final pin established during implementation).
- **Render pipeline:** Built-in RP. Built-in is the original implementation the MToon-1.0 spec was authored against and gives the adapter strongest "consortium reference" positioning. URP and HDRP are out of scope (URP is a port; HDRP is unsupported by UniVRM's MToon).
- **Platform:** macOS-only for v1.0. `-batchmode` without `-nographics` so Metal initializes for PNG capture. Same precedent as `vrm-metal-kit`.
- **License:** Unity Personal (free) for personal/organization use under $200K USD/year. Pro ($2,310/seat/yr) above that threshold. No CI license footprint — renders run locally on the maintainer's Mac Studio; CI does build-validate only.

### Scope: Phase 1 + Phase 2 from day one

Initial ship covers the full 80-test corpus end-to-end:

- 44 MToon material variants (`emit-sweep`).
- 18 spring-bone settle variants (`emit-springbone-sweep`).
- 18 spring-bone swing variants (`emit-springbone-swing-sweep`).

Phase 2 spring-bone physics ops (`step_physics`, `reset_physics`, `animate_root_transform`) are implemented via manual VRMSpringBone stepping at 60 Hz fixed step, mirroring the godot-vrm L4 implementation pattern (auto-stepping disabled, explicit per-frame stepping, rest-pose reset before measurement).

Phase 3 ops (`set_humanoid_pose`, `set_expression`, `set_environment`, `set_root_transform`) return `-32000 Unimplemented` with `data.phase` declared, same as the other adapters.

### Components

#### `manifest.json` schema (runner → Unity)

```json
{
  "manifest_version": 1,
  "output_dir": "/abs/path/to/goldens-cache/univrm",
  "renderer_name": "univrm",
  "renderer_version": "v0.131.2",
  "tests": [
    {
      "test_id": "mtoon_default",
      "vrm_path": "/abs/path/mtoon_default.vrm",
      "spec_section": "VRMC_materials_mtoon",
      "camera":   { "position":[0,1.4,1.5], "target":[0,1.4,0], "up":[0,1,0], "fov_degrees":30 },
      "lighting": { "directional":{"dir":[-0.3,-0.6,-0.7],"color":[1,1,1],"intensity":1.0},
                    "ambient":{"color":[0.5,0.5,0.5],"intensity":0.3},
                    "cast_shadows":false, "receive_shadows":false },
      "post_processing": { "tone_mapping":"None", "exposure":1.0 },
      "output":   { "width":1024, "height":1024, "color_space":"Srgb", "msaa":4 },
      "physics":  { "settle_steps":30 },
      "animation": { "root_transform": { "translation_start":[0,0,0], "translation_end":[0.15,0,0], "duration_seconds":0.25, "fps":60 } }
    }
  ]
}
```

The existing `TestPlan` Rust struct flattened to JSON, with paths resolved to absolute. No YAML inside Unity (keeps `YamlDotNet` out of scope).

#### `results.ndjson` schema (Unity → runner)

One JSON object per line, terminated by `\n`. First line is a `_meta` envelope; subsequent lines are per-test results.

**Line 1 — batch metadata:**

```json
{"_meta":true,"manifest_version":1,"renderer_name":"univrm","renderer_version":"v0.131.0","unity_version":"6000.4.6f1","render_pipeline":"Built-in RP","total_tests":80}
```

**Lines 2..N+1 — per-test results:**

```json
{"test_id":"mtoon_default","status":"ok","output_path":"/abs/path/mtoon_default.png","blake3":"blake3:...","actual_color_space":"Srgb","render_seconds":0.18}
{"test_id":"swing_springbone_drag_1","status":"error","error":{"code":-32002,"message":"VRMSpringBone init failed","data":{"phase":"Phase 2"}}}
```

Per-test status is independent — one bad test_id does not abort the batch. Error envelopes use existing JSON-RPC error codes sourced from the in-tree convention:
- `-32000 Unimplemented` (declared but not implemented) — `data.phase` required, per `docs/operation-contract.md`.
- `-32001 LoadFailed` — `.vrm` load failure, per `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift:11`.
- `-32002 RenderFailed` — runtime render-path failure (init, GPU error, etc), per same source.
- `-32602 invalid params` — parameter value not supported (e.g., `tone_mapping: "Aces"`), per `adapters/godot-vrm/src/session.gd:154`.

NDJSON is new to this project (existing per-test adapters return one JSON-RPC response per stdio call). The batched model introduces it; choice is forced by crash-safety. A JSON array would be invalid until the closing `]` is written, so any crash before clean shutdown loses the whole file.

#### C# project layout

```
adapters/univrm/UniVRMConformance/
├── Assets/Conformance/
│   ├── Runtime/
│   │   ├── Conformance.cs          // static class with RunBatch entry point
│   │   ├── Manifest.cs             // [Serializable] DTOs mirroring the JSON shapes
│   │   ├── SceneSetup.cs           // camera/lighting/MToon param application + glTF→Unity coord conversion
│   │   ├── PhysicsDriver.cs        // VRMSpringBone manual stepping (Phase 2)
│   │   └── Capture.cs              // RenderTexture → PNG with color-space handling
│   ├── Tests/                      // EditMode + PlayMode tests (Unity Test Framework)
│   └── Editor/
│       └── DescribeOps.cs          // emit operation catalog JSON for the describe contract
├── Packages/manifest.json          // UniVRM v0.131+ pinned via UPM git URL
├── ProjectSettings/                // Linear color space, Built-in RP, MSAA, etc.
└── README.md
```

`Conformance.RunBatch` is a `public static void` method invoked via `-executeMethod`. Reads the manifest path from `Environment.GetCommandLineArgs()` (Unity passes `-- arg1 arg2` through), iterates `tests[]`, calls into `SceneSetup` + `PhysicsDriver` + `Capture` for each, accumulates results, writes `results.ndjson` incrementally with fsync per entry, and calls `EditorApplication.Exit(0)` or `Exit(1)` on batch-level failure.

#### Rust-side: `vrm-runner execute-test-batch`

New subcommand:

```bash
cargo run -p vrm-runner -- execute-test-batch \
    --plans corpus/ \
    --adapter-bin adapters/univrm/launcher.sh \
    --output-dir goldens-cache/univrm/ \
    [--renderer-name univrm] [--batch-size N]
```

Glob-matches `*.test.yaml` in `--plans`, parses each (reuses `vrm-test-plan` crate), pairs with the sibling `.vrm`, builds `manifest.json` in a temp dir, spawns the adapter, reads `results.ndjson`, validates the `_meta` envelope, ingests per-test results, computes BLAKE3 for each PNG, writes `goldens-cache/univrm/local-manifest.json` matching the existing per-renderer convention.

`--batch-size` reserved for future use; v1.0 always batches the full input set in one invocation.

The existing `execute-test-plan` (per-test) subcommand stays unchanged for the other three adapters.

### Data flow

1. User runs `cargo run -p vrm-runner -- execute-test-batch --plans corpus/ --adapter-bin adapters/univrm/launcher.sh --output-dir goldens-cache/univrm/`.
2. Runner globs `*.test.yaml`, builds `manifest.json` (one entry per test_id), writes to temp dir.
3. Runner spawns `launcher.sh manifest.json results.ndjson`. The launcher resolves `UNITY_BIN` from env (default `/Applications/Unity/Hub/Editor/6000.4.6f1/Unity.app/Contents/MacOS/Unity`), invokes `Unity -batchmode -projectPath adapters/univrm/UniVRMConformance -executeMethod Conformance.RunBatch -- manifest.json results.ndjson -logFile -`.
4. Unity boots (~12 s), `Conformance.RunBatch` runs, writes per-test PNGs into `output_dir/` and `results.ndjson` incrementally next to the manifest.
5. Unity exits. Runner reads `results.ndjson` line-by-line, validates each entry, computes BLAKE3 for any PNG missing the hash field, writes the local manifest.

### Coordinate-system convention (cross-adapter)

**Convention** (sourced from existing adapters, not asserted):

> Test-plan `camera`, `lighting.directional.dir`, and `animation.root_transform.translation_*` params are in glTF-native coordinates: right-handed, Y-up, Z-forward. three-vrm, vrm-metal-kit, and godot-vrm all consume these params directly without conversion (their host engines are right-handed Y-up).

Sources verified in tree at design time:
- `adapters/three-vrm/src/renderer-host.html:94` — `state.camera.position.set(...params.position)` direct spread, no conversion (three.js is right-handed Y-up).
- `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Matrices.swift:16,32` — explicit "Right-handed look-at view matrix" + "Right-handed perspective projection" comments.
- `adapters/godot-vrm/src/session.gd:106` — `camera.look_at_from_position(Vector3(pos[0], pos[1], pos[2]), ...)` direct (Godot 4 is right-handed Y-up).

**UniVRM (Unity is left-handed Y-up): apply a Z-mirror at the SceneSetup layer.** Conversion in `SceneSetup.cs`:

```csharp
Vector3 GltfToUnity(float[] v) => new Vector3(v[0], v[1], -v[2]);
```

Applied to: `camera.position`, `camera.target`, `camera.up`, `lighting.directional.dir`, `animation.root_transform.translation_start/end`. Coordinate-free fields (`intensity`, `fov_degrees`, `color`, `cast_shadows`, MToon material params, physics params) pass through unmodified.

A roundtrip test asserts `mtoon_default` produces the expected screen-space layout on UniVRM (head-mounted sphere centered horizontally, vertically biased toward the upper-middle per the camera's 1.4-m target height). The roundtrip is **confirmatory**, not definitional — the convention is sourced from the three reference adapter implementations above.

### Color-space handling (Built-in RP)

Unity project setting: `PlayerSettings.colorSpace = ColorSpace.Linear`. With Linear set, Built-in RP shades in linear space and the swap-chain texture's sRGB flag controls whether the OETF is applied on output.

**`color_space: Srgb` (v1.0 default per `docs/methodology.md`):** render to default `ARGB32` RT (sRGB target), `ReadPixels` produces sRGB-encoded 8-bit bytes, `EncodeToPNG` writes them. `actual_color_space: "Srgb"` returned.

**`color_space: Linear`:** render to `RenderTextureFormat.ARGB32` with `RenderTexture.sRGB = false`, `ReadPixels` produces raw linear 8-bit bytes. `actual_color_space: "Linear"` returned.

> **Diagnostic-only caveat.** `ARGB32` is 8 bits per channel. "Linear" mode stores linear values quantized across 0–255 with no perceptual remapping — the dark end of the linear range, where most perceptual differences sit, gets crushed into very few quantization steps. This is exactly what sRGB OETF exists to fix. The Linear output is intended for inspecting linear shading math, not for SSIM-grade perceptual comparison. **Use `Srgb` for any diff-engine input.** If a future test plan needs faithful linear output, that's a cross-adapter RFC about adding 16-bit PNG support, not a UniVRM-local feature.

### MSAA, tone mapping, magenta sentinel

**MSAA**: applied per-RT via `new RenderTexture(w, h, 24, ARGB32) { antiAliasing = manifest.output.msaa }`. Per-test setting; cross-test variation works without project reconfigures.

**Tone mapping**: Built-in RP has no built-in per-camera tone mapper. For `tone_mapping: "None"` — the v1.0 MToon-math default — nothing needs to be configured (the linear→sRGB OETF on output is not tone mapping, per the methodology). For other modes (`Linear`, `Reinhard`, `Aces`) the adapter returns the in-tree convention sourced from `godot-vrm/src/session.gd:154`:

```json
{ "code": -32602, "message": "unknown tone_mapping: Aces",
  "data": { "feature": "tone_mapping", "value": "Aces", "supported": ["None"] } }
```

`-32602` (invalid params) is correct because `set_post_processing` exists — what's unsupported is the value, not the op. Adding Reinhard/Aces is a future scope expansion requiring a custom fullscreen post-process pass.

**Magenta sentinel**: `Camera.clearFlags = CameraClearFlags.SolidColor`, `Camera.backgroundColor = new Color(1f, 0f, 1f, 1f)`. Matches the `[255, 0, 255]` convention used by all three existing adapters.

### Error handling layers

1. **Batch-level failures** — Unity can't boot, license not activated, manifest can't be parsed, Unity project missing. Launcher exits non-zero; `Conformance.RunBatch` never runs. Runner sees the non-zero exit + no `results.ndjson`, returns an error to the caller with the launcher's stderr included.

2. **Per-test failures** — one `.vrm` corrupt, one MToon param out of range, one VRMSpringBone init fails. Batch continues; failing test_id appends an error entry to `results.ndjson` using the standard JSON-RPC error envelope. Other test_ids still render.

3. **Partial output (Unity crash mid-batch)** — `results.ndjson` is written incrementally. C# entry point flushes-to-disk per entry:

   ```csharp
   using var stream = new FileStream(resultsPath, FileMode.Create, FileAccess.Write);
   // ... per test:
   var line = JsonConvert.SerializeObject(entry) + "\n";
   var bytes = Encoding.UTF8.GetBytes(line);
   stream.Write(bytes, 0, bytes.Length);
   stream.Flush(flushToDisk: true);   // fsync; survives OOM kill / segfault
   ```

   `Flush(flushToDisk: true)` issues fsync. Per-entry fsync cost on SSD ≈ 5 ms × 80 entries = ~400 ms total — negligible against render time. Without the `flushToDisk` overload, `Flush()` only writes to the OS buffer, which is lost on segfault before the OS gets a chance to write through.

   The runner reads line 1, validates the `_meta` envelope, then reads lines 2..N as per-test results, then counts entries against `total_tests` to detect partial output. Missing tests are reported as `status: "error", error.message: "batch terminated before this test ran"`. The runner does not retry — partial output is reported faithfully; CI/caller decides whether to re-invoke for missing test_ids.

### Logging

Unity stdout pollution is irrelevant in batched one-shot because nothing on the Rust side reads Unity's stdout. Unity's stdout/stderr go to `-logFile -`; the launcher tees combined output to `adapters/univrm/last-run.log` for postmortems. The only Unity artifact the runner consumes is `results.ndjson`. No NDJSON progress on stderr in the batched model — progress is "Unity launched, ran, exited" (the runner can watch `results.ndjson` file size grow for poor-man's progress if needed).

### Testing

Three layers, each owning a distinct failure surface.

**Unity-side (C# EditMode + PlayMode via Unity Test Framework)** at `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/`:

- **EditMode (pure C#, no scene):** `ManifestDeserializationTest`, `CoordinateConversionTest` (assert `GltfToUnity([0, 1.4, 1.5])` produces `Vector3(0, 1.4, -1.5)`; directional dir `[-0.3, -0.6, -0.7]` produces `Vector3(-0.3, -0.6, 0.7)`), `NdjsonEmitterTest` (assert per-entry independent validity + fsync called per entry), `ErrorEnvelopeTest` (assert -32002/-32001/-32000/-32602 codes with documented `data` shapes).
- **PlayMode (real scene, no UniVRM dep):** `BatchExitCodeTest` (empty manifest in, `Exit(0)` + `_meta`-only `results.ndjson` out).

UniVRM-loading tests deferred to the integration layer (need real `.vrm` fixtures; slow).

**Rust-side (workspace integration tests)** at `crates/vrm-runner/tests/execute_test_batch.rs`:

- `manifest_builder_test` — construct manifest from 3 test plans, assert JSON shape.
- `execute_test_batch_with_mock_binary` — invoke `execute-test-batch` with `--adapter-bin tests/fixtures/mock-univrm.sh`, a shell script that writes a known `results.ndjson`. Asserts runner correctly parses `_meta`, ingests entries, computes BLAKE3, emits local manifest. **No Unity dependency.** Analog of `crates/vrm-godot-shim/tests/contract.rs` — protocol verification without the heavyweight runtime.
- `execute_test_batch_partial_output_test` — `mock-univrm-partial.sh` writes 5 entries then exits non-zero; assert remaining 75 reported as `status: "error", error.message: "batch terminated before this test ran"`.
- `execute_test_batch_malformed_meta_test` — `mock-univrm-bad-meta.sh` writes `_meta` missing `total_tests`; assert runner rejects with a clear error.

**Integration (`scripts/`, local-only)** at `scripts/smoke-univrm.sh`:

- Generate 1-test corpus (`emit-default --id smoke`), invoke real Unity batch through the runner, assert one PNG produced, assert centerline pixel is non-magenta (avatar rendered), assert SSIM ≥ 0.75 against three-vrm baseline for the same test_id. Smoke threshold is loose enough to absorb cross-renderer methodology variance but tight enough that a pass means something. If 0.75 turns out too tight against three-vrm in practice, ratchet down with a comment recording the observed value — do not pre-emptively over-loosen.
- `scripts/bootstrap-goldens.sh` gains `RUN_UNIVRM=1` env flag (matching existing `RUN_THREE_VRM=1` convention).

**CI guardrails** — new `.github/workflows/univrm.yml` modeled after `swift.yml`:

- Triggers on `adapters/univrm/**` or runner subcommand changes.
- **Build-validate only.** Sets up Unity via `game-ci/unity-actions` (free-tier license caching), opens the project in `-batchmode -quit`, asserts the C# project compiles, runs EditMode tests via `-runTests -testPlatform EditMode`. Does not attempt to render — GitHub-hosted runners don't have the GPU + display config that `-batchmode` (without `-nographics`) needs for Metal.
- PlayMode tests are local-only. Same precedent as vrm-metal-kit.

### Test-coverage gaps explicitly acknowledged

Three things this layer doesn't cover, deliberately:

1. **Cross-renderer pixel agreement.** That's the corpus's job, not the adapter's. The adapter is tested for "produces some render"; whether the render matches the four-renderer consensus is a `consensus-report.sh` question, run via `scripts/bootstrap-goldens.sh` after the adapter is wired up.
2. **VRMSpringBone determinism across host hardware.** The 60-Hz manual-step convention is asserted to produce deterministic output, but proving it stays deterministic across different macOS versions / Apple Silicon generations needs cross-machine bootstrap runs. Follow-up after the adapter ships, same as the godot-vrm spring-bone determinism story.
3. **License-not-activated regression.** If Unity license activation lapses, the launcher exits non-zero with Unity's specific error message. No automated test for this; manual smoke test the maintainer runs after ~6-month license refresh cycles. Surface area for human error.

## Alternatives considered

### Persistent JSON-RPC over TCP via Rust shim (rejected)

Mirror the `vrm-godot-shim` pattern: a `vrm-univrm-shim` Rust crate spawns Unity Editor in batch mode once, bridges framed stdio ↔ TCP-loopback to a C# JSON-RPC dispatcher inside Unity. ~15 s startup amortized across the whole corpus. Most complexity, best perf.

Rejected because (a) Unity's stdout pollution is solved at the TCP indirection layer but Unity's startup-time hit is the same as one-shot batched (~15 s for either), and (b) Unity batch mode is idiomatic for "run, do work, exit"; persistent JSON-RPC inside a Unity process swims upstream against engine design. The persistent design buys interactive MCP, which UniVRM-as-oracle does not need.

### Persistent JSON-RPC over direct stdio (rejected as near-veto)

Single-binary adapter: a small launcher starts Unity batch-mode with a C# entry point that owns stdin/stdout directly, framed JSON-RPC. Avoids TCP indirection and the shim crate.

Rejected because Unity's stdout pollution is not a small inconvenience that careful stream redirection fixes. Unity Editor writes to stdout from many sources (Editor itself, package import, AssetDatabase, Burst, IL2CPP, third-party `Debug.Log` calls). UniVRM v0.131 alone has non-trivial `Debug.Log` calls during model load. Reliably reserving stdout for framed JSON-RPC inside a process that fundamentally wants to log to stdout is fragile in a way that produces intermittent CI failures that are very hard to root-cause. The TCP loopback in the godot-vrm shim exists specifically to escape this problem; eliminating it puts the problem back.

### One-shot Unity per test (rejected)

Spawn Unity fresh for each test_id. Simplest implementation — no session model, no networking, no shim. ~20-minute penalty over the 80-test corpus.

Rejected because the batched-one-shot design closes most of the perf gap (~3 minutes for the full corpus) without the IPC complexity that persistent designs require. Persistence at the batch level rather than the process level is the right level of indirection for Unity's idioms.

### URP / HDRP render pipelines (rejected)

UniVRM ships `MToon10` for URP as a port of the original `VRMShaders/MToon` Built-in RP implementation. URP is more aligned with what newer Unity projects ship.

Rejected for v1.0 because Built-in RP is the implementation the MToon-1.0 spec was authored against — the strongest "consortium reference" / "ground truth" positioning for UniVRM-as-oracle. URP MToon is a port. HDRP is unsupported by UniVRM's MToon and out of scope. Adding URP later is a future-RFC scope expansion (would double shader surface, require Volume framework for tone mapping, and the value-add is "match three-vrm's WebGL approach more closely" which the four-renderer consensus already provides via three-vrm itself).

### Phase 1 only (rejected)

Ship the 44-MToon-variant corpus through UniVRM first; defer spring-bone ops (`step_physics`, `reset_physics`, `animate_root_transform`) to a follow-up layer.

Rejected — full Phase 1 + Phase 2 from day one. Larger initial scope; gives full four-renderer consensus across the entire corpus on first ship. Spring-bone determinism story is less well-documented for UniVRM than for godot, but the L4 godot-vrm precedent (manual 60-Hz stepping, auto-stepping disabled, rest-pose reset) is directly transferable to VRMSpringBone.

## Open questions

- **UniVRM revision pin.** v0.131.2 is the current latest; design pins "v0.131+" as the floor. Final exact revision selected at implementation time based on which release contains the fixes needed for the macOS Metal path (if any are needed beyond v0.131.0).
- **License activation cadence.** Unity Personal licenses lapse roughly every 6 months and require reactivation. Not blocking but worth scheduling a calendar reminder. Documented as a manual smoke test in the test-coverage gaps section.
- **Reference renderer status.** Currently test plans default to `reference_renderer: "vrm-metal-kit"`. Whether UniVRM should become the new default `reference_renderer` is a separate methodology decision deferred to a future findings.md entry once corpus-wide UniVRM data exists. Until then, UniVRM is "additional consensus voter + spec-correctness oracle for ambiguous cases."

## References

- `docs/operation-contract.md` — JSON-RPC error code envelope.
- `docs/methodology.md` — color-space convention; MToon math test pins.
- `docs/findings.md` runs 1–7 — three-renderer corpus baseline + the three-vrm/vrm-metal-kit/godot-vrm pixel divergence the UniVRM oracle is being added to disambiguate.
- `adapters/godot-vrm/README.md` — L3/L4 layering precedent; manual physics pump pattern.
- `adapters/vrm-metal-kit/README.md` — macOS-only adapter precedent.
- [pixiv/three-vrm#1838](https://github.com/pixiv/three-vrm/issues/1838) — color-space methodology refinement that informed the `color_space: Srgb` default.
- [pixiv/three-vrm#1839](https://github.com/pixiv/three-vrm/issues/1839) — outline-rendering three-way divergence that motivates a fourth disambiguation renderer.
- VRMC_materials_mtoon-1.0 spec — UniVRM's reference implementation.
