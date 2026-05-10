# vrm-conformance

Cross-renderer conformance and render-fidelity infrastructure for the [VRM 1.0](https://github.com/vrm-c/vrm-specification) ecosystem.

**Status:** Phase 1 (single-renderer vertical slice) — under active development.
**License:** [Apache 2.0](./LICENSE). Generated test assets are CC0.
**Donation intent:** Methodology-compatible with [KhronosGroup/glTF-Render-Fidelity](https://github.com/KhronosGroup/glTF-Render-Fidelity); we intend to donate to the Khronos glTF Working Group / VRM Consortium once VRM extensions ratify as `KHR_*` (Oct 2024 Khronos × VRM Consortium liaison makes this likely within 1–2 years).

## What this is

- A **parametric VRM asset generator** that emits a deterministic test corpus covering the MToon material spec, spring bone behaviors, constraints, and expressions.
- An **agent-first conformance runner** that drives every supported renderer through the same test plan via a uniform operation catalog (structured CLI with `--json` mode + thin MCP wrapper, both backed by one core).
- A **golden-image comparison site** that displays renders from each renderer side-by-side, with SSIM diff + property-assertion overlays.

## What this is not

- Not a glTF / VRM file-format validator — that job belongs to [`mrxz/vrm-validator`](https://github.com/mrxz/vrm-validator), which we depend on as a precondition gate.
- Not a live cross-renderer comparison server. Renders are PR-submitted offline, exactly the model Khronos uses.
- Not a VTuber-specific behavior suite (Perfect Sync, ARKit face tracking).

## Surface contract — agent-first

Every binary in this project is built around a single core library exposed as **two transports**:

1. A **structured CLI** with `--json` I/O mode and a `describe` subcommand emitting the operation catalog as JSON Schema.
2. A **JSON-RPC stdio server** speaking the same operations (the MCP wrapper is a thin shim).

Schemas are the source of truth for both. Binary payloads (`.vrm`, `.png`, `.mov`) are passed via file paths or BLAKE3 content-addressed refs — never embedded in JSON. Long ops emit NDJSON progress on stderr. See [`docs/operation-contract.md`](./docs/operation-contract.md).

## Repository layout

| Path | Purpose |
|---|---|
| `crates/vrm-asset-generator/` | Rust binary. Structured CLI (`--json`, `describe`) emitting paired `<asset>.vrm` + `<asset>.meta.json` + `<asset>.test.yaml`. |
| `crates/vrm-runner/` | Rust binary. Structured CLI (`--json`, `describe`) that reads test plans, drives an adapter (CLI or MCP transport), runs diff engine. |
| `crates/vrm-ops/` | Operation catalog + JSON Schema emission + JSON-RPC stdio transport. CLI and MCP wrappers both depend on this. |
| `crates/vrm-diff-engine/` | SSIM + property-assertion engine. |
| `crates/vrm-test-plan/` | YAML schema types. |
| `crates/vrm-validator-wrap/` | Subprocess wrapper for `mrxz/vrm-validator`. |
| `crates/vrm-s3/` | S3 manifest + push/pull tooling for goldens. |
| `adapters/vrm-metal-kit/` | Swift package: VRMMetalKit MCP adapter (macOS / Metal). |
| `test-plans/` | Generated and hand-authored test plans. |
| `goldens/manifest.json` | In-repo manifest pointing to S3-hosted golden images. |
| `site/` | Static comparison site (Vite + TS), deployed to GitHub Pages. |
| `rfcs/` | Architectural decision records. |
| `docs/` | Methodology hazards, operation contract, plans. |

## Getting started

See [CONTRIBUTING.md](./CONTRIBUTING.md).

## Acknowledgements

- The VRM Consortium and contributors to `vrm-c/vrm-specification`.
- Khronos's `glTF-Render-Fidelity` and `glTF-Asset-Generator` projects, whose methodology this work mirrors.
- Frans Bouma (`mrxz`) for the VRM validator we depend on.
- The maintainers of three-vrm, godot-vrm, UniVRM, and Babylon-VRM-Loader.
