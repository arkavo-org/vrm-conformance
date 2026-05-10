# vrm-metal-kit adapter

Swift adapter that bridges the [VRMMetalKit](https://github.com/arkavo-org/VRMMetalKit) renderer
to the project's renderer-agnostic operation contract documented at
[`docs/operation-contract.md`](../../docs/operation-contract.md).

The adapter is a tiny executable that speaks **JSON-RPC over stdio** with
LSP-style `Content-Length` framing. The Rust runner spawns one of these per
test session and drives it through the operation set (`load_vrm`,
`set_camera`, `set_lighting`, `set_post_processing`, `render`, `dispose`).

## Status

| Phase | Status |
|---|---|
| L1 — package skeleton                          | implemented |
| L2 — JSON-RPC stdio framing + dispatcher       | implemented (all ops return Unimplemented) |
| L3-a — VRMMetalKit dependency wired + linked    | implemented (smoke import only; ops still Unimplemented) |
| L3-b..e — Phase 1 + Phase 2 ops against VRMMetalKit | not yet |

Through L3-a, every Phase 1 operation still returns a structured
`Unimplemented` error (JSON-RPC code `-32000`) with
`data: { "phase": "L3 (VRMMetalKit integration deferred)" }`. Reserved Phase
2+ operations (`set_environment`, `set_expression`, `set_humanoid_pose`,
`set_root_transform`) likewise return `-32000` with the appropriate phase
label. The Phase 2 physics ops (`step_physics`, `reset_physics`,
`animate_root_transform`) are tracked separately and will be promoted out of
Reserved when the corresponding L3 step lands. Unknown methods return
`-32601`.

The L3-a step bumps the package platform floor to `macOS 26`, pins
`arkavo-org/VRMMetalKit` to a specific upstream revision, and imports
`VRMMetalKit` from `main.swift`. The smoke probe verifies a Metal device
boots and the dependency links — so subsequent L3-b work can dispatch
straight into `VRMRenderer` / `VRMModel.load(...)` without re-litigating
the toolchain story.

## Build

```bash
cd adapters/vrm-metal-kit
swift build --configuration release
```

The binary lands at:

```
adapters/vrm-metal-kit/.build/release/vrm-metal-kit-adapter
```

For development:

```bash
swift build --configuration debug
swift test
```

The tests exercise the JSON-RPC framing roundtrip — they do not require a
GPU and do not link any VRM asset.

## How the runner invokes it

The runner spawns the binary as a long-lived child, inherits stderr (for
adapter-side trace logging), and pipes JSON-RPC requests/responses over the
child's stdin/stdout using LSP framing:

```
Content-Length: NNN\r\n
\r\n
{"jsonrpc":"2.0","id":1,"method":"load_vrm","params":{"path":"…"}}
```

See [`docs/operation-contract.md`](../../docs/operation-contract.md) for the
full operation set, JSON shapes, and error envelope.

## Caveats

- The adapter inherits VRMMetalKit's `.macOS(.v26)` platform floor (Swift
  6.2 / Xcode 26). Older macOS hosts can't run this adapter; they can still
  consume golden images downloaded with `scripts/pull-goldens.sh`.
- Spring-bone determinism is a known methodology hazard; the adapter must
  expose deterministic stepping when L3 wires in physics ops.
