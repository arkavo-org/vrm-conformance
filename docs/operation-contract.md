# Operation contract (v0.1)

Every binary in this project — `vrm-asset-generator`, `vrm-runner`, every renderer adapter — exposes the same operation set through **two transports**:

1. **Structured CLI** with `--json` I/O mode and a `describe` subcommand emitting the operation catalog as JSON Schema. Per-op invocations are stateless (good for shell scripts, CI, simple agents).
2. **JSON-RPC stdio server** speaking the same operations. Long-lived sessions are stateful (good for stateful adapters that load a `.vrm` once and render many frames). MCP-aware agents wrap this transport.

**Schemas are the source of truth.** Both transports are generated/validated against the same JSON Schema. The Rust types live in `crates/vrm-ops/`; this document is the language-agnostic contract.

## Discovery

Every binary supports:

```bash
vrm-asset-generator describe --format json
```

Output: a JSON document listing every operation, its input schema, output schema, and a one-line summary. Agents use this for tool discovery; humans use `--help` if they prefer prose.

## Binary I/O

Binary payloads (`.vrm` files, PNG renders, `.mov` artifacts) are **never** embedded in JSON. Operations take input/output **file paths** or **BLAKE3 content-addressed refs** (`blake3:<64-char-hex>`). Content-addressing composes with iroh-blobs and TDF refs for sealed inputs.

## Progress and logging

Long ops emit **NDJSON progress events on stderr**:

```
{"event":"progress","op":"render","frame":42,"total":120,"eta_seconds":3.1}
{"event":"phase","op":"render","phase":"shading_pass"}
```

Stdout is reserved for the structured result (or, in `--json` mode, the response object). Agents tail stderr; humans see a progress bar.

## Plan vs execute

Expensive ops decouple `plan` from `execute`:

```bash
vrm-runner plan-test-plan path/to/plan.yaml --json
# emits: { "estimated_renders": 1, "estimated_seconds": 4.2, "outputs": [...] }
vrm-runner execute-test-plan path/to/plan.yaml --json
# emits NDJSON progress on stderr + final result on stdout
```

Agents can preview cost before committing.

## Idempotency and determinism

Every op declares its codec/container/colorspace explicitly. Implicit defaults are forbidden — they are the most common cause of agent-produced broken output.

## Required operations (Phase 1)

These cover MToon material tests and must be implemented by every renderer adapter.

### `load_vrm`

```json
{ "input": { "path": "string" }, "output": { "session_id": "string" } }
```

### `set_camera`

```json
{
  "input": {
    "session_id": "string",
    "position": [0.0, 1.4, 1.5],
    "target":   [0.0, 1.4, 0.0],
    "up":       [0.0, 1.0, 0.0],
    "fov_degrees": 30.0
  },
  "output": {}
}
```

### `set_lighting`

```json
{
  "input": {
    "session_id": "string",
    "directional": { "dir":[-0.3,-0.6,-0.7], "color":[1,1,1], "intensity":1.0 },
    "ambient":     { "color":[0.5,0.5,0.5], "intensity":0.3 },
    "cast_shadows": false,
    "receive_shadows": false
  },
  "output": {}
}
```

### `set_post_processing`

```json
{
  "input": {
    "session_id": "string",
    "tone_mapping": "None | Linear | Reinhard | Aces",
    "exposure": 1.0
  },
  "output": {}
}
```

### `render`

```json
{
  "input": {
    "session_id": "string",
    "width": 1024, "height": 1024,
    "output_path": "string",
    "color_space": "Linear | Srgb",
    "msaa": 4,
    "output_type": "Color"
  },
  "output": { "output_path": "string", "actual_color_space": "Linear | Srgb" }
}
```

### `render_sequence`

Capture N frames at a fixed display Hz while advancing physics (and optionally a `.vrma` clip or root-transform animation) between samples. The adapter steps physics by `physics_dt_seconds` between each captured frame — display Hz and physics Hz are decoupled so the spring-bone determinism pin (60 Hz fixed step) holds independent of capture rate. See [`rfcs/0004-render-sequence-op.md`](../rfcs/0004-render-sequence-op.md) for the full design.

Adapters that have not implemented sequences return the standard Unimplemented envelope:

```
-32000 Unimplemented   data: { "phase": "v1.x-sequence" }
```

- Params: `RenderSequenceParams { session_id, width, height, output_dir, frame_count, frame_hz, physics_dt_seconds, color_space, msaa, output_type, output_format, animate_root_transform?, apply_vrma? }`
- Result: `RenderSequenceResult { frames: [{ index, timestamp_seconds, path, blake3 }], duration_seconds, actual_color_space, frame_hz_achieved, muxed_path? }`

**Validation rules:**

Adapters MUST enforce:

- `animate_root_transform` and `apply_vrma` are mutually exclusive. Both set ⇒ `-32602 invalid params`.

Adapters SHOULD enforce (methodology pin):

- `physics_dt_seconds > 1/60` violates the spring-bone determinism methodology pin ⇒ `-32602 invalid params`. The runner SHOULD pre-validate before dispatch.

**Output layout:**

- `output_format: png_sequence` → `<output_dir>/<frame_index:04>.png`, no muxed file.
- `output_format: mp4` → per-frame PNGs **plus** `<output_dir>/sequence.mp4` (h.264 yuv420p, lossless qp=0). `muxed_path` is set in the result.
- `output_format: mov` → per-frame PNGs **plus** `<output_dir>/sequence.mov` (Apple ProRes 4444). `muxed_path` is set in the result.

The diff engine consumes per-frame PNGs canonically regardless of `output_format`. Muxed files are convenience for site display and reviewer ergonomics.

### `dispose`

```json
{ "input": { "session_id": "string" }, "output": {} }
```

## Reference implementations

- **`vrm-mock-renderer`** (in-tree, Rust). A deterministic CPU adapter that satisfies the Phase 1 op contract. Renders are a stable function of `MToonParams` — identical params produce byte-identical PNGs, so self-diff is SSIM 1.0 by construction. Used as the default smoke-test adapter; not a real renderer.
- **`adapters/vrm-metal-kit/`** (in-tree, Swift). Real macOS / Metal renderer scaffold. JSON-RPC framing is implemented; the actual VRMMetalKit integration (L3) is deferred.
- **`adapters/three-vrm/`** (in-tree, TypeScript). Node-based renderer for the [pixiv/three-vrm](https://github.com/pixiv/three-vrm) library. Phase 1 ops (`load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`, `dispose`) drive a Playwright headless Chromium WebGL2 context with three.js + three-vrm running inside. Reserved Phase 2+ ops return `Unimplemented`. Requires `npx playwright install chromium` after `npm install`.

## Physics operations

These ops are required for renderer adapters that support `VRMC_springBone`. Adapters without spring-bone support may implement them as no-ops (the mock does this).

### `step_physics`

```json
{
  "input": {
    "session_id": "string",
    "dt_seconds": 0.016666666,
    "count": 1
  },
  "output": {}
}
```

Advances the renderer's internal physics state by `count` steps of `dt_seconds`. For spring-bone determinism, callers always pass `dt_seconds = 1/60` (16.66ms).

### `reset_physics`

```json
{
  "input": {
    "session_id": "string",
    "settle_steps": 30
  },
  "output": {}
}
```

Resets all spring-bone chains to their rest pose, then advances physics by `settle_steps` frames so the chain reaches a stable hanging position before measurement. Default `settle_steps = 30` (0.5 s at 60 Hz) is documented in `docs/methodology.md` as the convention.

The runner calls `reset_physics({ settle_steps })` after `set_post_processing` and before `render` whenever the test plan has a `physics:` block.

### `animate_root_transform`

```json
{
  "input": {
    "session_id": "string",
    "translation_start": [0.0, 0.0, 0.0],
    "translation_end":   [0.2, 0.0, 0.0],
    "duration_seconds": 0.5,
    "fps": 60
  },
  "output": {}
}
```

Linearly interpolates the avatar's root translation from `translation_start` to `translation_end` over `duration_seconds`, advancing the physics simulation by `1/fps` between samples. This is the canonical spring-bone excitation primitive: a static avatar under `step_physics` only exercises gravity settling; sideways/forward motion is what excites inertia and drag.

Translation-only in v0.1 — rotation excitation lands when there's a real test calling for it. Adapters that lack a real physics engine (the mock) should acknowledge the call as a no-op so plan-level protocol coverage still passes.

The runner calls `animate_root_transform` after `reset_physics` (so the chain starts from a settled rest pose) and before `render` whenever the test plan has an `animation.root_transform` block.

### `dump_bone_positions`

```json
{
  "input": {
    "session_id": "string",
    "spring_index": null
  },
  "output": {
    "springs": [
      {
        "name": "hair_chain",
        "joint_positions": [[0.0, 1.5, 0.0], [0.0, 1.4, 0.02]]
      }
    ]
  }
}
```

Returns world-space joint positions for one or all spring-bone chains in the session, captured as of the most recent state-advancing op (`render`, `step_physics`, `reset_physics`, `animate_root_transform`). Does NOT advance physics itself.

**Params:**

| field | type | required | notes |
|---|---|---|---|
| `session_id` | string | yes | from `load_vrm` |
| `spring_index` | integer | no | omit (or `null`) to dump all springs; out-of-range returns empty springs array (permissive probe semantics) |

`joint_positions` is in world space, in the VRM 1.0 coordinate system, joint-order head-to-tail.

**Errors:**

- `-32602` InvalidParams — unknown `session_id`
- `-32000` Unimplemented — adapter does not have a spring-bone system (e.g. univrm L3 in rest-pose-only mode)

**Adapter support (as of phase 1):**

| adapter | status |
|---|---|
| mock | deterministic empty array (mock has no springs) |
| three-vrm | implemented (chain reconstruction via `VRMSpringBoneJoint.child` walks) |
| vrm-metal-kit | implemented (reads `session.model.springBone.springs[].joints[].node` → `nodes[idx].worldPosition`) |
| godot-vrm | implemented (reads `vrm_secondary.spring_bones[].joint_nodes` → Skeleton3D global pose) |
| univrm | returns `-32000 Unimplemented` (phase: "L3") |

## VRMA (VRMC_vrm_animation) operations

Defined in the [VRMA conformance design spec](superpowers/specs/2026-05-17-vrma-conformance-design.md). Adapters that have not implemented VRMA return the standard Unimplemented envelope:

```
-32000 Unimplemented   data: { "phase": "vrma-v1" }
```

### `load_vrma`

Parse a `.vrma` file (VRMC_vrm_animation glTF) and return an opaque handle plus a summary of the channels it contains. Only `animations[0]` is treated as the portable clip per VRMA spec.

- Params: `LoadVrmaParams { vrma_path: string }`
- Result: `LoadVrmaResult { vrma_handle: u32, channel_summary: { humanoid_bones, expressions, has_look_at, duration_seconds } }`

### `apply_vrma_at_time`

Sample the loaded clip at `time_seconds` and write the resulting pose onto the avatar identified by `vrm_handle`. Linear interpolation is the spec-mandated default. State-advancing.

- Params: `ApplyVrmaAtTimeParams { session_id, vrma_handle, vrm_handle, time_seconds }`
- Result: `ApplyVrmaAtTimeResult { channels_applied: { humanoid_bones, expressions, look_at } }`

### `dump_humanoid_pose`

Return per-bone local rotations + the hips translation as of the most recent state-advancing op. Bones referenced by the .vrma but absent from the .vrm appear in `bones_missing` and are excluded from per-bone diff (methodology hazard #3).

- Params: `DumpHumanoidPoseParams { session_id }`
- Result: `DumpHumanoidPoseResult { source_spec_version, bones, hips_translation, bones_missing }`

### `dump_expression_weights`

Return current expression weights, preset + custom kept structurally separate per spec. Weights are clamped to `[0, 1]`.

- Params: `DumpExpressionWeightsParams { session_id }`
- Result: `DumpExpressionWeightsResult { source_spec_version, presets: map<string, f32>, custom: map<string, f32> }`

### `dump_look_at_state`

Return current gaze direction (raw quat + spec-defined Extrinsic ZXY yaw/pitch) and the avatar's application mode (`bone | expression | off`).

- Params: `DumpLookAtStateParams { session_id }`
- Result: `DumpLookAtStateResult { source_spec_version, gaze_direction_quat, yaw_deg, pitch_deg, applied_via, offset_from_head_bone }`

### Response field: `source_spec_version`

Every dump operation's response (`dump_humanoid_pose`, `dump_expression_weights`, `dump_look_at_state`) carries a required `source_spec_version: "0.x" | "1.0"` field, echoing what the adapter parsed from the loaded asset.

Cross-checks at runner level (Task 26): must match the test plan's declared `spec_version` and the manifest entry's `spec_version`. Mismatches trigger hard-error abort.

This field is the third gate in the three-way spec-version cross-check: (1) test plan ↔ manifest, (2) test plan camera convention ↔ spec_version, (3) test plan ↔ adapter-reported `source_spec_version`.

Adapters detect this from the loaded asset's `extensionsUsed` array: `VRMC_vrm` → `"1.0"`, `VRM` → `"0.x"`.

The field is **required** — not optional, no `serde(default)`. Adapters cannot omit it.

## Reserved operations (Phase 2+)

Required to be **declared** by every adapter (`describe` lists them) but may return a structured `Unimplemented` error in v0.1:

- `set_environment` (HDRI) — v1.x
- `set_expression` — Phase 3
- `set_humanoid_pose` — Phase 2
- `set_root_transform` — Phase 2

## Runner-only operations

These are exposed by `vrm-runner` but not by renderer adapters. They orchestrate adapter calls and produce derived artifacts.

### `diff`

```json
{
  "input": {
    "plan":          "string (path to test plan YAML)",
    "render":        "string (path to render PNG)",
    "reference":     "string (path to reference PNG)",
    "renderer_name": "string"
  },
  "output": {
    "test_id":            "string",
    "renderer":           "string",
    "reference_renderer": "string",
    "ssim":               "number",
    "ssim_threshold":     "number",
    "ssim_passed":        "boolean",
    "properties":         "array of PropertyResult"
  }
}
```

`diff` runs SSIM between `render` and `reference`, then evaluates each property assertion in the plan against the render image. Exits non-zero when `overall_passed` (i.e., `ssim_passed && all properties pass`) is false; agents and CI use the exit code as the pass/fail signal.

`execute-test-plan` accepts an optional `--reference` flag that runs `diff` inline after the render and includes the `DiffResult` (plus an `overall_passed` boolean) in its JSON output. Its exit code is unchanged: 0 means "the pipeline ran"; pass/fail is signaled via `overall_passed` in the JSON. Callers who want exit-gating use the standalone `diff` subcommand.

## Output types

`output_type` on `render`:

- `Color` — required, sRGB or linear PNG.
- `Normal`, `Depth`, `Albedo`, `MToonShadingMask` — reserved for v1.x debug-pass SSIM.

## Error envelope

Both transports use the same error codes (CLI exits non-zero with a JSON error on stderr in `--json` mode; JSON-RPC returns the error in the response):

| Code | Meaning |
|---|---|
| `-32601` | Operation not found (standard JSON-RPC code). |
| `-32602` | Invalid params. |
| `-32000` | `Unimplemented` — declared but not implemented in this version. `data: { "phase": "v1.x" }`. |
| `-32001` | `LoadFailed` — `.vrm` failed to load (validation, missing extension). `data: { "validator_report": "..." }`. |
| `-32002` | `RenderFailed` — render step failed (OOM, GPU error). `data: { "reason": "..." }`. |

## Stdio framing (JSON-RPC transport)

JSON-RPC messages framed per the [Language Server Protocol header convention](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#headerPart):

```
Content-Length: NNN\r\n
\r\n
{"jsonrpc": "2.0", ...}
```

This is the same framing MCP itself uses for stdio transports — the MCP wrapper is a thin shim, not a separate protocol.
