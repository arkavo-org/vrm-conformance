# Contributing to vrm-conformance

Thanks for your interest. This project builds cross-renderer conformance tooling for VRM 1.0, methodology-compatible with [KhronosGroup/glTF-Render-Fidelity](https://github.com/KhronosGroup/glTF-Render-Fidelity) for future donation to the Khronos glTF Working Group.

## Quick start

```bash
# Rust core
cargo build --workspace
cargo test --workspace

# Emit the full MToon + spring-bone parameter sweeps (~70 assets) for local diff.
cargo run -p vrm-asset-generator -- emit-sweep --output-dir /tmp/mtoon-sweep
cargo run -p vrm-asset-generator -- emit-springbone-sweep --output-dir /tmp/springbone-sweep

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

## Pulling goldens for offline diff

If you want to run the diff engine locally against the published golden corpus without re-rendering, pull every PNG to a local mirror:

```bash
./scripts/pull-goldens.sh /tmp/goldens-mirror
```

This reads `goldens/manifest.json`, downloads each entry from S3 to `/tmp/goldens-mirror/<test_id>/<renderer_name>.png`, and verifies BLAKE3 content addressing. A hash mismatch exits non-zero with a clear pointer at the bad entry.

Then drive the runner's diff against a local render:

```bash
cargo run -p vrm-runner -- diff \
  --plan path/to/plan.yaml \
  --render path/to/your-render.png \
  --reference /tmp/goldens-mirror/<test_id>/<renderer_name>.png \
  --json
```

Requires AWS credentials with `s3:GetObject` on the bucket(s) referenced by the manifest. Reviewers without write access can request a read-only IAM role.

## RFCs

Architectural decisions go through RFCs in `rfcs/`. Use `rfcs/template.md` to draft a new one.

## License

Apache 2.0. Generated assets are CC0. By contributing you agree your contributions are licensed under the same terms.
