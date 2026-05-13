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
| L1 — project skeleton + UPM pin | scaffolded |
| L2 — Rust runner subcommand + NDJSON contract + mock-fixture tests | scaffolded |
| L3 — Phase 1 ops real (`load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`) | deferred |
| L4 — Phase 2 spring-bone physics ops | deferred |

Through L2, the runner side speaks the full batch contract end-to-end against mock-binary fixtures (`crates/vrm-runner/tests/fixtures/mock-univrm-*.sh`). The Unity side compiles cleanly with UniVRM v0.131.2 pinned via UPM but `Conformance.RunBatch` is a stub — every test returns `-32000 Unimplemented` with `data.phase: "L3"`. Real rendering arrives in the L3 plan.

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
scripts/smoke-univrm.sh   # not present until L3
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
