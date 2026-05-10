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
| L3-a — VRMMetalKit dependency wired + linked    | implemented |
| L3-b — `load_vrm` + `dispose` against VRMMetalKit | implemented |
| L3-c..e — `set_*` + `render` + physics ops      | not yet |

`load_vrm` parses the path, checks file existence, calls
`VRMModel.load(from:device:)` (bridging async→sync via `DispatchSemaphore`
since the JSON-RPC dispatcher is sync), and allocates a session id
`vrm-metal-kit-N`. The session registry holds the loaded `VRMModel` until
`dispose` is called. Missing files surface as `-32001 LoadFailed`;
malformed VRMs surface the same code with the underlying
`VRMMetalKit.invalidGLBFormat(reason:)` (or analogous) error in
`data.reason`. `dispose` is idempotent — disposing an unknown id returns
ok, matching the three-vrm and mock-renderer contracts.

The remaining Phase 1 ops (`set_camera`, `set_lighting`,
`set_post_processing`, `render`) still return `-32000 Unimplemented` with
`data: { "phase": "L3 (VRMMetalKit integration deferred)" }` and land in
L3-c. The Phase 2 physics ops (`step_physics`, `reset_physics`,
`animate_root_transform`) are also tracked under the L3 deferral label and
move out when the L3 spring-bone integration lands. Unknown methods return
`-32601`.

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
