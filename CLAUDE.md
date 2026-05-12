# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Cross-renderer conformance + render-fidelity infrastructure for VRM 1.0. Methodology-compatible with [KhronosGroup/glTF-Render-Fidelity](https://github.com/KhronosGroup/glTF-Render-Fidelity); architecture decisions optimize for clean future donation to the Khronos glTF WG.

Three components: (1) parametric `.vrm` test asset generator (Rust), (2) conformance runner driving renderer adapters through a uniform operation catalog (Rust), (3) static golden-image comparison site (Vite/TS). Renderer adapters live in-tree under `adapters/` (Swift, TypeScript).

Not a `.vrm` file-format validator — that's `mrxz/vrm-validator`, invoked via `crates/vrm-validator-wrap`.

## Common commands

Rust workspace (toolchain pinned to 1.88 via `rust-toolchain.toml`):

```bash
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check     # CI runs this
cargo clippy --workspace --all-targets -- -D warnings   # CI fails on any warning
cargo test -p <crate> <test_name>   # single test
cargo test --workspace -- --ignored   # validator-gated integration tests (require .tools/vrm-validator-cli; install via scripts/install-validator.sh)
```

Asset generator subcommands (every binary supports `describe --format json` and per-cmd `--json`):

```bash
cargo run -p vrm-asset-generator -- emit-default --id smoke --output-dir /tmp/out
cargo run -p vrm-asset-generator -- emit-sweep --output-dir /tmp/mtoon-sweep                    # ~44 MToon variants
cargo run -p vrm-asset-generator -- emit-springbone-sweep --output-dir /tmp/sb-sweep            # ~18 settle variants
cargo run -p vrm-asset-generator -- emit-springbone-swing-sweep --output-dir /tmp/swing-sweep   # ~18 swing variants
```

Runner:

```bash
cargo run -p vrm-runner -- execute-test-plan --plan <plan.yaml> --adapter-bin <binary> [--adapter-args ...] \
    --asset-dir <dir> --output-dir <dir> --renderer-name <name> [--reference <png>] [--json]
cargo run -p vrm-runner -- diff --plan <plan.yaml> --render <png> --reference <png> --renderer-name <name> --json
cargo run -p vrm-runner -- consensus-diff --plan <plan.yaml> --render <name>=<png> --render <name>=<png> ...
```

Mock renderer (deterministic CPU adapter — Phase 1 op contract, no GPU/Swift dependency; default smoke target):

```bash
cargo build --release -p vrm-mock-renderer    # → target/release/vrm-mock-renderer used as --adapter-bin
```

Adapter projects (each builds independently):

```bash
cd adapters/vrm-metal-kit && swift build && swift test    # macOS only; requires Xcode 26 (macOS 26 platform floor)
cd adapters/three-vrm && npm install && npm run build && npm test    # also: npx playwright install chromium (one-time, ~250 MB)
cd adapters/babylon-vrm && npm install && npm test
cd site && npm install && npm run dev          # Vite dev server; site reads goldens/manifest.json
```

End-to-end smoke and goldens scripts:

```bash
scripts/smoke.sh                  # asset gen → mock render → diff → site build; RUN_THREE_VRM=1 also exercises the TS adapter
scripts/bootstrap-goldens.sh      # full corpus through every available real adapter → goldens-cache/ + manifest
scripts/consensus-report.sh       # pairwise SSIM across the manifest; writes goldens-cache/consensus-report.json
scripts/pull-goldens.sh <dir>     # mirror manifest entries locally (handles both s3:// and file:// URLs)
scripts/install-validator.sh      # one-time install of mrxz/vrm-validator shim into .tools/
```

`QUICK=1`, `SKIP_THREE_VRM=1`, `SKIP_VRM_METAL_KIT=1`, `GOLDENS_DIR=path`, `VRM_GOLDENS_BUCKET=<bucket>` modify bootstrap behavior (see `CONTRIBUTING.md`). With `VRM_GOLDENS_BUCKET` unset, bootstrap writes `file://` URLs into the manifest — useful locally, but don't commit such a manifest.

## Architecture

### Agent-first surface contract — the load-bearing decision

Every binary (`vrm-asset-generator`, `vrm-runner`, every renderer adapter) is built around a **single core library** exposed as two transports backed by the same JSON Schema:

1. **Structured CLI** with `--json` I/O mode and a `describe` subcommand emitting the operation catalog.
2. **JSON-RPC stdio server** speaking the same operations. The MCP wrapper is a thin shim — not a separate protocol.

This is non-negotiable for new binaries. When adding an operation:

- Define it in `crates/vrm-ops/` (operation types + JSON Schema emission + JSON-RPC stdio transport). Both CLI and MCP wrappers depend on this crate; neither owns it.
- Never embed binary payloads (`.vrm`, `.png`, `.mov`) in stdout JSON — pass file paths or **BLAKE3 content-addressed refs** (`blake3:<64-hex>`). BLAKE3 is the only acceptable hash (chosen for composition with iroh-blobs / TDF refs).
- Long ops emit **NDJSON progress events on stderr**. Stdout is reserved for the structured result.
- Expensive ops decouple `plan-*` from `execute-*` so agents can preview cost before committing.
- Declare every reserved Phase 2+ op even when it returns the standard `Unimplemented` error (`-32000`, `data: { phase: "v1.x" }`) — adapters must agree on the surface even where capability differs.

See `docs/operation-contract.md` for the full op set, error envelope, and stdio framing (LSP-style `Content-Length` headers, same as MCP).

### Asset generator: paired triplets, never hand-authored plans

`vrm-asset-generator` emits `<asset>.vrm` + `<asset>.meta.json` + `<asset>.test.yaml` from one parameter dictionary. Single source of truth — no desync risk between asset and plan. Manual plans live only in `test-plans/manual/` for edge cases.

Sweeps are one-axis-at-a-time: every variant changes a single parameter against a baseline so regressions can be pinned without confounding. Default MToon is held constant across spring-bone variants and vice versa.

### Runner: drives an adapter through a YAML plan, then diffs

`vrm-runner execute-test-plan` spawns the adapter binary, drives it through `load_vrm → set_camera → set_lighting → set_post_processing → [reset_physics → animate_root_transform] → render → dispose`, then optionally runs `diff` against a `--reference` PNG. Diff = SSIM + bbox-relative property assertions (`crates/vrm-diff-engine/`). `consensus-diff` does N-way pairwise SSIM and flags outliers — there is **no oracle renderer**; consensus or a named reference decides.

The runner's `execute-test-plan` exit code is 0 when the pipeline ran; **pass/fail is signaled via `overall_passed` in the JSON output**. The standalone `diff` subcommand exits non-zero on failure for callers that want exit-gated CI.

### Goldens: S3, not git

`.vrm` and golden `.png` artifacts go to S3 — never to git LFS. `goldens/manifest.json` is the committed pointer file (with BLAKE3 hashes, host metadata, `image_url`). `goldens-cache/` is gitignored. CI does **not** re-render — renderer maintainers submit PNGs via PR; CI re-validates manifest schema, dimensions, and S3 presence. That's the trust model.

### Methodology pins (don't drift)

Cross-renderer pixel-exact comparison is impossible for non-PBR toon shading — these conventions are baseline; new tests must respect them or be a deliberate exception in `docs/methodology.md`:

- **MToon math tests**: `tone_mapping: none`, `cast_shadows: false`, `receive_shadows: false`. ACES/Filmic mangle non-PBR output; engine shadow noise is not a renderer bug worth flagging.
- **Outline tests**: MSAA 4×, wider local SSIM tolerance band on outline regions.
- **Spring-bone determinism**: 60 Hz fixed step, `reset_physics(settle_steps=30)` from rest pose before measurement.
- **Spring-bone excitation**: static `step_physics` only exercises gravity; testing drag/stiffness/inertia requires `animate_root_transform`. Swing-sweep corpus exists specifically for this.

### Crate map (the parts that matter)

- `crates/vrm-ops/` — op types, JSON Schema, JSON-RPC stdio transport. Foundation.
- `crates/vrm-asset-generator/` — emits paired triplets; sweep logic in `sweep.rs`, spring-bone in `spring_bone.rs`.
- `crates/vrm-runner/` — drives adapters (`adapter.rs`), executes plans (`execute.rs`, `plan_to_ops.rs`), runs diff/consensus (`diff.rs`).
- `crates/vrm-diff-engine/` — SSIM, property assertions, consensus math.
- `crates/vrm-test-plan/` — YAML schema types shared between generator and runner.
- `crates/vrm-mock-renderer/` — deterministic CPU adapter; identical params → byte-identical PNGs (self-diff is SSIM 1.0 by construction). Use this for E2E without GPU.
- `crates/vrm-validator-wrap/` — subprocess wrapper for `mrxz/vrm-validator`.
- `crates/vrm-s3/` — manifest validator (`validate-manifest` binary) + push/pull plumbing.

### Adapter status (changes — re-check when relevant)

- `adapters/vrm-metal-kit/` — Swift / Metal / macOS 26 platform floor. Pins a specific VRMMetalKit upstream revision in `Package.swift`; bump that revision deliberately as part of the change. CI builds (debug) but does not test (Xcode 26 binaries link against macOS 26 libs absent on `macos-15` runners — runtime coverage runs locally on M-series Macs / Xcode Cloud).
- `adapters/three-vrm/` — Phase 1 ops real; Playwright headless Chromium running three.js + three-vrm. Reserved ops return `Unimplemented`. Requires `npx playwright install chromium` after `npm install`.
- `adapters/babylon-vrm/` — L1+L2 scaffold; renderer integration deferred to L3. Exists to give consensus diff a third real renderer when ready.
- `adapters/godot-vrm/` — Godot 4 / GDScript paired with the `crates/vrm-godot-shim/` Rust shim. L3 (Phase 1 ops real); MToon corpus renders end-to-end. Phase 2 spring-bone ops still `Unimplemented` so spring-bone test plans skip this adapter. Runner consumes `target/release/vrm-godot-shim` as `--adapter-bin`. Requires Godot 4.x on `PATH`.

### CI guardrails

- `rust.yml` enforces `cargo fmt --check` and `cargo clippy -D warnings` — zero clippy warnings is a hard merge gate.
- `swift.yml` runs `sudo xcode-select -s /Applications/Xcode_26.3.app` before `swift build` (the default macOS runner ships Xcode 16).
- `manifest-validate.yml` runs `vrm-s3`'s `validate-manifest` binary on PRs that touch `goldens/manifest.json`.
- `site.yml` deploys `site/dist` to GitHub Pages on `main`.

## When in doubt

- Architectural decisions go through RFCs (`rfcs/`, template at `rfcs/template.md`).
- Cross-renderer findings (observed divergence beyond expected methodology hazards) are logged in `docs/findings.md` — that file is a deliverable, not a scratchpad.
- License: Apache 2.0 for code; generated assets are CC0.
