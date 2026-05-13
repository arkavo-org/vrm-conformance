# univrm renderer adapter

Bridges [UniVRM](https://github.com/vrm-c/UniVRM) (the VRM consortium reference implementation, Unity-based) to the project's renderer-agnostic operation contract documented at [`docs/operation-contract.md`](../../docs/operation-contract.md).

Architecture differs from the [three-vrm](../three-vrm/README.md) and [vrm-metal-kit](../vrm-metal-kit/README.md) adapters: Unity batch mode is idiomatic for "run, do work, exit," and Unity's stdout pollution makes in-process JSON-RPC fragile (see [`rfcs/0003`](../../rfcs/0003-engine-idiom-divergence.md)). This adapter uses **batched one-shot**: the Rust runner builds a JSON manifest of test_ids, invokes Unity once for the whole batch, the C# entry point writes per-test results to an NDJSON file, exits. Filesystem-as-protocol; no stdio, no TCP, no Rust shim.

```
runner ──manifest.json──▶ launcher.sh ──▶ Unity -batchmode -executeMethod Conformance.RunBatch
runner ◀───results.ndjson──────────────── Unity (incrementally written, fsync per entry)
```

## Status

| Phase | Status |
|---|---|
| L1 — project skeleton + UPM pin | shipped |
| L2 — Rust runner subcommand + NDJSON contract + mock-fixture tests | shipped |
| L3 — Phase 1 ops real (`load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`) | **shipped** |
| L4 — Phase 2 spring-bone physics ops (manual 60 Hz stepping) | partial; PlayMode batch required |

L3 covers Phase 1 end-to-end: VRM load via `Vrm10.LoadPathAsync(awaitCaller: new ImmediateCaller())`, camera + directional + ambient via `SceneSetup` with glTF→Unity Z-mirror, `RenderTexture` → `ReadPixels` → `EncodeToPNG` via `Capture`. All 80 test_ids render successfully through the adapter; the full corpus's `goldens-cache/univrm/local-manifest.json` reports 80 OK / 0 error.

L4 is **partially implemented**. `PhysicsDriver.cs` carries the manual-stepping logic (`RestoreInitialTransform` + `Process(1/60)` × `settle_steps` + per-frame `Process(1/fps)` during `animate_root_transform`), but UniVRM v0.131.0's `Vrm10FastSpringboneRuntimeStandalone` constructs its Burst-compiled job buffers only when `Application.isPlaying == true` (see `Packages/VRM10/Runtime/Components/Vrm10Runtime/Springbone/Vrm10FastSpringboneRuntimeStandalone.cs`). `-batchmode -executeMethod` runs in EditMode, so the driver detects EditMode and no-ops cleanly; spring-bone tests render in **rest pose** at L3 (with the avatar root parked at `animation.root_transform.translation_end` for swing-test framing). Full L4 stepping needs a PlayMode batch entry point — separate follow-up plan.

## Runtime dependency

Unity 6000.4.6f1 (Unity 6 LTS) must be installed via Unity Hub. The launcher resolves the binary at `/Applications/Unity/Hub/Editor/6000.4.6f1/Unity.app/Contents/MacOS/Unity` by default; override with `UNITY_BIN` env or `UNITY_VERSION` env (the Hub install path is derived from `UNITY_VERSION`).

License: Unity Personal (free for individuals/orgs under $200K USD/yr). Activation is a one-time manual flow per machine; see [Unity's docs](https://docs.unity3d.com/Manual/ManagingYourUnityLicense.html). Lapses every ~6 months and needs re-activation; no automated regression test for the lapse path.

## Build

The Rust side (subcommand + tests) builds as part of the workspace:

```bash
cargo build -p vrm-runner --release
```

The Unity project is opened lazily by the launcher when Unity is invoked. On first open, Unity will re-import UniVRM from its UPM git URL (~5 minutes); subsequent opens use the cached `Library/` directory.

## Tests

```bash
# Rust contract tests (no Unity needed)
cargo test -p vrm-runner --test execute_test_batch

# Unity EditMode tests (requires Unity 6000.4.6f1 installed)
"$UNITY_BIN" \
  -batchmode \
  -projectPath adapters/univrm/UniVRMConformance \
  -runTests \
  -testPlatform EditMode \
  -testResults /tmp/univrm-test-results.xml \
  -logFile -

# Smoke (L3+, requires real rendering)
scripts/smoke-univrm.sh   # one-test E2E through the adapter
```

CI (`.github/workflows/univrm.yml`) runs both the Rust contract tests (via the workspace `rust.yml`) and the Unity EditMode tests (via `game-ci/unity-test-runner`) on `macos-latest`.

## How the runner invokes it

```bash
cargo run -p vrm-runner -- execute-test-batch \
  --plans <corpus-dir>/ \
  --adapter-bin adapters/univrm/launcher.sh \
  --output-dir goldens-cache/univrm/ \
  --renderer-name univrm \
  --json
```

`<corpus-dir>` is a directory of `*.test.yaml` + matching `.vrm` files (one pair per test_id), as produced by `cargo run -p vrm-asset-generator -- emit-sweep --output-dir <corpus-dir>`.

## How it's *not* like other adapters

| Adapter | Adapter shape | Engine idiom fit |
|---|---|---|
| three-vrm | Direct JSON-RPC over stdio | Node.js reserves stdout cleanly |
| vrm-metal-kit | Direct JSON-RPC over stdio | Swift Foundation reserves stdout cleanly |
| godot-vrm | Persistent JSON-RPC over TCP via Rust shim | Godot --headless is idiomatic long-running |
| **univrm** | **Batched one-shot via filesystem** | **Unity batch mode is idiomatic "run, do work, exit"** |

See [`rfcs/0003`](../../rfcs/0003-engine-idiom-divergence.md) for the principle this divergence sits beneath.
