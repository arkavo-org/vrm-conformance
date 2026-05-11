# Contributing to vrm-conformance

Thanks for your interest. This project builds cross-renderer conformance tooling for VRM 1.0, methodology-compatible with [KhronosGroup/glTF-Render-Fidelity](https://github.com/KhronosGroup/glTF-Render-Fidelity) for future donation to the Khronos glTF Working Group.

## Quick start

```bash
# Rust core
cargo build --workspace
cargo test --workspace

# Emit the full MToon + spring-bone parameter sweeps (~90 assets) for local diff.
cargo run -p vrm-asset-generator -- emit-sweep --output-dir /tmp/mtoon-sweep
cargo run -p vrm-asset-generator -- emit-springbone-sweep --output-dir /tmp/springbone-sweep
# Same chains, plus an animate_root_transform swing block in every test plan:
cargo run -p vrm-asset-generator -- emit-springbone-swing-sweep --output-dir /tmp/springbone-swing-sweep

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

## Bootstrapping goldens (renderer maintainers, first-time setup)

To produce a fresh golden corpus across every real adapter on your machine — for local diffing, or as the seed for the first S3 publication — run:

```bash
./scripts/bootstrap-goldens.sh
```

This:

1. Generates the full test corpus locally (MToon sweep + spring-bone settle sweep + spring-bone swing sweep).
2. Builds every real adapter (`three-vrm`, `vrm-metal-kit` on macOS).
3. Runs each test plan through each adapter and writes the PNGs to `goldens-cache/<renderer>/<test_id>.png` (gitignored).
4. Populates `goldens/manifest.json` with one entry per (test_id, renderer) pair, including BLAKE3 hash, byte size, host metadata, and the `image_url`.

The script picks one of two publish modes based on env:

- **Local mode** (no `VRM_GOLDENS_BUCKET`): each `image_url` is a `file://<absolute-path>` URL pointing at the local PNG. The manifest is fully functional for local diffing — `pull-goldens.sh` handles `file://` URLs by copying — but it's host-specific, so don't commit it.
- **S3 mode** (`VRM_GOLDENS_BUCKET=<bucket>` + AWS creds in env): each PNG is uploaded to S3 and the manifest records `s3://` URLs. This is the manifest meant for git.

Useful env knobs:

- `QUICK=1` — render only 2 assets (emit-default + emit-springbone) to smoke-validate the pipeline without waiting through the full corpus.
- `SKIP_THREE_VRM=1` — skip the three-vrm adapter (e.g., on a machine where Playwright install is broken).
- `SKIP_VRM_METAL_KIT=1` — skip the vrm-metal-kit adapter (e.g., on Linux).
- `GOLDENS_DIR=path` — override the on-disk cache location.

Re-runnable: the script upserts manifest entries, so re-rendering a single (test_id, renderer) pair only updates that entry. Other entries are preserved.

## Pulling goldens for offline diff

If you want to run the diff engine locally against the published golden corpus without re-rendering, pull every PNG to a local mirror:

```bash
./scripts/pull-goldens.sh /tmp/goldens-mirror
```

This reads `goldens/manifest.json`, downloads each entry to `/tmp/goldens-mirror/<test_id>/<renderer_name>.png`, and verifies BLAKE3 content addressing. A hash mismatch exits non-zero with a clear pointer at the bad entry.

Both `s3://` and `file://` URLs are supported in the manifest — the pull is a real S3 GetObject in the former case and a local file copy in the latter, so the same workflow works against both a published corpus and a freshly-bootstrapped local one.

## Corpus-wide consensus report

After bootstrapping, run the consensus report to find every test_id where renderers diverge:

```bash
./scripts/consensus-report.sh
```

This walks the manifest, invokes `vrm-runner consensus-diff` for every test_id, and writes a single JSON report at `goldens-cache/consensus-report.json` plus a human-readable summary on stdout (pairwise SSIM stats + top-N most-divergent test_ids).

Findings from running it are recorded in [`docs/findings.md`](./docs/findings.md). Each top-divergent test_id is a candidate for an upstream issue (or a methodology refinement when the divergence is legitimate).

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
