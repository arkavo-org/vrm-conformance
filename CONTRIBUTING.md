# Contributing to vrm-conformance

Thanks for your interest. This project builds cross-renderer conformance tooling for VRM 1.0, methodology-compatible with [KhronosGroup/glTF-Render-Fidelity](https://github.com/KhronosGroup/glTF-Render-Fidelity) for future donation to the Khronos glTF Working Group.

## Quick start

```bash
# Rust core
cargo build --workspace
cargo test --workspace

# VRMMetalKit adapter (macOS only)
cd adapters/vrm-metal-kit && swift build && swift test

# Site (Node 20+)
cd site && npm install && npm run dev
```

## Repository structure

This is a polyglot monorepo. See [README.md](./README.md) for the layout.

## Surface contract — agent-first

Every binary in this project (`vrm-asset-generator`, `vrm-runner`, every renderer adapter) is built around a single core library exposed as **two transports**:

1. A structured CLI with `--json` I/O mode and a `describe` subcommand emitting the operation catalog as JSON Schema.
2. A JSON-RPC stdio server speaking the same operations (the MCP wrapper is a thin shim over this).

Schemas are the source of truth. Binary payloads are passed via file paths or BLAKE3 content-addressed refs — never embedded in JSON. Long ops emit NDJSON progress on stderr. See [`docs/operation-contract.md`](./docs/operation-contract.md).

## Submitting renderer renders

Renderer maintainers submit golden images via PR. The workflow:

1. Run the conformance runner locally against the published asset corpus.
2. Open a PR adding renders to S3 (via `scripts/push-goldens.sh`) and updating `goldens/manifest.json`.
3. CI re-validates manifest schema, dimensions, file presence on S3. **CI does not re-render** — that is a deliberate trust model.

Each submission must include the metadata schema enforced by CI:

- `renderer_name`, `renderer_version`, `git_hash`
- `os`, `os_version`
- `gpu_vendor`, `gpu_model`, `driver_version`
- `build_flags`

PRs missing or malformed metadata are auto-rejected.

## RFCs

Architectural decisions go through RFCs in `rfcs/`. Use `rfcs/template.md` to draft a new one.

## License

Apache 2.0. Generated assets are CC0. By contributing you agree your contributions are licensed under the same terms.
