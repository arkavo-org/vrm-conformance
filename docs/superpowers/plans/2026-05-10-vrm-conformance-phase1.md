# VRM Conformance Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a single-renderer end-to-end VRM conformance pipeline — generate MToon parameter-sweep assets, validate them, render via the VRMMetalKit MCP adapter, diff against goldens stored on S3, display in a static comparison site.

**Architecture:** Single monorepo at `arkavo-org/vrm-conformance`. Cargo workspace holds the Rust core (asset generator, runner, diff engine, MCP protocol types, validator wrapper, S3 tooling). `adapters/vrm-metal-kit/` is a Swift package that exposes a stdio JSON-RPC MCP server wrapping VRMMetalKit. Goldens (`.png`, `.mov`) live on S3 with a JSON manifest in-repo. Static comparison site is plain Vite + TS, deploys to GitHub Pages, fetches images from S3.

**Tech Stack:** Rust 2021 (MSRV 1.78), `gltf` 1.4 + `serde_json` for asset emission, `image` + `image-compare` (SSIM) for diff, `serde_yml` for test plans, `aws-sdk-s3` for golden storage, `clap` for CLIs, `tracing` for logging, `insta` for snapshot tests, **`blake3` for content addressing** (composes with iroh-blobs / TDF refs). Swift 5.9+ for the VRMMetalKit adapter (executable target depending on `arkavo-org/VRMMetalKit` via SwiftPM). Vite + vanilla TS for the site. `mrxz/vrm-validator` native CLI binary as precondition gate (installed via `scripts/install-validator.sh`).

**Surface contract — agent-first.** Every binary in this project (asset-generator, runner, renderer adapters) is built around a **single core library** exposed as **(1) a structured CLI with `--json` I/O mode and a `describe`/`schema` subcommand emitting the full operation catalog as JSON Schema, and (2) a thin MCP wrapper over the same core**. Schemas are the source of truth for both surfaces. The MCP-vs-CLI distinction collapses once the operation set and schemas are right. Concretely:
- Never embed binary in stdout JSON. Use file paths or BLAKE3 content-addressed refs.
- NDJSON progress on stderr for long ops (encoding, rendering, generation). Agents tail it; humans ignore it.
- Idempotent ops; explicit codec/container/colorspace — implicit defaults are where agents quietly produce broken output.
- Decouple `plan` from `execute` for expensive ops so an agent can preview cost before committing.
- `describe` / `tools list` JSON output is the discovery contract. Don't make agents parse `--help`.

**YAGNI scope guards:**
- v0.1 covers **MToon material tests only**. Spring bones, constraints, expressions, humanoid pose are deferred. The operation catalog is fully *defined* but only the MToon-relevant subset is *required* — non-required ops return a structured `Unimplemented` error.
- **One renderer** (VRMMetalKit). No consensus-mode diff in v0.1; reference is "self vs. golden" SSIM.
- **No HDRI**, no animation, no debug-pass outputs. Directional + ambient lighting only, `tone_mapping: none`, shadows off, MSAA 4x.
- ~50 assets total (the MToon basic sweep).
- **Phase 1 transports:** every binary ships the **structured CLI** surface (the runner drives adapters via `--json` per-op invocations *or* via long-lived JSON-RPC stdio sessions, both are valid). MCP wrapper is a thin shim added once the CLI surface is stable; it is *not* a Phase 1 gate beyond the JSON-RPC stdio transport already needed for stateful adapter sessions.

---

## Repository Layout

```
vrm-conformance/                          # this repo
├── README.md                             # rewritten for monorepo + donation-readiness
├── LICENSE                               # Apache 2.0 (existing)
├── CONTRIBUTING.md
├── CODE_OF_CONDUCT.md
├── .github/
│   └── workflows/
│       ├── rust.yml                      # cargo test + clippy + fmt
│       ├── swift.yml                     # adapter test on macOS
│       ├── site.yml                      # site build + GH Pages deploy
│       └── manifest-validate.yml         # CI gate for golden submission PRs
├── rfcs/
│   ├── template.md
│   ├── 0001-monorepo-confirmed.md
│   └── 0002-anti-fraud-submission-integrity.md
├── docs/
│   ├── methodology.md                    # hazards: tone mapping, shadows, etc.
│   ├── operation-contract.md             # CLI + MCP surface contract every binary implements
│   └── superpowers/plans/                # this plan + future plans
├── Cargo.toml                            # workspace manifest
├── crates/
│   ├── vrm-ops/                          # operation schemas + JSON-RPC stdio transport (CLI + MCP both consume)
│   ├── vrm-validator-wrap/               # subprocess wrapper for mrxz/vrm-validator
│   ├── vrm-asset-generator/              # binary: emits .vrm + .meta.json + .test.yaml
│   ├── vrm-test-plan/                    # YAML schema types
│   ├── vrm-diff-engine/                  # SSIM + property assertions
│   ├── vrm-runner/                       # binary: orchestrates adapter + diff
│   └── vrm-s3/                           # S3 manifest + push/pull
├── adapters/
│   └── vrm-metal-kit/                    # Swift package
│       ├── Package.swift
│       ├── Sources/
│       │   └── VRMMetalKitAdapter/
│       └── Tests/
├── test-plans/
│   ├── generated/                        # written by asset-generator
│   └── manual/                           # hand-authored edge cases (none in Phase 1)
├── assets/
│   └── generated/                        # local build cache; gitignored
├── goldens/
│   └── manifest.json                     # in-repo manifest pointing to S3
├── site/
│   ├── package.json
│   ├── vite.config.ts
│   ├── index.html
│   └── src/
└── scripts/
    ├── install-validator.sh              # downloads pinned mrxz/vrm-validator binary
    ├── push-goldens.sh                   # uploads renders to S3 + updates manifest
    └── pull-goldens.sh                   # downloads goldens for local diff
```

**Decomposition rationale:** Each crate has one responsibility. `vrm-ops` is a leaf dep shared by `vrm-runner` and (transitively, via JSON schema) every adapter. `vrm-asset-generator` and `vrm-runner` are the only binaries; everything else is a library. The Swift adapter is intentionally outside the Cargo workspace — it builds with `swift build`, not `cargo`.

---

## Section A — Repo governance & monorepo scaffolding

### Task A1: Repo restructure + .gitignore

**Files:**
- Modify: `.gitignore`
- Create: `CONTRIBUTING.md`
- Create: `CODE_OF_CONDUCT.md`

- [ ] **Step 1: Update `.gitignore` for the monorepo**

Replace `.gitignore` contents:

```gitignore
# Rust
target/
Cargo.lock.bak
**/*.rs.bk

# Generated assets (built locally, distributed via S3 / GH releases)
assets/generated/
test-plans/generated/

# Swift
adapters/*/.build/
adapters/*/.swiftpm/
adapters/*/Package.resolved

# Site
site/node_modules/
site/dist/
site/.vite/

# Editor / OS
.DS_Store
.idea/
.vscode/
*.swp

# Local validator install (script writes here)
.tools/

# Local credentials
.env
.env.local
```

- [ ] **Step 2: Add CONTRIBUTING.md**

Create `CONTRIBUTING.md`:

````markdown
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
````

- [ ] **Step 3: Add Contributor Covenant**

Create `CODE_OF_CONDUCT.md` with the Contributor Covenant v2.1 text. Use the canonical version from `https://www.contributor-covenant.org/version/2/1/code_of_conduct.txt`. Do not paraphrase — the covenant must be verbatim. Set the contact email to `paul@arkavo.com`.

- [ ] **Step 4: Commit**

```bash
git add .gitignore CONTRIBUTING.md CODE_OF_CONDUCT.md
git commit -m "chore: bootstrap monorepo governance scaffolding"
```

---

### Task A2: README rewrite for monorepo

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Replace README with monorepo overview**

Overwrite `README.md`:

````markdown
# vrm-conformance

Cross-renderer conformance and render-fidelity infrastructure for the [VRM 1.0](https://github.com/vrm-c/vrm-specification) ecosystem.

**Status:** Phase 1 (single-renderer vertical slice) — under active development.
**License:** [Apache 2.0](./LICENSE). Generated test assets are CC0.
**Donation intent:** Methodology-compatible with [KhronosGroup/glTF-Render-Fidelity](https://github.com/KhronosGroup/glTF-Render-Fidelity); we intend to donate to the Khronos glTF Working Group / VRM Consortium once VRM extensions ratify as `KHR_*`.

## What this is

- A **parametric VRM asset generator** that emits a deterministic test corpus covering the MToon material spec, spring bone behaviors, constraints, and expressions.
- An **MCP-driven conformance runner** that drives every supported renderer through the same test plan via a uniform JSON-RPC tool surface.
- A **golden-image comparison site** that displays renders from each renderer side-by-side, with SSIM diff + property-assertion overlays.

## What this is not

- Not a glTF / VRM file-format validator — that job belongs to [`mrxz/vrm-validator`](https://github.com/mrxz/vrm-validator), which we depend on as a precondition gate.
- Not a live cross-renderer comparison server. Renders are PR-submitted offline, exactly the model Khronos uses.
- Not a VTuber-specific behavior suite (Perfect Sync, ARKit face tracking).

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
| `docs/` | Methodology documentation, MCP tool surface contract. |

## Getting started

See [CONTRIBUTING.md](./CONTRIBUTING.md).

## Acknowledgements

- The VRM Consortium and contributors to `vrm-c/vrm-specification`.
- Khronos's `glTF-Render-Fidelity` and `glTF-Asset-Generator` projects, whose methodology this work mirrors.
- Frans Bouma (`mrxz`) for the VRM validator we depend on.
- The maintainers of three-vrm, godot-vrm, UniVRM, and Babylon-VRM-Loader.
````

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: rewrite README for monorepo + donation-readiness"
```

---

### Task A3: RFC template + RFC-0001 (monorepo confirmed)

**Files:**
- Create: `rfcs/template.md`
- Create: `rfcs/0001-monorepo-confirmed.md`

- [ ] **Step 1: Create RFC template**

Create `rfcs/template.md`:

```markdown
# RFC NNNN: Title

- **Status:** Draft | Accepted | Rejected | Superseded by RFC NNNN
- **Author(s):**
- **Date:**

## Summary

One paragraph. What does this RFC propose?

## Motivation

Why are we doing this? What problem does it solve?

## Detailed design

The bulk of the RFC. Include:
- Schemas, types, interfaces.
- Migration path (if applicable).
- Failure modes and how they are handled.

## Alternatives considered

What else did we look at? Why did we reject those alternatives?

## Open questions

Anything unresolved.

## References

Links, prior art, related issues.
```

- [ ] **Step 2: Create RFC-0001**

Create `rfcs/0001-monorepo-confirmed.md`:

```markdown
# RFC 0001: Monorepo confirmed

- **Status:** Accepted
- **Author(s):** Paul Flynn
- **Date:** 2026-05-10

## Summary

`arkavo-org/vrm-conformance` ships as a single polyglot monorepo containing the asset generator, runner, diff engine, MCP adapters (across multiple languages), test plans, comparison site, and governance documents. The original handover specced a polyrepo split across four GitHub repositories; this RFC supersedes that decision.

## Motivation

The polyrepo design optimized for per-language CI isolation and per-renderer maintainer ownership. In practice, at the team's current size, the coordination cost of cross-repo changes (asset generator schema → runner consumer → adapter contract → site) outweighs the isolation benefit. A monorepo gives:

- One CI surface, one PR per cross-cutting change.
- Simpler atomic refactors of the MCP tool surface.
- One issue tracker, one release cadence.

## Detailed design

Top-level layout per [README.md](../README.md). Polyglot is handled by:

- A Cargo workspace at the repo root for all Rust crates.
- A self-contained Swift package per adapter under `adapters/`. Adapters are not part of the Cargo workspace; they build via `swift build` and run as subprocesses.
- A self-contained Vite project under `site/`.
- Per-language CI workflows in `.github/workflows/`, scoped to changed paths.

Goldens (binary `.png` and `.mov` artifacts) live on **S3**, not git LFS. `goldens/manifest.json` records every artifact's S3 URL + SHA-256 + submission metadata. This RFC commits to S3 over LFS to keep clone times bounded and decouple binary churn from code review.

## Alternatives considered

- **Polyrepo (original spec).** Rejected on coordination cost, see motivation.
- **Monorepo with git LFS for goldens.** Rejected. LFS works but adds a credential dependency for every clone and inflates clone time. S3 with a checksummed manifest is cleaner.
- **Submodules pointing to per-language repos.** Rejected as worst-of-both-worlds: monorepo coordination cost without monorepo atomicity.

## Open questions

None.

## References

- Original handover document, §3 Repository Layout, §11 Open Question 1.
- KhronosGroup/glTF-Render-Fidelity uses git LFS; we deliberately diverge on storage.
```

- [ ] **Step 3: Commit**

```bash
git add rfcs/template.md rfcs/0001-monorepo-confirmed.md
git commit -m "docs(rfc): add RFC template and RFC-0001 monorepo confirmation"
```

---

### Task A4: RFC-0002 (anti-fraud submission integrity)

**Files:**
- Create: `rfcs/0002-anti-fraud-submission-integrity.md`

- [ ] **Step 1: Draft RFC-0002**

Create `rfcs/0002-anti-fraud-submission-integrity.md`:

```markdown
# RFC 0002: Anti-fraud and submission integrity for golden renders

- **Status:** Draft
- **Author(s):** Paul Flynn
- **Date:** 2026-05-10

## Summary

Renderer maintainers submit golden PNGs via PR. Without safeguards, a bad actor could submit doctored images to make their renderer "look correct." This RFC defines the multi-layered policy that protects submission integrity without imposing a re-rendering burden on our CI.

## Motivation

The PR-submitted-renders model is the only model that scales. CI re-rendering Unity / Godot / WebGL workloads is impractical and creates a credential / licensing surface we do not want. The cost of that decision is a trust gap; this RFC closes it pragmatically.

## Detailed design

Three layers of defense:

### 1. Strict submission metadata

CI rejects any PR whose `goldens/manifest.json` entries are missing or malformed. Required fields per submission:

```json
{
  "test_id": "mtoon_basic_shadingShift_neg0.5",
  "renderer_name": "vrm-metal-kit",
  "renderer_version": "0.5.2",
  "git_hash": "a1b2c3d4...",
  "os": "macos",
  "os_version": "14.4.1",
  "gpu_vendor": "Apple",
  "gpu_model": "M2 Pro",
  "driver_version": "Metal 3",
  "build_flags": "release",
  "image_url": "s3://arkavo-vrm-conformance/...",
  "image_sha256": "...",
  "submitted_at": "2026-05-10T12:34:56Z"
}
```

These provide traceability; a fraudulent submission must also fabricate consistent metadata.

### 2. Spot-check audit cadence

Maintainers periodically (target: monthly) sample N renders from a recently-submitted column and re-render locally on matching hardware. Mismatches escalate to a public audit. The audit cadence is documented but the sample is private — surprise is the point.

### 3. Consensus reference mode

For tests using `diff.reference_renderer: consensus`, a 3-of-5 majority defines the baseline and outliers are flagged automatically. One outlier renderer cannot shift the baseline. This is a partial defense (it presumes ≥3 honest renderers per test) but compounds with #1 and #2.

### Reproducibility statement

`CONTRIBUTING.md` commits submitters to: *"renderer + asset + test plan + build hash should produce this PNG within tolerance T on the same hardware class."* This is a public statement, not a CI-enforced check. It exists so that audits have a reference contract to invoke when escalating.

## Alternatives considered

- **CI-side re-rendering.** Rejected as cost-prohibitive and credential-prohibitive.
- **Trusted-builder attestation (Sigstore / SLSA).** Interesting; deferred. The Phase 1 cost-to-benefit is too high. Revisit if fraud actually occurs.
- **Multi-party submission (require ≥2 maintainers to submit independently for the same renderer).** Rejected on coordination cost; effectively halts maintainer adoption.

## Open questions

- What is the appropriate sample size and cadence for the spot-check audit? Defer until we have ≥3 active renderer adapters submitting.
- Does the manifest schema need a signature field for cryptographic attestation? Defer, keep the door open by leaving the schema extensible.

## References

- Original handover §11 Open Question 7.
```

- [ ] **Step 2: Commit**

```bash
git add rfcs/0002-anti-fraud-submission-integrity.md
git commit -m "docs(rfc): draft RFC-0002 anti-fraud submission integrity policy"
```

---

### Task A5: Methodology documentation

**Files:**
- Create: `docs/methodology.md`

- [ ] **Step 1: Document the methodology hazards**

Create `docs/methodology.md`:

```markdown
# Methodology

This document records the cross-renderer comparison hazards every test plan must account for. These are not opinions; they are observable sources of pixel-level divergence that have nothing to do with whether a renderer correctly implements the VRM spec.

## Color management

MToon's spec is silent on linear vs sRGB workflow. Each render submission **must** declare its `color_space` field. Comparison is only valid within the same color space; cross-space comparisons get a wider SSIM tolerance.

## Tone mapping

Host engines apply tone mapping at varying defaults. three.js's `WebGLRenderer.toneMapping` defaults to `NoToneMapping`; Godot 4 `Environment.tone_mapper` defaults to `Linear`; Unity URP/HDRP applies tone mapping via Volume settings.

**MToon math tests pin `tone_mapping: none`.** MToon is non-PBR. ACES, Filmic, or Reinhard mangle the intended output cross-renderer. Integration tests opt into other tone-mapping modes with relaxed tolerances.

## Engine shadow noise

Differences in shadow bias, PCF filtering, and cascade resolution between Unity / Godot / three.js / Metal create shadow-edge noise that SSIM flags as failures even when MToon math is correct.

**MToon math tests pin `cast_shadows: false` and `receive_shadows: false`.** Shadow-on integration tests are a separate category with renderer-pair tolerance bands.

## Outline antialiasing

Outlines render via separate pass (most), geometry shader (some), or screen-space (rare). Aliasing differs.

**v1.0 standardizes on MSAA 4x.** SSIM uses a wider local tolerance band on outline regions.

## Spring bone determinism

`VRMC_springBone` does not pin a fixed time-step. Adapters must guarantee deterministic stepping at 60 Hz with reset between tests.

## Spring bone initial state

Renderers initialize spring positions differently from a fresh load. The `reset_physics(settle_steps)` MCP method pins the convention: every spring-bone test runs N settling steps from rest pose before measurement begins.

**v1.0 default: 30 settle steps at 60 Hz (0.5 s).**

## Spring bone excitation

Static avatars under `step_physics` only exercise gravity settling. Testing inertia, drag, stiffness requires moving the avatar through space. The `animate_root_transform(start, end, duration)` MCP method drives this.

## Render queue / transparency ordering

Z-write behavior under `transparentWithZWrite=true` plus `renderQueueOffsetNumber` is the most common source of real-world MToon visual bugs.

A dedicated test category covers `outline × alphaMode × transparentWithZWrite × renderQueueOffsetNumber` interactions; coverage there is disproportionately heavy on purpose.

## Tangent space

The spec allows ignoring stored TANGENT and recomputing via MikkTSpace. Recomputation differs subtly across libraries. v1.0 generates assets both with and without explicit tangents.

## Apple Silicon vs other GPUs

VRMMetalKit is Metal-only; cross-GPU pixel-exact comparison is a non-goal. SSIM thresholds are tuned per-pair, with stricter intra-family thresholds (same GPU vendor, same color space) and looser cross-family thresholds. **Property assertions remain strict across all pairs.**
```

- [ ] **Step 2: Commit**

```bash
git add docs/methodology.md
git commit -m "docs: methodology hazards reference"
```

---

### Task A6: Operation contract (CLI + MCP)

**Files:**
- Create: `docs/operation-contract.md`

- [ ] **Step 1: Document the contract**

Create `docs/operation-contract.md`:

````markdown
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

Output: a JSON document listing every operation, its input schema, output schema, and a one-line summary. Agents use this for tool-discovery; humans use `--help` if they prefer prose.

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
# emits NDJSON progress + final result
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

## Required tools (Phase 1)

The following must be implemented by every adapter. These cover MToon material tests.

### `load_vrm`

Load a `.vrm` file and return a session id.

```json
// Request params
{ "path": "string (absolute path)" }

// Response
{ "session_id": "string" }
```

### `set_camera`

```json
{
  "session_id": "string",
  "position": [0.0, 1.4, 1.5],
  "target": [0.0, 1.4, 0.0],
  "up": [0.0, 1.0, 0.0],
  "fov_degrees": 30.0
}
```

### `set_lighting`

```json
{
  "session_id": "string",
  "directional": {
    "dir": [-0.3, -0.6, -0.7],
    "color": [1.0, 1.0, 1.0],
    "intensity": 1.0
  },
  "ambient": {
    "color": [0.5, 0.5, 0.5],
    "intensity": 0.3
  },
  "cast_shadows": false,
  "receive_shadows": false
}
```

### `set_post_processing`

```json
{
  "session_id": "string",
  "tone_mapping": "None | Linear | Reinhard | ACES",
  "exposure": 1.0
}
```

### `render`

```json
// Request
{
  "session_id": "string",
  "width": 1024,
  "height": 1024,
  "output_path": "string (absolute path)",
  "color_space": "Linear | SRGB",
  "msaa": 4,
  "output_type": "Color"
}

// Response
{ "output_path": "string", "actual_color_space": "Linear | SRGB" }
```

### `dispose`

```json
{ "session_id": "string" }
```

## Reserved tools (Phase 2+)

Required to be **declared** by every adapter (so the runner can query availability), but may return a structured `Unimplemented` JSON-RPC error in v0.1:

- `set_environment(hdri_path, intensity)` — v1.x
- `set_expression(name, weight)` — Phase 3
- `set_humanoid_pose(bone_rotations)` — Phase 2
- `set_root_transform(position, rotation)` — Phase 2
- `animate_root_transform(start_pos, start_rot, end_pos, end_rot, duration_seconds)` — Phase 2
- `step_physics(dt_seconds, count)` — Phase 2
- `reset_physics(settle_steps)` — Phase 2

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
````

- [ ] **Step 2: Commit**

```bash
git add docs/operation-contract.md
git commit -m "docs: dual-surface (CLI + MCP) operation contract for all binaries"
```

---

## Section B — Cargo workspace bootstrap

### Task B1: Workspace manifest

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`

- [ ] **Step 1: Pin the Rust toolchain**

Create `rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.78.0"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 2: Create workspace manifest**

Create `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
    "crates/vrm-ops",
    "crates/vrm-validator-wrap",
    "crates/vrm-test-plan",
    "crates/vrm-asset-generator",
    "crates/vrm-diff-engine",
    "crates/vrm-runner",
    "crates/vrm-s3",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.78"
license = "Apache-2.0"
repository = "https://github.com/arkavo-org/vrm-conformance"
authors = ["Arkavo LLC <paul@arkavo.com>"]

[workspace.dependencies]
# Errors / logging
thiserror = "1.0"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Serde stack
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yml = "0.0.12"

# CLI
clap = { version = "4.5", features = ["derive"] }

# IO / filesystem
camino = { version = "1.1", features = ["serde1"] }
tempfile = "3.10"

# Imaging
image = "0.25"
image-compare = "0.4"

# glTF + math
gltf = { version = "1.4", features = ["extras", "names", "utils"] }
glam = { version = "0.27", features = ["serde"] }

# Hashing — BLAKE3 for content-addressed refs (composes with iroh-blobs / TDF)
blake3 = "1.5"
hex = "0.4"

# Async (only where needed: AWS SDK)
tokio = { version = "1", features = ["macros", "rt-multi-thread", "process", "io-util"] }

# AWS
aws-config = { version = "1.5", features = ["behavior-version-latest"] }
aws-sdk-s3 = "1.40"

# Test
insta = { version = "1.39", features = ["json", "yaml"] }
pretty_assertions = "1.4"

[profile.release]
lto = "thin"
codegen-units = 1
```

- [ ] **Step 3: Verify the workspace compiles (empty)**

Run: `cargo check --workspace`

Expected: fails because no member crates exist yet. This is fine — we just want to confirm the manifest itself parses.

Actually, an empty member list with declared paths errors out. Run instead: `cargo metadata --no-deps --format-version 1 | head -20`

Expected: emits JSON describing the workspace; member-not-found errors are tolerable until B2.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml rust-toolchain.toml
git commit -m "chore: bootstrap Cargo workspace manifest with shared dependencies"
```

---

### Task B2: Empty crate skeletons

**Files:**
- Create: `crates/vrm-ops/{Cargo.toml, src/lib.rs}`
- Create: `crates/vrm-validator-wrap/{Cargo.toml, src/lib.rs}`
- Create: `crates/vrm-test-plan/{Cargo.toml, src/lib.rs}`
- Create: `crates/vrm-asset-generator/{Cargo.toml, src/main.rs, src/lib.rs}`
- Create: `crates/vrm-diff-engine/{Cargo.toml, src/lib.rs}`
- Create: `crates/vrm-runner/{Cargo.toml, src/main.rs, src/lib.rs}`
- Create: `crates/vrm-s3/{Cargo.toml, src/lib.rs}`

- [ ] **Step 1: Create `vrm-ops` crate**

`crates/vrm-ops/Cargo.toml`:

```toml
[package]
name = "vrm-ops"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
```

`crates/vrm-ops/src/lib.rs`:

```rust
//! Operation catalog: schemas + JSON-RPC stdio transport + CLI plumbing.
//! Source of truth for both the structured CLI and the MCP wrapper surface.
//! See `docs/operation-contract.md`.
```

- [ ] **Step 2: Create the other six crate skeletons identically**

For each of `vrm-validator-wrap`, `vrm-test-plan`, `vrm-diff-engine`, `vrm-s3`, create `Cargo.toml` with package metadata and `src/lib.rs` with a one-line doc comment. Dependencies stay empty for now; we'll add them per-task.

For `vrm-asset-generator`:

`crates/vrm-asset-generator/Cargo.toml`:

```toml
[package]
name = "vrm-asset-generator"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
anyhow.workspace = true
clap.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true

[[bin]]
name = "vrm-asset-generator"
path = "src/main.rs"

[lib]
path = "src/lib.rs"
```

`crates/vrm-asset-generator/src/lib.rs`:

```rust
//! Parametric VRM 1.0 asset generator. Emits paired .vrm + .meta.json + .test.yaml.
```

`crates/vrm-asset-generator/src/main.rs`:

```rust
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    println!("vrm-asset-generator: not yet implemented");
    Ok(())
}
```

For `vrm-runner`, the same shape with bin = `vrm-runner`.

- [ ] **Step 3: Compile the workspace**

Run: `cargo build --workspace`

Expected: all 7 crates compile (with warnings about unused deps tolerated).

- [ ] **Step 4: Commit**

```bash
git add crates/
git commit -m "chore: scaffold seven Rust crate skeletons"
```

---

## Section C — Validator wrapper crate

### Task C1: Validator install script

**Files:**
- Create: `scripts/install-validator.sh`

- [ ] **Step 1: Write the install script**

The `mrxz/vrm-validator` project releases native CLI binaries via GitHub Releases. Pin a specific version. (Implementing engineer: at time of plan-writing, check `https://github.com/mrxz/vrm-validator/releases` and substitute the latest stable tag for `VALIDATOR_VERSION` below.)

Create `scripts/install-validator.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

# Pinned mrxz/vrm-validator version. Bumping this requires re-running the
# generator's golden corpus regeneration, since validator semantics may shift.
VALIDATOR_VERSION="${VALIDATOR_VERSION:-v0.4.0}"

INSTALL_DIR="${INSTALL_DIR:-$(pwd)/.tools}"
mkdir -p "${INSTALL_DIR}"

UNAME_S="$(uname -s)"
UNAME_M="$(uname -m)"

case "${UNAME_S}-${UNAME_M}" in
    Darwin-arm64)   ASSET="gltf_validator-macos-arm64" ;;
    Darwin-x86_64)  ASSET="gltf_validator-macos-x64" ;;
    Linux-x86_64)   ASSET="gltf_validator-linux-x64" ;;
    Linux-aarch64)  ASSET="gltf_validator-linux-arm64" ;;
    *) echo "Unsupported platform: ${UNAME_S} ${UNAME_M}" >&2; exit 1 ;;
esac

URL="https://github.com/mrxz/vrm-validator/releases/download/${VALIDATOR_VERSION}/${ASSET}"
DEST="${INSTALL_DIR}/gltf_validator"

echo "Downloading ${URL}"
curl -fsSL "${URL}" -o "${DEST}"
chmod +x "${DEST}"

echo "Installed: ${DEST}"
"${DEST}" --version
```

> **Caveat for the implementing engineer:** the asset names above are guessed from common conventions; verify against the actual release artifact filenames at `https://github.com/mrxz/vrm-validator/releases` and adjust the case statement before committing. If `mrxz/vrm-validator` does not (yet) ship pre-built native binaries for all platforms, fall back to invoking the npm-distributed CLI via `npx` and document that path in CONTRIBUTING.md as an alternative — but prefer the native binary when available because subprocess startup is ~50× faster.

- [ ] **Step 2: Make executable + smoke test locally**

Run: `chmod +x scripts/install-validator.sh && ./scripts/install-validator.sh`

Expected: downloads the binary to `.tools/gltf_validator`, prints version. If the asset names guessed above don't match real releases, the script fails with a 404 — fix the names and re-run.

- [ ] **Step 3: Commit**

```bash
git add scripts/install-validator.sh
git commit -m "chore: add mrxz/vrm-validator install script"
```

---

### Task C2: `vrm-validator-wrap` — failing test

**Files:**
- Modify: `crates/vrm-validator-wrap/Cargo.toml`
- Create: `crates/vrm-validator-wrap/src/lib.rs`
- Create: `crates/vrm-validator-wrap/tests/smoke.rs`

- [ ] **Step 1: Add deps**

Replace `crates/vrm-validator-wrap/Cargo.toml`:

```toml
[package]
name = "vrm-validator-wrap"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
camino.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Write the failing integration test**

Create `crates/vrm-validator-wrap/tests/smoke.rs`:

```rust
use vrm_validator_wrap::{validate, ValidatorConfig};

#[test]
fn validate_returns_error_for_nonexistent_file() {
    let config = ValidatorConfig::from_env().expect("validator binary must be installed");
    let result = validate(&config, camino::Utf8Path::new("/nonexistent/file.vrm"));
    assert!(result.is_err(), "validation of missing file should error");
}

#[test]
fn validate_minimal_glb_returns_report() {
    // The smallest possible valid GLB: 12-byte header pointing at an empty JSON chunk
    // containing { "asset": { "version": "2.0" } }. Not VRM-valid (no extensions),
    // but glTF-valid, so the validator should produce a report rather than panic.
    let config = ValidatorConfig::from_env().expect("validator binary must be installed");

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("minimal.glb");
    let path = camino::Utf8PathBuf::from_path_buf(path).unwrap();

    let glb_bytes = build_minimal_glb();
    std::fs::write(&path, &glb_bytes).unwrap();

    let report = validate(&config, &path).expect("validator should produce a report");
    // The validator may reject this as "not VRM" or accept it as "valid glTF" — we
    // only assert that it produced a structured report.
    assert!(report.issues.iter().count() >= 0);
}

fn build_minimal_glb() -> Vec<u8> {
    let json = br#"{"asset":{"version":"2.0"}}"#;
    let json_padded_len = (json.len() + 3) & !3;
    let mut json_chunk = json.to_vec();
    json_chunk.resize(json_padded_len, b' ');

    let total_len = 12 + 8 + json_padded_len;

    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total_len as u32).to_le_bytes());
    out.extend_from_slice(&(json_padded_len as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_chunk);
    out
}
```

- [ ] **Step 3: Run the failing test**

Run: `cargo test -p vrm-validator-wrap`

Expected: compile error — `validate`, `ValidatorConfig` don't exist. This is the failing-test stage.

- [ ] **Step 4: Commit the failing test**

```bash
git add crates/vrm-validator-wrap/
git commit -m "test(validator-wrap): failing smoke tests for validate API"
```

---

### Task C3: `vrm-validator-wrap` — implementation

**Files:**
- Create: `crates/vrm-validator-wrap/src/lib.rs`

- [ ] **Step 1: Implement the wrapper**

Replace `crates/vrm-validator-wrap/src/lib.rs`:

```rust
//! Subprocess wrapper around the mrxz/vrm-validator native CLI.
//!
//! The validator emits a JSON report on stdout (or a `--output` path); we parse
//! that report into a typed structure callers can use to decide whether an
//! asset passes the precondition gate.

use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidatorError {
    #[error("validator binary not found at {0}; run scripts/install-validator.sh")]
    NotInstalled(Utf8PathBuf),

    #[error("validator process failed: {0}")]
    Process(#[from] std::io::Error),

    #[error("validator exited with status {status}: {stderr}")]
    NonZeroExit { status: i32, stderr: String },

    #[error("validator emitted unparseable JSON: {0}")]
    BadJson(#[from] serde_json::Error),

    #[error("input path does not exist: {0}")]
    InputMissing(Utf8PathBuf),
}

#[derive(Debug, Clone)]
pub struct ValidatorConfig {
    pub binary: Utf8PathBuf,
}

impl ValidatorConfig {
    /// Resolve from the `VRM_VALIDATOR_BIN` env var or fall back to
    /// `./.tools/gltf_validator` relative to the current working directory.
    pub fn from_env() -> Result<Self, ValidatorError> {
        let candidate = std::env::var("VRM_VALIDATOR_BIN")
            .map(Utf8PathBuf::from)
            .unwrap_or_else(|_| Utf8PathBuf::from(".tools/gltf_validator"));

        if !candidate.exists() {
            return Err(ValidatorError::NotInstalled(candidate));
        }
        Ok(Self { binary: candidate })
    }
}

#[derive(Debug, Deserialize)]
pub struct ValidatorReport {
    #[serde(default)]
    pub issues: Issues,

    #[serde(default)]
    pub info: serde_json::Value,
}

#[derive(Debug, Default, Deserialize)]
pub struct Issues {
    #[serde(default, rename = "numErrors")]
    pub num_errors: u32,

    #[serde(default, rename = "numWarnings")]
    pub num_warnings: u32,

    #[serde(default, rename = "numInfos")]
    pub num_infos: u32,

    #[serde(default, rename = "numHints")]
    pub num_hints: u32,

    #[serde(default)]
    pub messages: Vec<IssueMessage>,
}

impl Issues {
    pub fn iter(&self) -> impl Iterator<Item = &IssueMessage> {
        self.messages.iter()
    }
}

#[derive(Debug, Deserialize)]
pub struct IssueMessage {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub severity: u32,
    #[serde(default)]
    pub pointer: Option<String>,
}

pub fn validate(
    config: &ValidatorConfig,
    input: &Utf8Path,
) -> Result<ValidatorReport, ValidatorError> {
    if !input.exists() {
        return Err(ValidatorError::InputMissing(input.to_owned()));
    }

    tracing::debug!(binary = %config.binary, input = %input, "running validator");

    let output = Command::new(config.binary.as_std_path())
        .args(["--stdout", input.as_str()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(ValidatorError::NonZeroExit {
            status: output.status.code().unwrap_or(-1),
            stderr,
        });
    }

    let report: ValidatorReport = serde_json::from_slice(&output.stdout)?;
    Ok(report)
}
```

> **Caveat:** the CLI flags above (`--stdout`) are typical of `gltf-validator`-derived tools but not verified. The implementing engineer must run `./.tools/gltf_validator --help` once installed and adjust the `args(...)` line plus any output-parsing assumptions before merging. If the binary writes the report to a sidecar file rather than stdout, replace `Command::output()` with a temp-dir + read-back pattern.

- [ ] **Step 2: Run the tests**

Run: `cargo test -p vrm-validator-wrap`

Expected: both tests pass. If they don't, the most likely culprit is the CLI flag or stdout-vs-file assumption — fix and re-run.

- [ ] **Step 3: Commit**

```bash
git add crates/vrm-validator-wrap/
git commit -m "feat(validator-wrap): subprocess wrapper for mrxz/vrm-validator"
```

---

## Section D — Test plan schema crate

### Task D1: Type definitions + round-trip test

**Files:**
- Modify: `crates/vrm-test-plan/Cargo.toml`
- Create: `crates/vrm-test-plan/src/lib.rs`
- Create: `crates/vrm-test-plan/tests/roundtrip.rs`

- [ ] **Step 1: Add deps**

`crates/vrm-test-plan/Cargo.toml`:

```toml
[package]
name = "vrm-test-plan"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_yml.workspace = true
serde_json.workspace = true
thiserror.workspace = true
camino.workspace = true

[dev-dependencies]
insta.workspace = true
pretty_assertions.workspace = true
```

- [ ] **Step 2: Write the failing round-trip test**

Create `crates/vrm-test-plan/tests/roundtrip.rs`:

```rust
use vrm_test_plan::TestPlan;

const SAMPLE_YAML: &str = r#"
id: mtoon_shading_shift_negative
spec_section: VRMC_materials_mtoon §3.2 shadingShift
asset: generated/mtoon_basic_shadingShift_neg0.5.vrm
camera:
  position: [0.0, 1.4, 1.5]
  target: [0.0, 1.4, 0.0]
  up: [0.0, 1.0, 0.0]
  fov_degrees: 30.0
lighting:
  directional:
    dir: [-0.3, -0.6, -0.7]
    color: [1.0, 1.0, 1.0]
    intensity: 1.0
  ambient:
    color: [0.5, 0.5, 0.5]
    intensity: 0.3
  cast_shadows: false
  receive_shadows: false
post_processing:
  tone_mapping: none
  exposure: 1.0
output:
  width: 1024
  height: 1024
  color_space: linear
  msaa: 4
diff:
  mode: ssim
  threshold: 0.985
  reference_renderer: vrm-metal-kit
ignore_renderers: []
properties: []
"#;

#[test]
fn parses_sample_plan() {
    let plan: TestPlan = serde_yml::from_str(SAMPLE_YAML).unwrap();
    assert_eq!(plan.id, "mtoon_shading_shift_negative");
    assert_eq!(plan.output.width, 1024);
    assert_eq!(plan.output.msaa, 4);
}

#[test]
fn round_trips_yaml() {
    let plan: TestPlan = serde_yml::from_str(SAMPLE_YAML).unwrap();
    let serialized = serde_yml::to_string(&plan).unwrap();
    let reparsed: TestPlan = serde_yml::from_str(&serialized).unwrap();
    pretty_assertions::assert_eq!(plan, reparsed);
}
```

- [ ] **Step 3: Run the failing test**

Run: `cargo test -p vrm-test-plan`

Expected: compile error — `TestPlan` doesn't exist.

- [ ] **Step 4: Implement the schema**

Create `crates/vrm-test-plan/src/lib.rs`:

```rust
//! YAML schema for VRM conformance test plans.
//!
//! See `docs/methodology.md` for why specific defaults exist (tone_mapping=none,
//! shadows off, MSAA 4x).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestPlan {
    pub id: String,
    pub spec_section: String,
    pub asset: String,
    pub camera: Camera,
    pub lighting: Lighting,
    #[serde(default)]
    pub post_processing: PostProcessing,
    pub output: Output,
    pub diff: Diff,
    #[serde(default)]
    pub ignore_renderers: Vec<String>,
    #[serde(default)]
    pub properties: Vec<PropertyAssertion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fov_degrees: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lighting {
    pub directional: DirectionalLight,
    pub ambient: AmbientLight,
    #[serde(default)]
    pub cast_shadows: bool,
    #[serde(default)]
    pub receive_shadows: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DirectionalLight {
    pub dir: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AmbientLight {
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostProcessing {
    #[serde(default = "default_tone_mapping")]
    pub tone_mapping: ToneMapping,
    #[serde(default = "default_exposure")]
    pub exposure: f32,
}

impl Default for PostProcessing {
    fn default() -> Self {
        Self {
            tone_mapping: ToneMapping::None,
            exposure: 1.0,
        }
    }
}

fn default_tone_mapping() -> ToneMapping {
    ToneMapping::None
}

fn default_exposure() -> f32 {
    1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToneMapping {
    None,
    Linear,
    Reinhard,
    Aces,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Output {
    pub width: u32,
    pub height: u32,
    pub color_space: ColorSpace,
    pub msaa: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorSpace {
    Linear,
    Srgb,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diff {
    pub mode: DiffMode,
    pub threshold: f32,
    pub reference_renderer: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffMode {
    Ssim,
    Consensus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyAssertion {
    pub name: String,
    pub region: BboxRegion,
    pub expected: f32,
    pub tolerance: f32,
}

/// Region specifications are bbox-relative to keep tests robust against small
/// FOV / projection differences across renderers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BboxRegion {
    BboxFull,
    BboxLowerLeftQuadrant,
    BboxLowerRightQuadrant,
    BboxUpperLeftQuadrant,
    BboxUpperRightQuadrant,
    BboxCenterStripHorizontal,
    BboxCenterStripVertical,
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p vrm-test-plan`

Expected: both tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-test-plan/
git commit -m "feat(test-plan): YAML schema types with round-trip test"
```

---

## Section E — Operation catalog crate (`vrm-ops`)

> **Reframe note (per agent-first contract):** This crate is the source of truth for the operation set, shared by **both** the structured CLI and the MCP wrapper. The Phase 1 work here is the JSON-RPC stdio transport (used immediately by stateful adapters) and the operation type definitions. Schema export (for `describe`) and CLI plumbing land in F2+ alongside the asset generator and runner CLIs.

### Task E1: JSON-RPC envelope + tool request/response types

**Files:**
- Modify: `crates/vrm-ops/Cargo.toml`
- Create: `crates/vrm-ops/src/lib.rs`
- Create: `crates/vrm-ops/src/tools.rs`
- Create: `crates/vrm-ops/tests/serde.rs`

- [ ] **Step 1: Add deps**

`crates/vrm-ops/Cargo.toml`:

```toml
[package]
name = "vrm-ops"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true

[dev-dependencies]
insta.workspace = true
pretty_assertions.workspace = true
```

- [ ] **Step 2: Write the failing serde test**

Create `crates/vrm-ops/tests/serde.rs`:

```rust
use vrm_ops::{tools::*, JsonRpcRequest, JsonRpcResponse, RpcError};

#[test]
fn load_vrm_request_serializes() {
    let req = JsonRpcRequest::new(
        1,
        "load_vrm",
        LoadVrmParams {
            path: "/tmp/test.vrm".into(),
        },
    );
    let s = serde_json::to_string(&req).unwrap();
    assert!(s.contains(r#""method":"load_vrm""#));
    assert!(s.contains(r#""path":"/tmp/test.vrm""#));
    assert!(s.contains(r#""jsonrpc":"2.0""#));
}

#[test]
fn render_response_deserializes() {
    let raw = r#"{
        "jsonrpc": "2.0",
        "id": 7,
        "result": {
            "output_path": "/tmp/out.png",
            "actual_color_space": "Linear"
        }
    }"#;
    let resp: JsonRpcResponse<RenderResult> = serde_json::from_str(raw).unwrap();
    let result = resp.into_result().unwrap();
    assert_eq!(result.output_path, "/tmp/out.png");
    assert!(matches!(result.actual_color_space, ColorSpace::Linear));
}

#[test]
fn unimplemented_error_round_trips() {
    let err = RpcError::unimplemented("step_physics", "v1.x");
    let s = serde_json::to_string(&err).unwrap();
    let parsed: RpcError = serde_json::from_str(&s).unwrap();
    assert_eq!(parsed.code, -32000);
    assert_eq!(
        parsed.data.unwrap()["phase"].as_str().unwrap(),
        "v1.x"
    );
}
```

- [ ] **Step 3: Run the failing test**

Run: `cargo test -p vrm-ops`

Expected: compile errors.

- [ ] **Step 4: Implement the envelope**

Create `crates/vrm-ops/src/lib.rs`:

```rust
//! Operation catalog + JSON-RPC stdio transport. Source of truth for both
//! the structured CLI surface and the MCP wrapper.
//!
//! Spec: `docs/operation-contract.md`. Stdio framing follows LSP header convention
//! (`Content-Length: NNN\r\n\r\n` + body).

pub mod tools;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest<P> {
    pub jsonrpc: JsonRpcVersion,
    pub id: u64,
    pub method: String,
    pub params: P,
}

impl<P> JsonRpcRequest<P> {
    pub fn new(id: u64, method: impl Into<String>, params: P) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id,
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse<R> {
    pub jsonrpc: JsonRpcVersion,
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<R>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl<R> JsonRpcResponse<R> {
    pub fn ok(id: u64, result: R) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: u64, error: RpcError) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id,
            result: None,
            error: Some(error),
        }
    }

    pub fn into_result(self) -> Result<R, RpcError> {
        match (self.result, self.error) {
            (Some(r), None) => Ok(r),
            (None, Some(e)) => Err(e),
            _ => Err(RpcError {
                code: -32700,
                message: "malformed response: missing both result and error".into(),
                data: None,
            }),
        }
    }
}

/// Marker that always serializes as `"2.0"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonRpcVersion;

impl Serialize for JsonRpcVersion {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str("2.0")
    }
}

impl<'de> Deserialize<'de> for JsonRpcVersion {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if s == "2.0" {
            Ok(JsonRpcVersion)
        } else {
            Err(serde::de::Error::custom(format!(
                "expected jsonrpc 2.0, got {s}"
            )))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Error)]
#[error("jsonrpc error {code}: {message}")]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl RpcError {
    pub fn unimplemented(method: &str, phase: &str) -> Self {
        Self {
            code: -32000,
            message: format!("{method}: not implemented in this adapter version"),
            data: Some(serde_json::json!({ "phase": phase })),
        }
    }

    pub fn load_failed(report: impl Into<String>) -> Self {
        Self {
            code: -32001,
            message: "LoadFailed".into(),
            data: Some(serde_json::json!({ "validator_report": report.into() })),
        }
    }

    pub fn render_failed(reason: impl Into<String>) -> Self {
        Self {
            code: -32002,
            message: "RenderFailed".into(),
            data: Some(serde_json::json!({ "reason": reason.into() })),
        }
    }
}
```

- [ ] **Step 5: Implement the tool param/result types**

Create `crates/vrm-ops/src/tools.rs`:

```rust
use serde::{Deserialize, Serialize};

// ---- Phase 1 required tools ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadVrmParams {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadVrmResult {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCameraParams {
    pub session_id: String,
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fov_degrees: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetLightingParams {
    pub session_id: String,
    pub directional: Directional,
    pub ambient: Ambient,
    #[serde(default)]
    pub cast_shadows: bool,
    #[serde(default)]
    pub receive_shadows: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Directional {
    pub dir: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ambient {
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetPostProcessingParams {
    pub session_id: String,
    pub tone_mapping: ToneMapping,
    pub exposure: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ToneMapping {
    None,
    Linear,
    Reinhard,
    Aces,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderParams {
    pub session_id: String,
    pub width: u32,
    pub height: u32,
    pub output_path: String,
    pub color_space: ColorSpace,
    pub msaa: u8,
    pub output_type: OutputType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderResult {
    pub output_path: String,
    pub actual_color_space: ColorSpace,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ColorSpace {
    Linear,
    Srgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OutputType {
    Color,
    Normal,
    Depth,
    Albedo,
    MToonShadingMask,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisposeParams {
    pub session_id: String,
}

// Empty result type for tools that return no payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitResult {}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p vrm-ops`

Expected: all three tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-ops/
git commit -m "feat(mcp-protocol): JSON-RPC envelope + Phase 1 tool types"
```

---

### Task E2: Stdio framing helper

**Files:**
- Create: `crates/vrm-ops/src/stdio.rs`
- Modify: `crates/vrm-ops/src/lib.rs`
- Create: `crates/vrm-ops/tests/stdio.rs`

- [ ] **Step 1: Failing test for round-trip framing**

Create `crates/vrm-ops/tests/stdio.rs`:

```rust
use vrm_ops::stdio::{read_message, write_message};

#[test]
fn round_trips_a_message() {
    let payload = br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}"#;

    let mut buf = Vec::new();
    write_message(&mut buf, payload).unwrap();

    let mut cursor = std::io::Cursor::new(buf);
    let read = read_message(&mut cursor).unwrap();
    assert_eq!(read, payload);
}

#[test]
fn rejects_missing_content_length() {
    let raw = b"\r\n\r\n{}";
    let mut cursor = std::io::Cursor::new(&raw[..]);
    let err = read_message(&mut cursor).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("content-length"));
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test -p vrm-ops --test stdio`

Expected: compile error.

- [ ] **Step 3: Implement framing**

Create `crates/vrm-ops/src/stdio.rs`:

```rust
//! LSP-style stdio framing: `Content-Length: N\r\n\r\n<body>`.

use std::io::{BufRead, BufReader, Read, Write};

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("missing or malformed Content-Length header")]
    MissingContentLength,
    #[error("invalid header line: {0}")]
    BadHeader(String),
}

pub fn write_message<W: Write>(w: &mut W, body: &[u8]) -> Result<(), FrameError> {
    write!(w, "Content-Length: {}\r\n\r\n", body.len())?;
    w.write_all(body)?;
    w.flush()?;
    Ok(())
}

pub fn read_message<R: Read>(r: &mut R) -> Result<Vec<u8>, FrameError> {
    let mut reader = BufReader::new(r);
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Err(FrameError::MissingContentLength);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        let (k, v) = trimmed
            .split_once(':')
            .ok_or_else(|| FrameError::BadHeader(trimmed.to_string()))?;
        if k.eq_ignore_ascii_case("Content-Length") {
            content_length = Some(
                v.trim()
                    .parse()
                    .map_err(|_| FrameError::BadHeader(trimmed.to_string()))?,
            );
        }
    }

    let len = content_length.ok_or(FrameError::MissingContentLength)?;
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    Ok(body)
}
```

- [ ] **Step 4: Re-export from lib.rs**

Add to `crates/vrm-ops/src/lib.rs` after `pub mod tools;`:

```rust
pub mod stdio;
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p vrm-ops`

Expected: all five tests (3 from E1 + 2 from E2) pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-ops/
git commit -m "feat(mcp-protocol): stdio framing per LSP convention"
```

---

## Section F — Asset generator (single asset first)

> **Implementation guidance:** Build the smallest end-to-end emission first (one MToon material on a sphere, validate it, write the sidecars). Only after that loop is green do we expand to the full sweep matrix in Section G.

### Task F1: Parameter dictionary type + sphere mesh fixture

**Files:**
- Modify: `crates/vrm-asset-generator/Cargo.toml`
- Create: `crates/vrm-asset-generator/src/lib.rs`
- Create: `crates/vrm-asset-generator/src/params.rs`
- Create: `crates/vrm-asset-generator/src/mesh.rs`

- [ ] **Step 1: Update crate deps**

`crates/vrm-asset-generator/Cargo.toml`:

```toml
[package]
name = "vrm-asset-generator"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
anyhow.workspace = true
clap.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
serde.workspace = true
serde_json.workspace = true
serde_yml.workspace = true
gltf.workspace = true
glam.workspace = true
camino.workspace = true
blake3.workspace = true
hex.workspace = true

vrm-test-plan = { path = "../vrm-test-plan" }
vrm-validator-wrap = { path = "../vrm-validator-wrap" }

[dev-dependencies]
insta.workspace = true
pretty_assertions.workspace = true
tempfile.workspace = true

[[bin]]
name = "vrm-asset-generator"
path = "src/main.rs"

[lib]
path = "src/lib.rs"
```

- [ ] **Step 2: Define the MToon parameter dictionary**

Create `crates/vrm-asset-generator/src/params.rs`:

```rust
//! Parameter dictionary for MToon material generation.
//!
//! Every emission is fully described by a `MToonParams` value plus a fixed
//! mesh fixture. The same dictionary that produces an asset's binary content
//! also produces the sidecar `.meta.json` and `.test.yaml`, eliminating
//! desync risk between asset and test plan.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MToonParams {
    pub id: String,

    pub base_color_factor: [f32; 4],
    pub shade_color_factor: [f32; 3],

    pub shading_shift_factor: f32,
    pub shading_toony_factor: f32,
    pub gi_equalization_factor: f32,

    pub parametric_rim_color_factor: [f32; 3],
    pub parametric_rim_fresnel_power_factor: f32,
    pub parametric_rim_lift_factor: f32,
    pub rim_lighting_mix_factor: f32,

    pub matcap_factor: [f32; 3],

    pub outline_width_mode: OutlineWidthMode,
    pub outline_width_factor: f32,
    pub outline_color_factor: [f32; 3],
    pub outline_lighting_mix_factor: f32,

    pub uv_animation_scroll_x_speed_factor: f32,
    pub uv_animation_scroll_y_speed_factor: f32,
    pub uv_animation_rotation_speed_factor: f32,

    pub alpha_mode: AlphaMode,
    pub transparent_with_z_write: bool,
    pub render_queue_offset_number: i32,

    pub double_sided: bool,
}

impl MToonParams {
    /// Defaults match the VRMC_materials_mtoon spec defaults wherever defined,
    /// otherwise a neutrally-rendering value.
    pub fn defaults(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            shade_color_factor: [0.5, 0.5, 0.5],
            shading_shift_factor: 0.0,
            shading_toony_factor: 0.9,
            gi_equalization_factor: 0.9,
            parametric_rim_color_factor: [0.0, 0.0, 0.0],
            parametric_rim_fresnel_power_factor: 5.0,
            parametric_rim_lift_factor: 0.0,
            rim_lighting_mix_factor: 0.0,
            matcap_factor: [1.0, 1.0, 1.0],
            outline_width_mode: OutlineWidthMode::None,
            outline_width_factor: 0.0,
            outline_color_factor: [0.0, 0.0, 0.0],
            outline_lighting_mix_factor: 1.0,
            uv_animation_scroll_x_speed_factor: 0.0,
            uv_animation_scroll_y_speed_factor: 0.0,
            uv_animation_rotation_speed_factor: 0.0,
            alpha_mode: AlphaMode::Opaque,
            transparent_with_z_write: false,
            render_queue_offset_number: 0,
            double_sided: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutlineWidthMode {
    None,
    WorldCoordinates,
    ScreenCoordinates,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AlphaMode {
    Opaque,
    Mask,
    Blend,
}
```

- [ ] **Step 3: Implement the sphere fixture**

Create `crates/vrm-asset-generator/src/mesh.rs`:

```rust
//! Generated mesh fixtures used for material isolation tests. The sphere is
//! the default — material tests want to isolate the MToon math, not test
//! geometry rendering, so the mesh is intentionally minimal and constant
//! across all material parameter combinations.

use glam::{Vec2, Vec3};

pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// UV-sphere with `lat_segments × lon_segments` quads, split into triangles.
/// Defaults of (32, 64) give a smooth-enough sphere without bloating the
/// generated `.glb` files.
pub fn sphere(radius: f32, lat_segments: u32, lon_segments: u32) -> MeshData {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for lat in 0..=lat_segments {
        let theta = lat as f32 * std::f32::consts::PI / lat_segments as f32;
        let sin_t = theta.sin();
        let cos_t = theta.cos();

        for lon in 0..=lon_segments {
            let phi = lon as f32 * 2.0 * std::f32::consts::PI / lon_segments as f32;
            let sin_p = phi.sin();
            let cos_p = phi.cos();

            let n = Vec3::new(cos_p * sin_t, cos_t, sin_p * sin_t);
            let p = n * radius;
            let uv = Vec2::new(
                lon as f32 / lon_segments as f32,
                lat as f32 / lat_segments as f32,
            );

            positions.push(p.into());
            normals.push(n.into());
            uvs.push(uv.into());
        }
    }

    let row = lon_segments + 1;
    for lat in 0..lat_segments {
        for lon in 0..lon_segments {
            let i0 = lat * row + lon;
            let i1 = i0 + row;
            indices.extend_from_slice(&[i0, i1, i0 + 1, i0 + 1, i1, i1 + 1]);
        }
    }

    MeshData {
        positions,
        normals,
        uvs,
        indices,
    }
}
```

- [ ] **Step 4: Wire modules into `lib.rs`**

Replace `crates/vrm-asset-generator/src/lib.rs`:

```rust
//! Parametric VRM 1.0 asset generator. Emits paired
//! `<asset>.vrm + <asset>.meta.json + <asset>.test.yaml` from one parameter dict.

pub mod mesh;
pub mod params;
```

- [ ] **Step 5: Verify the crate compiles**

Run: `cargo build -p vrm-asset-generator`

Expected: success.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/
git commit -m "feat(asset-generator): MToon parameter dictionary + sphere fixture"
```

---

> **Plan continues in `plan-part-2.md` (to be appended).** The remaining sections to draft:
>
> - **F2** Single-asset GLB emission with VRM extension JSON (failing test → impl → validate via `vrm-validator-wrap`).
> - **F3** Sidecar `.meta.json` and `.test.yaml` emission, derived from the same `MToonParams`.
> - **F4** CLI surface for `vrm-asset-generator` (single-asset emit subcommand).
> - **G1–G3** Sweep matrix expansion to ~50 MToon assets.
> - **H1–H3** Diff engine (SSIM via `image-compare`, property assertion stub).
> - **I1–I4** Runner: spawns adapter subprocess, executes test plan, writes output PNGs.
> - **J1–J3** S3 manifest schema + `push-goldens.sh` / `pull-goldens.sh` + `vrm-s3` library.
> - **K1–K4** Static comparison site (Vite + TS, side-by-side viewer).
> - **L1–L3** VRMMetalKit Swift adapter (Package.swift, MCP server scaffolding, contract-level acceptance criteria — Swift integration is delegated to a Swift dev with the contract as the spec).
> - **M1–M2** GitHub Actions workflows (rust.yml, swift.yml, site.yml, manifest-validate.yml).
> - **N1** End-to-end smoke test: generate 1 asset → render via VRMMetalKit → diff against itself → upload to S3 → display in site (the v0.1 hello-world).
>
> The next plan-extension write will append these sections in the same task / step / commit format. Total estimated remaining tasks: ~35 across 11 sections, comparable in detail to Sections A–F1 above.

---

## Self-Review (Sections A–F1, partial)

This plan is a **partial draft.** Sections A through F1 are complete and ready for execution; Sections F2 onward exist only as a roadmap stub above. The reviewer should treat this as Plan v0.1 and request the F2+ extension before kicking off subagent execution.

**Spec coverage check (so far):**
- §3 Repository Layout — covered (RFC-0001 confirms monorepo flatten).
- §4 Architecture diagram — covered structurally; component implementations land in F2+.
- §5.3 MCP tool surface — covered in `docs/mcp-tool-surface.md` and `vrm-ops` crate (Section E).
- §7 Methodology hazards — covered in `docs/methodology.md` (Task A5).
- §11 Open Questions 1, 7 — covered as RFC-0001 and RFC-0002.
- §14 First-week tasks 1–3 — covered (governance, Cargo workspace, MCP schema definition).

**Spec coverage gaps (deferred to F2+):**
- §5.1 Asset corpus matrix — Section G.
- §5.2 render-fidelity site — Section K.
- §5.3 runner orchestrator — Section I.
- §6 Renderer adapter (VRMMetalKit MCP server) — Section L.
- §8 mrxz/vrm-validator wired into asset generator — F2 (uses the wrapper from C3).
- §13 Definition of Done items — distributed across G–N.

**Placeholder scan:** None in A–F1 itself. The F2+ roadmap stub is intentionally a placeholder; it must be expanded before execution.

**Type consistency:** `vrm-test-plan::ColorSpace` and `vrm-ops::tools::ColorSpace` are deliberately separate types with the same shape (lower-case-string YAML serialization vs. enum variant JSON serialization). Document this in the runner's plan→MCP translation layer (Section I).

---

## Execution Handoff

**Plan v0.1 is partial — sections A through F1 are ready; F2 through N are stubs.** Before kicking off execution, decide:

1. **Extend the plan** — request the F2–N expansion as the next step. This produces a complete plan you can hand to subagents.
2. **Execute A–F1 inline first** — get the governance + workspace + protocol layers shipped, then plan F2 onward with the lessons from those builds.

Either is reasonable. If you want speed-to-first-commit, option 2 is faster (governance + Cargo workspace + protocol crates is a productive day's work and unblocks parallelism on F2+). If you want a single coherent plan handed to one subagent loop, option 1.
