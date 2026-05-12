# godot-vrm renderer adapter

A renderer adapter that bridges [V-Sekai/godot-vrm](https://github.com/V-Sekai/godot-vrm) to the project's renderer-agnostic operation contract documented at [`docs/operation-contract.md`](../../docs/operation-contract.md).

Architecture differs from the [three-vrm](../three-vrm/README.md) and [babylon-vrm](../babylon-vrm/README.md) adapters by necessity: GDScript on Godot 4 does not expose a byte-safe stdout API (no `OS.write_buffer_to_stdout`, `print`/`printraw` are banner-polluted, `--quiet` suppresses everything, `FileAccess.open("/dev/stdout", ...)` is rejected). The conformance runner's framed stdio contract therefore lives in a separate Rust shim — `vrm-godot-shim` — which spawns Godot headless as a child and bridges JSON-RPC ↔ TCP-loopback. See [`docs/superpowers/plans/2026-05-11-adapter-godot-vrm-scaffold-v2.md`](../../docs/superpowers/plans/2026-05-11-adapter-godot-vrm-scaffold-v2.md) for the rationale.

```
runner ──framed stdio──> vrm-godot-shim ──NDJSON over TCP──> godot --headless --script src/main.gd
```

## Why a third adapter

vrm-conformance has two real adapters (three-vrm + vrm-metal-kit). The [N-way consensus diff](../../crates/vrm-diff-engine/src/consensus.rs) needs three or more independent renderers to flag outliers. The natural third candidate — `virtual-cast/babylon-vrm-loader` via [`adapters/babylon-vrm/`](../babylon-vrm/) — is upstream-blocked on VRM 1.0 support. `V-Sekai/godot-vrm` already implements VRMC_vrm, VRMC_materials_mtoon, VRMC_springBone, and VRMC_node_constraint, so it's the realistic next adapter for closing the third-renderer gap.

## Status

| Phase | Status |
|---|---|
| L1 — package skeleton                         | scaffolded |
| L2 — JSON-RPC + dispatch                      | scaffolded (all ops return `Unimplemented`) |
| L3 — Phase 1 ops against V-Sekai/godot-vrm    | shipped |
| L4 — Phase 2 spring-bone physics ops          | deferred — spring-bone settle/swing tests skip godot-vrm |

Through L3, Phase 1 ops are real. Remaining ops still return a structured `Unimplemented` error (JSON-RPC code `-32000`):

| Method | `data.phase` |
|---|---|
| `set_humanoid_pose`, `set_root_transform`, `animate_root_transform`, `step_physics`, `reset_physics` | `Phase 2` |
| `set_environment` | `v1.x` |
| `set_expression` | `Phase 3` |
| (unknown) | `-32601 method not found` |

## Runtime dependency

Godot 4.x must be on `PATH` as `godot` (or pointed at via `GODOT_BIN`). 4.3 minimum; tested on 4.6.2.

- macOS: `brew install --cask godot`
- Linux: download `Godot_v4.3-stable_linux.x86_64.zip` from [Godot releases](https://github.com/godotengine/godot/releases/tag/4.3-stable) and put the binary on `PATH`.

## Build

```bash
cargo build -p vrm-godot-shim --release
```

The runner consumes `target/release/vrm-godot-shim` as `--adapter-bin`. The `adapters/godot-vrm/` Godot project is discovered automatically; override with `GODOT_VRM_ADAPTER_DIR` if needed.

## Tests

```bash
# GDScript dispatch unit tests
godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd

# Rust end-to-end contract test (spawns shim + real Godot)
cargo test -p vrm-godot-shim --test contract -- --ignored
```

Both run in CI (`.github/workflows/godot-vrm.yml`).

## How the runner invokes it

Same wire as the other adapters — framed LSP `Content-Length` JSON-RPC over stdio. The shim handles framing; Godot only sees NDJSON over TCP loopback. Wire-level invocation:

```bash
cargo run -p vrm-runner -- execute-test-plan \
  --plan <plan.yaml> \
  --adapter-bin target/release/vrm-godot-shim \
  --asset-dir <assets> \
  --output-dir <out> \
  --renderer-name godot-vrm \
  --json
```
