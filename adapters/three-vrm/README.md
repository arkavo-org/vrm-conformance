# three-vrm renderer adapter

TypeScript renderer adapter for the [VRM conformance suite](https://github.com/arkavo-org/vrm-conformance). Speaks the [operation contract](../../docs/operation-contract.md) over stdio JSON-RPC, satisfying the same contract as the Rust mock and the Swift VRMMetalKit scaffold.

## Status

| Phase | Scope | State |
|---|---|---|
| 2C-a | TS scaffold + JSON-RPC framing + Unimplemented dispatch | shipped |
| 2C-b | three-vrm + Playwright headless WebGL2 + real Phase 1 ops | shipped |

Phase 1 ops are real: `load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`, `dispose`. Reserved Phase 2+ ops still return `Unimplemented` with the appropriate phase label.

## Browser dependency

This adapter spawns a headless Chromium instance via [Playwright](https://playwright.dev/) and runs three.js + three-vrm inside the browser context. The Chromium binary is **not** included in `node_modules`; install it once after `npm install`:

```bash
npx playwright install chromium
```

Disk: ~250 MB cached at `~/Library/Caches/ms-playwright/` (macOS) or `~/.cache/ms-playwright/` (Linux). RAM at runtime: ~50 MB per running adapter.

## Build

```bash
cd adapters/three-vrm
npm install
npm run build
node dist/main.js
```

Or, for the inner-loop dev experience without a build step:

```bash
npm start  # runs src/main.ts via tsx
```

## Run via the conformance runner

```bash
cargo run -p vrm-runner -- execute-test-plan \
  --plan ../../assets/generated/smoke_default.test.yaml \
  --adapter-bin "$(npm bin)/tsx" \
  --adapter-args "$(pwd)/src/main.ts" \
  --asset-dir ../../assets/generated \
  --output-dir /tmp/three-vrm-out \
  --renderer-name three-vrm \
  --json
```

Phase 1 ops will return `LoadFailed` or `Unimplemented` until 2C-b lands; the runner exits with the structured error in JSON form.

## Tests

```bash
npm test
```

Two test files:

- `test/framing.test.ts` — Content-Length round-trip primitives.
- `test/contract.test.ts` — spawn `dist/main.js` as a subprocess, exchange framed JSON-RPC, assert error envelopes.

## Phase 1 ops (must implement in 2C-b)

`load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`, `dispose`.

## Reserved (Unimplemented in v0.1, v1.x, Phase 2, Phase 3)

`set_environment` (v1.x), `set_humanoid_pose` (Phase 2), `set_root_transform` (Phase 2), `set_expression` (Phase 3).

Implemented Phase 2 ops: `step_physics`, `reset_physics`, `animate_root_transform`.
