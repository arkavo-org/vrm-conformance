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

### `dispose`

```json
{ "input": { "session_id": "string" }, "output": {} }
```

## Reserved operations (Phase 2+)

Required to be **declared** by every adapter (`describe` lists them) but may return a structured `Unimplemented` error in v0.1:

- `set_environment` (HDRI) — v1.x
- `set_expression` — Phase 3
- `set_humanoid_pose` — Phase 2
- `set_root_transform`, `animate_root_transform` — Phase 2
- `step_physics`, `reset_physics` — Phase 2

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
