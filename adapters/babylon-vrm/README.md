# babylon-vrm renderer adapter

A renderer adapter that will bridge [virtual-cast/babylon-vrm-loader](https://github.com/virtual-cast/babylon-vrm-loader) to the project's renderer-agnostic operation contract documented at [`docs/operation-contract.md`](../../docs/operation-contract.md).

Same architecture as the [three-vrm adapter](../three-vrm/README.md): a tiny Node executable that speaks JSON-RPC over stdio with LSP-style `Content-Length` framing. The runner spawns one of these per test session and drives it through the operation set.

## Why a third adapter

vrm-conformance currently has two real adapters (three-vrm + vrm-metal-kit). The [N-way consensus diff](../../crates/vrm-diff-engine/src/consensus.rs) (introduced in commit [`dbb44da`](https://github.com/arkavo-org/vrm-conformance/commit/dbb44da)) needs three or more independent renderers to produce a real majority-vs-outlier signal. With only two, "they disagree" is the strongest claim we can make; with three, "renderer X is the outlier" becomes possible.

The first finding from consensus diff ([arkavo-org/VRMMetalKit#183](https://github.com/arkavo-org/VRMMetalKit/issues/183) + [pixiv/three-vrm#1838](https://github.com/pixiv/three-vrm/issues/1838)) is exactly the case where a third reference would settle which renderer is correct: three-vrm renders the default MToon sphere with a dark gray shadow (~0.21 linear); vrm-metal-kit renders it as flat white. Without a third reference (UniVRM, Babylon-VRM-Loader, Godot-VRM), the divergence is undecidable from inside the conformance suite. This adapter — once L3 lands — adds Babylon as the tie-breaker.

## Status

| Phase | Status |
|---|---|
| L1 — package skeleton                         | implemented |
| L2 — JSON-RPC stdio framing + dispatcher      | implemented (all ops return `Unimplemented`) |
| L3 — Phase 1 ops against Babylon-VRM-Loader   | not yet |

Through L2, every operation returns a structured `Unimplemented` error (JSON-RPC code `-32000`):

| Method | `data.phase` |
|---|---|
| `load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`, `dispose` | `L3 (babylon-vrm integration deferred)` |
| `set_humanoid_pose`, `set_root_transform`, `animate_root_transform`, `step_physics`, `reset_physics` | `Phase 2` |
| `set_environment` | `v1.x` |
| `set_expression` | `Phase 3` |
| (unknown) | `-32601 method not found` |

L3 will swap in real handlers driving Babylon-VRM-Loader inside Playwright Chromium (same browser-host pattern three-vrm uses), so the adapter can render the conformance test corpus and contribute renders to consensus diff.

## Build

```bash
cd adapters/babylon-vrm
npm install
npm run build
node dist/main.js   # JSON-RPC stdio server
```

For development without an explicit build step:

```bash
npm start   # runs src/main.ts via tsx
```

## Tests

```bash
npm test
```

Two test files:

- `test/framing.test.ts` — Content-Length round-trip primitives (identical to three-vrm's framing; the framing module is intentionally byte-for-byte the same).
- `test/contract.test.ts` — spawn `dist/main.js` as a subprocess, exchange framed JSON-RPC, assert the L3-deferral phase labels for Phase 1 ops and the canonical phase labels for reserved ops.

## How the runner invokes it

Same wire as three-vrm and vrm-metal-kit. The runner spawns the binary as a long-lived child and pipes framed JSON-RPC requests/responses:

```
Content-Length: NNN\r\n
\r\n
{"jsonrpc":"2.0","id":1,"method":"load_vrm","params":{"path":"…"}}
```

When L3 lands, `scripts/bootstrap-goldens.sh` will pick up the babylon-vrm adapter automatically alongside three-vrm and vrm-metal-kit, contributing a third entry per test_id to the manifest. The consensus-diff command's `--render` flag will then take three or more `name=path` pairs.

## L3 sketch

The L3 milestone needs:

1. `dependencies` in `package.json`: `playwright`, `@babylonjs/core`, `babylon-vrm-loader`.
2. A `src/renderer-host.html` that imports Babylon + babylon-vrm-loader (importmap or bundled) and exposes `window.__loadVrm` / `__setCamera` / `__setLighting` / `__setPostProcessing` / `__render` / `__dispose`, matching three-vrm's interface so the dispatch shape stays parallel.
3. A `src/browser-session.ts` that boots a single Playwright Chromium and routes `https://app.local/asset` to the loaded VRM's disk path (three-vrm hit the same `app://` custom-scheme block on Chromium and the synthetic-HTTPS workaround is the cleanest path).
4. Operations dispatch updates: replace the `Unimplemented` returns for Phase 1 ops with passthrough to the browser session.
5. Magenta clear color `[255, 0, 255]` for property-assertion bbox detection.
6. Tone mapping pinned to `None` for MToon math tests (per `docs/methodology.md`).

Most of these can be lifted directly from `adapters/three-vrm/src/`; the renderer-host.html is the only file that needs significant rewriting (different library, different scene setup, different camera API).
