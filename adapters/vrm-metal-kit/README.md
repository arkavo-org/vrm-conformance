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
| L3-c — `set_camera` + `set_lighting` + `set_post_processing` + `render` | implemented |
| L3-d — MSAA 4x render path | implemented |
| L3-e — physics ops (`step_physics`, `reset_physics`, `animate_root_transform`) | implemented |

Every Phase 1 op now produces real output. `load_vrm` constructs a
`VRMRenderer`, calls `renderer.loadModel(model)`, and stashes both in a
per-session entry. `set_camera` / `set_lighting` / `set_post_processing`
park their params on the session; the projection matrix is built at
`render` time once aspect = width/height is known. `render` allocates a
single-sample `rgba8Unorm` color target and a `depth32Float` depth
target, calls `VRMRenderer.drawOffscreenHeadless(...)` (wrapped in
`MainActor.assumeIsolated` since that method is `@MainActor`-isolated),
waits for GPU completion via a semaphore, and writes a PNG via
`CGImageDestination`. The clear color is magenta `[255, 0, 255]` so the
diff engine's bbox-relative property assertions can detect the avatar
against a known sentinel — same convention as the mock and three-vrm.

Smoke-verified locally on Apple M4 Max:
- `cargo run -p vrm-asset-generator -- emit-default --id smoke --output-dir $D`
- Drive the adapter through `load_vrm → set_camera → set_lighting → set_post_processing → render → dispose` with the standard MToon test plan camera/lighting.
- Produced 256×256 PNG: ~3200 bytes, 45% non-magenta pixels (the head-mounted sphere), 55% magenta background.

Phase 2 physics ops (`step_physics`, `reset_physics`,
`animate_root_transform`) still return `-32000 Unimplemented` with
`data: { "phase": "L3 (VRMMetalKit integration deferred)" }`. Other
reserved ops (`set_environment`, `set_expression`, `set_humanoid_pose`,
`set_root_transform`) keep their original phase labels. Unknown methods
return `-32601`.

L3-e physics-op semantics:

- `step_physics(count)` calls `renderer.warmupPhysics(steps: count)`.
  `dt_seconds` is ignored — VRMMetalKit's spring-bone simulation uses
  fixed 120Hz substeps. No-op for models without spring-bone data.
- `reset_physics(settle_steps)` calls `warmupPhysics` too — that one
  function both zeros velocity and runs N settle steps, which is exactly
  reset-then-settle.
- `animate_root_transform(start, end, duration_seconds, fps)` runs the
  full per-frame loop: mutates each root node's translation, sets
  `characterVelocity` to the constant root velocity (so the
  inertia-compensation kernel applies trailing force), and issues a
  64×64 single-sample `drawOffscreenHeadless` to a `.private`
  throwaway target each frame. The render call drives
  `SpringBoneComputeSystem.update` via VRMMetalKit's internal path; GPU
  commit overhead provides natural pacing for the renderer's
  `CACurrentMediaTime`-based deltaTime. After the loop completes,
  `characterVelocity` is restored to zero so the next render doesn't
  inherit phantom motion.

**Known limitations** (revisit before declaring L3 complete):

- **MSAA pinned to 4x.** `RendererConfig.sampleCount` is set at
  `load_vrm` and the render pipeline state objects bake the sample count
  in, so we can't vary MSAA per render. The render request's `msaa`
  field is accepted for protocol compliance but does not change behavior
  — matching three-vrm, which similarly configures antialiasing at
  canvas creation. `docs/methodology.md` pins MSAA to 4x as the v1.0
  standard, so this is conformant for the current corpus.
- **Tone mapping.** VRMMetalKit doesn't expose tone mapping on its public
  API. The handler accepts `tone_mapping` but only "None" matches the
  rendered output. Test plans for MToon math pin this to "None" per
  `docs/methodology.md`, so this is conformant for the current corpus.
- **Color space.** `Linear` requests use `rgba8Unorm`, `Srgb` requests use
  `rgba8Unorm_srgb`. CG doesn't embed a color profile in the output PNG;
  downstream diff is pixel-exact within a single color space, which is
  what the operation contract guarantees.
- **Spring-bone visual signal hidden in current corpus.** The
  asset-generator's `emit-springbone-*` subcommands produce VRMs whose
  visible mesh (a sphere) is parented to the head node, **not skinned
  to the spring chain joints**. Spring-bone physics tick correctly in
  the adapter (verified via stderr `spring-bone: N springs, M joints`
  log per session), but the rendered pixels don't change because the
  visible geometry isn't deformed by chain motion. Cross-renderer
  visual diffing against three-vrm produces the same null result for
  the same asset-side reason. Adding a chain-skinned cylinder/ribbon
  mesh is a separate corpus-extension task.

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
