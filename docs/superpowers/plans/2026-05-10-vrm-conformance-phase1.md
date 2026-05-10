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

<!-- Plan v0.2 begins here: F2 through N -->

### Task F2a: GLB container writer (no extensions yet, TDD)

**Files:**
- Create: `crates/vrm-asset-generator/src/glb.rs`
- Modify: `crates/vrm-asset-generator/src/lib.rs`
- Create: `crates/vrm-asset-generator/tests/glb_smoke.rs`

The `gltf` crate's write support is limited; we hand-roll the GLB binary container. Spec: [glTF 2.0 GLB §4.4](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#binary-gltf-layout). Header (12 bytes) + JSON chunk + BIN chunk, each padded to 4 bytes.

- [ ] **Step 1: Failing test**

`crates/vrm-asset-generator/tests/glb_smoke.rs`:

```rust
use vrm_asset_generator::glb::{write_glb, GlbDocument};

#[test]
fn writes_glb_with_valid_magic_and_chunks() {
    let doc = GlbDocument {
        json: br#"{"asset":{"version":"2.0"}}"#.to_vec(),
        binary: vec![0u8; 16],
    };
    let bytes = write_glb(&doc).unwrap();

    assert_eq!(&bytes[0..4], b"glTF");
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert_eq!(version, 2);
    let total = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    assert_eq!(total, bytes.len());

    // First chunk = JSON
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    assert_eq!(&bytes[16..20], b"JSON");
    assert!(json_len % 4 == 0, "json chunk must be 4-byte aligned");

    // Second chunk = BIN
    let bin_offset = 20 + json_len;
    let bin_len = u32::from_le_bytes(
        bytes[bin_offset..bin_offset + 4].try_into().unwrap(),
    ) as usize;
    assert_eq!(&bytes[bin_offset + 4..bin_offset + 8], b"BIN\0");
    assert!(bin_len % 4 == 0, "bin chunk must be 4-byte aligned");
}

#[test]
fn empty_binary_omits_bin_chunk() {
    let doc = GlbDocument {
        json: br#"{"asset":{"version":"2.0"}}"#.to_vec(),
        binary: Vec::new(),
    };
    let bytes = write_glb(&doc).unwrap();

    // Only JSON chunk; BIN chunk is optional per spec.
    let total = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    assert_eq!(total, 12 + 8 + json_len, "no BIN chunk should be present");
}
```

- [ ] **Step 2: Run failing test**

`cargo test -p vrm-asset-generator --test glb_smoke` → compile error (`glb` module missing).

- [ ] **Step 3: Implement GLB writer**

`crates/vrm-asset-generator/src/glb.rs`:

```rust
//! GLB binary container writer (glTF 2.0 binary format).
//!
//! Layout:
//!   Header:   "glTF" + version=2 (u32 LE) + total_length (u32 LE)  = 12 bytes
//!   Chunk 0:  length (u32 LE) + "JSON" + padded JSON bytes (' ' pad)
//!   Chunk 1:  length (u32 LE) + "BIN\0" + padded binary bytes (0 pad)  [optional]

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct GlbDocument {
    pub json: Vec<u8>,
    pub binary: Vec<u8>,
}

const PAD_TO: usize = 4;

fn pad_to(input: &[u8], pad_byte: u8) -> Vec<u8> {
    let pad = (PAD_TO - input.len() % PAD_TO) % PAD_TO;
    let mut out = Vec::with_capacity(input.len() + pad);
    out.extend_from_slice(input);
    out.resize(input.len() + pad, pad_byte);
    out
}

pub fn write_glb(doc: &GlbDocument) -> Result<Vec<u8>> {
    let json_padded = pad_to(&doc.json, b' ');
    let json_chunk_len = json_padded.len();

    let (bin_padded, bin_chunk_len) = if doc.binary.is_empty() {
        (Vec::new(), 0usize)
    } else {
        let p = pad_to(&doc.binary, 0);
        let l = p.len();
        (p, l)
    };

    let total = 12
        + 8
        + json_chunk_len
        + if bin_chunk_len > 0 { 8 + bin_chunk_len } else { 0 };

    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());

    out.extend_from_slice(&(json_chunk_len as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_padded);

    if bin_chunk_len > 0 {
        out.extend_from_slice(&(bin_chunk_len as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin_padded);
    }

    Ok(out)
}
```

- [ ] **Step 4: Wire into lib.rs**

Add to `crates/vrm-asset-generator/src/lib.rs`:

```rust
pub mod glb;
```

- [ ] **Step 5: Tests pass**

`cargo test -p vrm-asset-generator --test glb_smoke` → both tests green.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/
git commit -m "feat(asset-generator): GLB binary container writer with TDD"
```

---

### Task F2b: Buffer/accessor builder for sphere mesh

**Files:**
- Create: `crates/vrm-asset-generator/src/buffer.rs`
- Modify: `crates/vrm-asset-generator/src/lib.rs`
- Create: `crates/vrm-asset-generator/tests/buffer.rs`

Builds a single GLB binary buffer containing positions, normals, UVs, and indices for a `MeshData`, plus the matching glTF `accessors` and `bufferViews` JSON. Output is a tuple of (binary blob, JSON fragment as `serde_json::Value`).

- [ ] **Step 1: Failing test**

`crates/vrm-asset-generator/tests/buffer.rs`:

```rust
use vrm_asset_generator::{buffer::pack_mesh, mesh::sphere};

#[test]
fn pack_sphere_emits_expected_accessors() {
    let m = sphere(1.0, 8, 16);
    let packed = pack_mesh(&m);

    // 4 accessors: positions, normals, uvs, indices.
    let acc = packed.json["accessors"].as_array().unwrap();
    assert_eq!(acc.len(), 4);

    // Positions accessor: VEC3, FLOAT, count = vertex count
    assert_eq!(acc[0]["type"], "VEC3");
    assert_eq!(acc[0]["componentType"], 5126); // GL_FLOAT
    assert_eq!(acc[0]["count"].as_u64().unwrap() as usize, m.positions.len());

    // Indices accessor: SCALAR, count = m.indices.len()
    assert_eq!(acc[3]["type"], "SCALAR");
    assert_eq!(acc[3]["count"].as_u64().unwrap() as usize, m.indices.len());

    // 4 bufferViews
    let bv = packed.json["bufferViews"].as_array().unwrap();
    assert_eq!(bv.len(), 4);

    // Single buffer with byteLength matching binary blob length
    let buf = &packed.json["buffers"][0];
    assert_eq!(
        buf["byteLength"].as_u64().unwrap() as usize,
        packed.binary.len()
    );

    // Binary length should be 4-aligned (we'll let GLB writer pad if not, but
    // the per-bufferView offsets must align to component size).
    assert!(
        packed.binary.len() >= 12 * m.positions.len() + 12 * m.normals.len()
            + 8 * m.uvs.len() + 4 * m.indices.len(),
        "binary should hold all 4 streams"
    );
}
```

- [ ] **Step 2: Run failing test**

`cargo test -p vrm-asset-generator --test buffer` → compile error.

- [ ] **Step 3: Implement**

`crates/vrm-asset-generator/src/buffer.rs`:

```rust
//! glTF 2.0 buffer/bufferView/accessor builder for our generated meshes.
//!
//! Produces a single packed binary blob plus the JSON fragments for one
//! buffer, four bufferViews, and four accessors covering: positions
//! (VEC3 FLOAT), normals (VEC3 FLOAT), uvs (VEC2 FLOAT), indices
//! (SCALAR UNSIGNED_INT).

use crate::mesh::MeshData;
use serde_json::{json, Value};

const GL_UNSIGNED_INT: u32 = 5125;
const GL_FLOAT: u32 = 5126;
const TARGET_ARRAY_BUFFER: u32 = 34962;
const TARGET_ELEMENT_ARRAY_BUFFER: u32 = 34963;

#[derive(Debug, Clone)]
pub struct PackedMesh {
    pub binary: Vec<u8>,
    pub json: Value,
}

fn align_to(v: &mut Vec<u8>, alignment: usize) {
    let pad = (alignment - v.len() % alignment) % alignment;
    v.resize(v.len() + pad, 0);
}

fn write_vec3_array(out: &mut Vec<u8>, data: &[[f32; 3]]) -> (usize, usize) {
    let offset = out.len();
    for v in data {
        for c in v {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
    let len = out.len() - offset;
    (offset, len)
}

fn write_vec2_array(out: &mut Vec<u8>, data: &[[f32; 2]]) -> (usize, usize) {
    let offset = out.len();
    for v in data {
        for c in v {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
    let len = out.len() - offset;
    (offset, len)
}

fn write_u32_array(out: &mut Vec<u8>, data: &[u32]) -> (usize, usize) {
    let offset = out.len();
    for x in data {
        out.extend_from_slice(&x.to_le_bytes());
    }
    let len = out.len() - offset;
    (offset, len)
}

fn min_max_vec3(data: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for v in data {
        for i in 0..3 {
            if v[i] < min[i] {
                min[i] = v[i];
            }
            if v[i] > max[i] {
                max[i] = v[i];
            }
        }
    }
    (min, max)
}

pub fn pack_mesh(mesh: &MeshData) -> PackedMesh {
    let mut bin: Vec<u8> = Vec::new();

    // 1) positions
    let (pos_off, pos_len) = write_vec3_array(&mut bin, &mesh.positions);
    align_to(&mut bin, 4);
    // 2) normals
    let (nrm_off, nrm_len) = write_vec3_array(&mut bin, &mesh.normals);
    align_to(&mut bin, 4);
    // 3) uvs
    let (uv_off, uv_len) = write_vec2_array(&mut bin, &mesh.uvs);
    align_to(&mut bin, 4);
    // 4) indices
    let (idx_off, idx_len) = write_u32_array(&mut bin, &mesh.indices);

    let (pos_min, pos_max) = min_max_vec3(&mesh.positions);

    let json = json!({
        "buffers": [
            { "byteLength": bin.len() }
        ],
        "bufferViews": [
            { "buffer": 0, "byteOffset": pos_off, "byteLength": pos_len, "target": TARGET_ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": nrm_off, "byteLength": nrm_len, "target": TARGET_ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": uv_off,  "byteLength": uv_len,  "target": TARGET_ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": idx_off, "byteLength": idx_len, "target": TARGET_ELEMENT_ARRAY_BUFFER }
        ],
        "accessors": [
            {
                "bufferView": 0, "componentType": GL_FLOAT,
                "count": mesh.positions.len(), "type": "VEC3",
                "min": pos_min, "max": pos_max
            },
            {
                "bufferView": 1, "componentType": GL_FLOAT,
                "count": mesh.normals.len(), "type": "VEC3"
            },
            {
                "bufferView": 2, "componentType": GL_FLOAT,
                "count": mesh.uvs.len(), "type": "VEC2"
            },
            {
                "bufferView": 3, "componentType": GL_UNSIGNED_INT,
                "count": mesh.indices.len(), "type": "SCALAR"
            }
        ]
    });

    PackedMesh { binary: bin, json }
}
```

- [ ] **Step 4: Wire**

Add to `lib.rs`: `pub mod buffer;`

- [ ] **Step 5: Tests pass**

`cargo test -p vrm-asset-generator --test buffer` → green.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/
git commit -m "feat(asset-generator): mesh-to-glTF-buffer packer with accessor JSON"
```

---

### Task F2c: VRM 1.0 humanoid skeleton stub

**Files:**
- Create: `crates/vrm-asset-generator/src/humanoid.rs`
- Modify: `crates/vrm-asset-generator/src/lib.rs`
- Create: `crates/vrm-asset-generator/tests/humanoid.rs`

VRMC_vrm requires a humanoid block with all required bones mapped to glTF nodes. For Phase 1 (sphere material tests), we don't need a real skeleton — just enough nodes to satisfy the validator. This module emits a fixed minimal A-pose skeleton (positions are spec-recommended placeholders), returning the node array fragment + a `bone_name → node_index` map.

The required-bone list per VRM 1.0 spec: hips, spine, chest, neck, head, leftShoulder, leftUpperArm, leftLowerArm, leftHand, rightShoulder, rightUpperArm, rightLowerArm, rightHand, leftUpperLeg, leftLowerLeg, leftFoot, rightUpperLeg, rightLowerLeg, rightFoot. (Some are optional but we emit them all to be safe.)

- [ ] **Step 1: Failing test**

`crates/vrm-asset-generator/tests/humanoid.rs`:

```rust
use vrm_asset_generator::humanoid::minimal_skeleton;

#[test]
fn skeleton_includes_all_required_bones() {
    let s = minimal_skeleton();
    let required = [
        "hips", "spine", "chest", "neck", "head",
        "leftShoulder", "leftUpperArm", "leftLowerArm", "leftHand",
        "rightShoulder", "rightUpperArm", "rightLowerArm", "rightHand",
        "leftUpperLeg", "leftLowerLeg", "leftFoot",
        "rightUpperLeg", "rightLowerLeg", "rightFoot",
    ];
    for b in required {
        assert!(
            s.bone_to_node.contains_key(b),
            "missing required bone: {b}"
        );
    }
}

#[test]
fn nodes_are_indexed_consistently() {
    let s = minimal_skeleton();
    let nodes = s.nodes_json.as_array().unwrap();
    for (bone, idx) in &s.bone_to_node {
        let node = &nodes[*idx];
        let name = node["name"].as_str().unwrap();
        assert_eq!(name, bone, "bone {bone} maps to node named {name}");
    }
}
```

- [ ] **Step 2: Run failing test**

`cargo test -p vrm-asset-generator --test humanoid` → compile error.

- [ ] **Step 3: Implement**

`crates/vrm-asset-generator/src/humanoid.rs`:

```rust
//! VRM 1.0 minimal humanoid skeleton stub.
//!
//! Emits a fixed A-pose skeleton with the spec-required bones as glTF nodes.
//! Phase 1 material tests don't pose the avatar; the skeleton exists only to
//! satisfy VRMC_vrm.humanoid.humanBones validation. Bone positions are
//! rough A-pose defaults.

use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Skeleton {
    /// glTF `nodes` array fragment.
    pub nodes_json: Value,
    /// Index of the root node (always `hips`'s parent or hips itself).
    pub root_node: usize,
    /// Map of VRM 1.0 bone name to glTF node index.
    pub bone_to_node: BTreeMap<String, usize>,
}

/// Bone definition: (name, parent_bone_or_None, translation_relative_to_parent).
struct B {
    name: &'static str,
    parent: Option<&'static str>,
    t: [f32; 3],
}

fn bones() -> &'static [B] {
    &[
        B { name: "hips",          parent: None,                t: [0.0,  0.86, 0.0] },
        B { name: "spine",         parent: Some("hips"),        t: [0.0,  0.10, 0.0] },
        B { name: "chest",         parent: Some("spine"),       t: [0.0,  0.10, 0.0] },
        B { name: "neck",          parent: Some("chest"),       t: [0.0,  0.20, 0.0] },
        B { name: "head",          parent: Some("neck"),        t: [0.0,  0.10, 0.0] },

        B { name: "leftShoulder",  parent: Some("chest"),       t: [ 0.05, 0.18, 0.0] },
        B { name: "leftUpperArm",  parent: Some("leftShoulder"), t: [ 0.10, 0.0,  0.0] },
        B { name: "leftLowerArm",  parent: Some("leftUpperArm"), t: [ 0.25, 0.0,  0.0] },
        B { name: "leftHand",      parent: Some("leftLowerArm"), t: [ 0.25, 0.0,  0.0] },

        B { name: "rightShoulder", parent: Some("chest"),        t: [-0.05, 0.18, 0.0] },
        B { name: "rightUpperArm", parent: Some("rightShoulder"),t: [-0.10, 0.0,  0.0] },
        B { name: "rightLowerArm", parent: Some("rightUpperArm"),t: [-0.25, 0.0,  0.0] },
        B { name: "rightHand",     parent: Some("rightLowerArm"),t: [-0.25, 0.0,  0.0] },

        B { name: "leftUpperLeg",  parent: Some("hips"),         t: [ 0.10, 0.0,  0.0] },
        B { name: "leftLowerLeg",  parent: Some("leftUpperLeg"), t: [ 0.0,  -0.40, 0.0] },
        B { name: "leftFoot",      parent: Some("leftLowerLeg"), t: [ 0.0,  -0.40, 0.0] },

        B { name: "rightUpperLeg", parent: Some("hips"),         t: [-0.10, 0.0,  0.0] },
        B { name: "rightLowerLeg", parent: Some("rightUpperLeg"),t: [ 0.0,  -0.40, 0.0] },
        B { name: "rightFoot",     parent: Some("rightLowerLeg"),t: [ 0.0,  -0.40, 0.0] },
    ]
}

pub fn minimal_skeleton() -> Skeleton {
    let bones = bones();
    let mut bone_to_node = BTreeMap::new();
    for (i, b) in bones.iter().enumerate() {
        bone_to_node.insert(b.name.to_string(), i);
    }

    // Build children arrays.
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); bones.len()];
    for (i, b) in bones.iter().enumerate() {
        if let Some(parent_name) = b.parent {
            let pidx = bone_to_node[parent_name];
            children[pidx].push(i);
        }
    }

    let nodes: Vec<Value> = bones
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let mut node = json!({
                "name": b.name,
                "translation": b.t,
            });
            if !children[i].is_empty() {
                node["children"] = json!(children[i]);
            }
            node
        })
        .collect();

    Skeleton {
        nodes_json: Value::Array(nodes),
        root_node: bone_to_node["hips"],
        bone_to_node,
    }
}
```

- [ ] **Step 4: Wire**

Add to `lib.rs`: `pub mod humanoid;`

- [ ] **Step 5: Tests pass**

`cargo test -p vrm-asset-generator --test humanoid` → green.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/
git commit -m "feat(asset-generator): minimal VRM 1.0 humanoid skeleton stub"
```

---

### Task F2d: VRM extension JSON + full asset emission (TDD, validated)

**Files:**
- Create: `crates/vrm-asset-generator/src/vrm_ext.rs`
- Create: `crates/vrm-asset-generator/src/emit.rs`
- Modify: `crates/vrm-asset-generator/src/lib.rs`
- Create: `crates/vrm-asset-generator/tests/emit.rs`

This is the integration moment: combine sphere mesh + buffer packer + skeleton + VRM extensions, write a `.vrm`, and run the validator wrapper to confirm 0 errors.

- [ ] **Step 1: Failing integration test**

`crates/vrm-asset-generator/tests/emit.rs`:

```rust
use camino::Utf8PathBuf;
use vrm_asset_generator::{emit::emit_vrm, params::MToonParams};
use vrm_validator_wrap::{validate, ValidatorConfig};

fn config_or_skip() -> Option<ValidatorConfig> {
    match ValidatorConfig::from_env() {
        Ok(c) => Some(c),
        Err(_) => {
            eprintln!("SKIP: validator not installed");
            None
        }
    }
}

#[test]
fn emits_validator_clean_vrm_with_default_mtoon() {
    let Some(cfg) = config_or_skip() else { return };

    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::from_path_buf(dir.path().join("default.vrm")).unwrap();

    let params = MToonParams::defaults("default");
    emit_vrm(&params, &out).expect("emission must succeed");

    let report = validate(&cfg, &out).expect("validator must produce a report");
    assert_eq!(
        report.issues.num_errors, 0,
        "emitted VRM should have zero validator errors. report: {:#?}",
        report.issues.messages
    );

    // mimeType should be GLB.
    assert_eq!(report.mime_type.as_deref(), Some("model/gltf-binary"));
}

#[test]
fn emits_validator_clean_vrm_with_outline() {
    let Some(cfg) = config_or_skip() else { return };

    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::from_path_buf(dir.path().join("outline.vrm")).unwrap();

    let mut params = MToonParams::defaults("outline_world_05cm");
    params.outline_width_mode = vrm_asset_generator::params::OutlineWidthMode::WorldCoordinates;
    params.outline_width_factor = 0.005;
    params.outline_color_factor = [0.0, 0.0, 0.0];

    emit_vrm(&params, &out).unwrap();
    let report = validate(&cfg, &out).unwrap();
    assert_eq!(report.issues.num_errors, 0);
}
```

- [ ] **Step 2: Run failing test**

`cargo test -p vrm-asset-generator --test emit` → compile errors.

- [ ] **Step 3: Implement VRM extension JSON builder**

`crates/vrm-asset-generator/src/vrm_ext.rs`:

```rust
//! Builds the JSON fragments for `VRMC_vrm` and `VRMC_materials_mtoon`.
//!
//! Spec references:
//! - VRMC_vrm: https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_vrm-1.0
//! - VRMC_materials_mtoon: https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_materials_mtoon-1.0

use crate::params::{AlphaMode, MToonParams, OutlineWidthMode};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Build the VRMC_vrm extension JSON.
///
/// `bone_to_node` maps VRM bone names to glTF node indices (from the
/// humanoid skeleton).
pub fn vrmc_vrm(meta_name: &str, bone_to_node: &BTreeMap<String, usize>) -> Value {
    let human_bones: serde_json::Map<String, Value> = bone_to_node
        .iter()
        .map(|(name, idx)| (name.clone(), json!({ "node": idx })))
        .collect();

    json!({
        "specVersion": "1.0",
        "meta": {
            "name": meta_name,
            "version": "0.1.0",
            "authors": ["arkavo-org/vrm-conformance generator"],
            "licenseUrl": "https://creativecommons.org/publicdomain/zero/1.0/",
            "thirdPartyLicenses": "",
            "avatarPermission": "everyone",
            "allowExcessivelyViolentUsage": false,
            "allowExcessivelySexualUsage": false,
            "commercialUsage": "personalNonProfit",
            "allowPoliticalOrReligiousUsage": false,
            "allowAntisocialOrHateUsage": false,
            "creditNotation": "unnecessary",
            "allowRedistribution": true,
            "modification": "allowModification"
        },
        "humanoid": {
            "humanBones": human_bones
        },
        "firstPerson": {
            "meshAnnotations": []
        },
        "lookAt": {
            "type": "bone",
            "offsetFromHeadBone": [0.0, 0.06, 0.0],
            "rangeMapHorizontalInner": { "inputMaxValue": 90.0, "outputScale": 10.0 },
            "rangeMapHorizontalOuter": { "inputMaxValue": 90.0, "outputScale": 10.0 },
            "rangeMapVerticalDown":     { "inputMaxValue": 90.0, "outputScale": 10.0 },
            "rangeMapVerticalUp":       { "inputMaxValue": 90.0, "outputScale": 10.0 }
        },
        "expressions": {
            "preset": {}
        }
    })
}

/// Build the per-material VRMC_materials_mtoon extension JSON.
pub fn vrmc_materials_mtoon(p: &MToonParams) -> Value {
    let outline_width_mode = match p.outline_width_mode {
        OutlineWidthMode::None => "none",
        OutlineWidthMode::WorldCoordinates => "worldCoordinates",
        OutlineWidthMode::ScreenCoordinates => "screenCoordinates",
    };

    json!({
        "specVersion": "1.0",
        "transparentWithZWrite": p.transparent_with_z_write,
        "renderQueueOffsetNumber": p.render_queue_offset_number,
        "shadeColorFactor": p.shade_color_factor,
        "shadingShiftFactor": p.shading_shift_factor,
        "shadingToonyFactor": p.shading_toony_factor,
        "giEqualizationFactor": p.gi_equalization_factor,
        "matcapFactor": p.matcap_factor,
        "parametricRimColorFactor": p.parametric_rim_color_factor,
        "parametricRimFresnelPowerFactor": p.parametric_rim_fresnel_power_factor,
        "parametricRimLiftFactor": p.parametric_rim_lift_factor,
        "rimLightingMixFactor": p.rim_lighting_mix_factor,
        "outlineWidthMode": outline_width_mode,
        "outlineWidthFactor": p.outline_width_factor,
        "outlineColorFactor": p.outline_color_factor,
        "outlineLightingMixFactor": p.outline_lighting_mix_factor,
        "uvAnimationScrollXSpeedFactor": p.uv_animation_scroll_x_speed_factor,
        "uvAnimationScrollYSpeedFactor": p.uv_animation_scroll_y_speed_factor,
        "uvAnimationRotationSpeedFactor": p.uv_animation_rotation_speed_factor
    })
}

/// glTF base material wrapping MToon. MToon depends on KHR_materials_unlit
/// in the base material so non-MToon-aware viewers fall back gracefully.
pub fn base_material(p: &MToonParams) -> Value {
    let alpha_mode = match p.alpha_mode {
        AlphaMode::Opaque => "OPAQUE",
        AlphaMode::Mask => "MASK",
        AlphaMode::Blend => "BLEND",
    };

    json!({
        "name": p.id,
        "pbrMetallicRoughness": {
            "baseColorFactor": p.base_color_factor,
            "metallicFactor": 0.0,
            "roughnessFactor": 0.9
        },
        "alphaMode": alpha_mode,
        "doubleSided": p.double_sided,
        "extensions": {
            "KHR_materials_unlit": {},
            "VRMC_materials_mtoon": vrmc_materials_mtoon(p)
        }
    })
}
```

- [ ] **Step 4: Implement the top-level emitter**

`crates/vrm-asset-generator/src/emit.rs`:

```rust
//! Top-level VRM 1.0 asset emission. Combines mesh, buffer, humanoid stub,
//! and VRM extensions into a single `.vrm` GLB on disk.

use crate::buffer::pack_mesh;
use crate::glb::{write_glb, GlbDocument};
use crate::humanoid::minimal_skeleton;
use crate::mesh::sphere;
use crate::params::MToonParams;
use crate::vrm_ext::{base_material, vrmc_vrm};
use anyhow::Result;
use camino::Utf8Path;
use serde_json::{json, Value};

pub fn emit_vrm(params: &MToonParams, output: &Utf8Path) -> Result<()> {
    // 1) Mesh + buffer
    let mesh = sphere(0.3, 24, 48); // small radius so the sphere fits at avatar chest height
    let packed = pack_mesh(&mesh);

    // 2) Humanoid skeleton
    let skeleton = minimal_skeleton();
    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();
    let head_node = skeleton.bone_to_node["head"];

    // 3) Add a mesh-bearing node parented to head (so the sphere visualizes
    //    where the head is). Material 0 = our MToon material.
    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", params.id),
        "mesh": 0
    }));
    // Append mesh_node_index as a child of head.
    let head = &mut nodes[head_node];
    let mut head_children = head["children"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

    // 4) Build the glTF JSON document
    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": ["KHR_materials_unlit", "VRMC_vrm", "VRMC_materials_mtoon"],
        "extensionsRequired": ["VRMC_vrm"],
        "scene": 0,
        "scenes": [
            { "nodes": [skeleton.root_node] }
        ],
        "nodes": nodes,
        "meshes": [
            {
                "name": format!("{}_geom", params.id),
                "primitives": [
                    {
                        "attributes": {
                            "POSITION": 0,
                            "NORMAL": 1,
                            "TEXCOORD_0": 2
                        },
                        "indices": 3,
                        "material": 0,
                        "mode": 4 // TRIANGLES
                    }
                ]
            }
        ],
        "materials": [base_material(params)],
        "extensions": {
            "VRMC_vrm": vrmc_vrm(&params.id, &skeleton.bone_to_node)
        }
    });

    // Splice in buffers/bufferViews/accessors from the packed mesh.
    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    // 5) Serialize and write GLB
    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, glb)?;
    Ok(())
}
```

- [ ] **Step 5: Wire**

Add to `lib.rs`:

```rust
pub mod buffer;
pub mod emit;
pub mod glb;
pub mod humanoid;
pub mod mesh;
pub mod params;
pub mod vrm_ext;
```

- [ ] **Step 6: Run test**

`cargo test -p vrm-asset-generator --test emit -- --nocapture` → both tests should pass with 0 validator errors.

> **Caveat for the implementing engineer:** the validator may flag specific issues that require iteration on the JSON we emit:
> - Missing required VRMC_vrm fields (e.g., `meta.contactInformation`, additional first-person settings) — add them.
> - bufferView alignment: glTF requires VEC3 FLOAT bufferViews to be 4-aligned, which our packer enforces, but if the validator complains about `BUFFER_VIEW_INVALID_BYTE_STRIDE` or similar, add an explicit `byteStride` to vertex bufferViews.
> - VRMC_materials_mtoon required fields the spec lists that we may have omitted. Read the validator messages and adjust `vrm_ext.rs` accordingly.
>
> **Iterate until both tests pass.** Do not commit until they are green.

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-asset-generator/
git commit -m "feat(asset-generator): emit validator-clean .vrm files with MToon material"
```

---

### Task F3: Sidecar emission (.meta.json + .test.yaml)

**Files:**
- Create: `crates/vrm-asset-generator/src/sidecar.rs`
- Modify: `crates/vrm-asset-generator/src/emit.rs`
- Create: `crates/vrm-asset-generator/tests/sidecar.rs`

The same `MToonParams` that drives emission also drives the sidecars. The `.meta.json` records what the asset is (license, parameter values, generator version). The `.test.yaml` is a `vrm_test_plan::TestPlan` derived from the params with sensible camera/lighting defaults.

- [ ] **Step 1: Failing test**

`crates/vrm-asset-generator/tests/sidecar.rs`:

```rust
use camino::Utf8PathBuf;
use vrm_asset_generator::{emit::emit_with_sidecars, params::MToonParams};

#[test]
fn emit_with_sidecars_produces_three_files() {
    let dir = tempfile::tempdir().unwrap();
    let stem = Utf8PathBuf::from_path_buf(dir.path().join("test_asset")).unwrap();

    emit_with_sidecars(&MToonParams::defaults("test_asset"), &stem).unwrap();

    assert!(stem.with_extension("vrm").exists());
    assert!(stem.with_extension("meta.json").exists());
    assert!(stem.with_extension("test.yaml").exists());
}

#[test]
fn meta_json_contains_parameter_values() {
    let dir = tempfile::tempdir().unwrap();
    let stem = Utf8PathBuf::from_path_buf(dir.path().join("a")).unwrap();
    let mut params = MToonParams::defaults("a");
    params.shading_shift_factor = -0.5;
    emit_with_sidecars(&params, &stem).unwrap();

    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(stem.with_extension("meta.json")).unwrap())
            .unwrap();
    assert_eq!(meta["params"]["shading_shift_factor"], -0.5);
    assert_eq!(meta["license"], "CC0-1.0");
    assert!(meta["blake3"].as_str().unwrap().starts_with("blake3:"));
}

#[test]
fn test_yaml_round_trips_into_test_plan() {
    let dir = tempfile::tempdir().unwrap();
    let stem = Utf8PathBuf::from_path_buf(dir.path().join("a")).unwrap();
    emit_with_sidecars(&MToonParams::defaults("a"), &stem).unwrap();

    let yaml = std::fs::read_to_string(stem.with_extension("test.yaml")).unwrap();
    let plan: vrm_test_plan::TestPlan = serde_yml::from_str(&yaml).unwrap();
    assert_eq!(plan.id, "a");
    assert!(matches!(plan.post_processing.tone_mapping, vrm_test_plan::ToneMapping::None));
    assert!(!plan.lighting.cast_shadows, "MToon math tests must run shadows-off");
}
```

- [ ] **Step 2: Implement sidecars**

`crates/vrm-asset-generator/src/sidecar.rs`:

```rust
//! Sidecar emission: `.meta.json` and `.test.yaml`, both derived from the
//! same `MToonParams` that produced the `.vrm`.

use crate::params::MToonParams;
use anyhow::Result;
use camino::Utf8Path;
use serde_json::json;
use vrm_test_plan::{
    AmbientLight, BboxRegion, Camera, ColorSpace, Diff, DiffMode, DirectionalLight, Lighting,
    Output, PostProcessing, PropertyAssertion, TestPlan, ToneMapping,
};

pub fn write_meta_json(params: &MToonParams, vrm_path: &Utf8Path, out: &Utf8Path) -> Result<()> {
    let bytes = std::fs::read(vrm_path)?;
    let hash = blake3::hash(&bytes);
    let meta = json!({
        "id": params.id,
        "license": "CC0-1.0",
        "generator": format!("arkavo-org/vrm-conformance vrm-asset-generator {}", env!("CARGO_PKG_VERSION")),
        "spec_section": "VRMC_materials_mtoon",
        "blake3": format!("blake3:{}", hash.to_hex()),
        "byte_size": bytes.len(),
        "params": params,
    });
    std::fs::write(out, serde_json::to_vec_pretty(&meta)?)?;
    Ok(())
}

pub fn build_default_test_plan(params: &MToonParams, asset_relpath: &str) -> TestPlan {
    TestPlan {
        id: params.id.clone(),
        spec_section: "VRMC_materials_mtoon".into(),
        asset: asset_relpath.into(),
        camera: Camera {
            // Camera framed on the head-mounted sphere (head ≈ y=1.36, sphere radius 0.3).
            position: [0.0, 1.4, 1.5],
            target: [0.0, 1.4, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_degrees: 30.0,
        },
        lighting: Lighting {
            directional: DirectionalLight {
                dir: [-0.3, -0.6, -0.7],
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
            },
            ambient: AmbientLight {
                color: [0.5, 0.5, 0.5],
                intensity: 0.3,
            },
            cast_shadows: false,   // see docs/methodology.md
            receive_shadows: false,
        },
        post_processing: PostProcessing {
            tone_mapping: ToneMapping::None, // pinned for MToon math
            exposure: 1.0,
        },
        output: Output {
            width: 1024,
            height: 1024,
            color_space: ColorSpace::Linear,
            msaa: 4,
        },
        diff: Diff {
            mode: DiffMode::Ssim,
            threshold: 0.985,
            reference_renderer: "vrm-metal-kit".into(),
        },
        ignore_renderers: Vec::new(),
        properties: default_properties(params),
    }
}

fn default_properties(_params: &MToonParams) -> Vec<PropertyAssertion> {
    // v0.1 default: one general-purpose lower-quad average-luminance check.
    // Test-specific assertions get added per parameter combination later.
    vec![PropertyAssertion {
        name: "avg_luminance_lower_left_quad".into(),
        region: BboxRegion::BboxLowerLeftQuadrant,
        expected: 0.4,
        tolerance: 0.3,
    }]
}

pub fn write_test_yaml(plan: &TestPlan, out: &Utf8Path) -> Result<()> {
    let yaml = serde_yml::to_string(plan)?;
    std::fs::write(out, yaml)?;
    Ok(())
}
```

- [ ] **Step 3: Add the high-level emit-with-sidecars to `emit.rs`**

Append to `crates/vrm-asset-generator/src/emit.rs`:

```rust
use crate::sidecar::{build_default_test_plan, write_meta_json, write_test_yaml};

/// Emits `<stem>.vrm`, `<stem>.meta.json`, and `<stem>.test.yaml` from a
/// single MToonParams value.
pub fn emit_with_sidecars(params: &MToonParams, stem: &Utf8Path) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm(params, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(params, &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = build_default_test_plan(params, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}
```

- [ ] **Step 4: Wire**

Add to `lib.rs`: `pub mod sidecar;`

- [ ] **Step 5: Run tests**

`cargo test -p vrm-asset-generator` → all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/
git commit -m "feat(asset-generator): paired .meta.json + .test.yaml sidecars"
```

---

### Task F4: Structured CLI surface for vrm-asset-generator

**Files:**
- Modify: `crates/vrm-asset-generator/src/main.rs`
- Create: `crates/vrm-asset-generator/src/cli.rs`
- Create: `crates/vrm-asset-generator/tests/cli.rs`

Per the agent-first contract: CLI with `--json` mode + `describe` subcommand. v0.1 has two ops: `emit-default` (emit one default-MToon asset) and `describe`. The sweep matrix expansion (Section G) adds more.

- [ ] **Step 1: Failing CLI test**

`crates/vrm-asset-generator/tests/cli.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn describe_outputs_json_schema() {
    let mut cmd = Command::cargo_bin("vrm-asset-generator").unwrap();
    cmd.args(["describe", "--format", "json"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"operations\""))
        .stdout(predicate::str::contains("\"emit-default\""));
}

#[test]
fn emit_default_writes_three_sidecars() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = dir.path().to_str().unwrap();

    let mut cmd = Command::cargo_bin("vrm-asset-generator").unwrap();
    cmd.args(["emit-default", "--id", "smoke", "--output-dir", out_dir]);
    cmd.assert().success();

    let stem = std::path::Path::new(out_dir).join("smoke");
    assert!(stem.with_extension("vrm").exists());
    assert!(stem.with_extension("meta.json").exists());
    assert!(stem.with_extension("test.yaml").exists());
}
```

Add the test deps to `crates/vrm-asset-generator/Cargo.toml`:

```toml
[dev-dependencies]
insta.workspace = true
pretty_assertions.workspace = true
tempfile.workspace = true
assert_cmd = "2.0"
predicates = "3.1"
```

- [ ] **Step 2: Implement CLI**

`crates/vrm-asset-generator/src/cli.rs`:

```rust
use crate::emit::emit_with_sidecars;
use crate::params::MToonParams;
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;

#[derive(Debug, Parser)]
#[command(version, about = "Parametric VRM 1.0 test asset generator")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Cmd,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Emit a `.vrm` + `.meta.json` + `.test.yaml` triplet using the
    /// VRMC_materials_mtoon spec defaults.
    EmitDefault {
        #[arg(long)]
        id: String,
        #[arg(long)]
        output_dir: Utf8PathBuf,
        /// Emit JSON status to stdout instead of human text.
        #[arg(long)]
        json: bool,
    },

    /// Print the operation catalog (JSON Schema by default).
    Describe {
        #[arg(long, value_enum, default_value_t = DescribeFormat::Json)]
        format: DescribeFormat,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DescribeFormat {
    Json,
    Text,
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Cmd::EmitDefault {
            id,
            output_dir,
            json: emit_json,
        } => {
            std::fs::create_dir_all(&output_dir)?;
            let stem = output_dir.join(&id);
            let params = MToonParams::defaults(&id);
            emit_with_sidecars(&params, &stem)?;

            if emit_json {
                let result = json!({
                    "ok": true,
                    "outputs": {
                        "vrm": stem.with_extension("vrm"),
                        "meta": stem.with_extension("meta.json"),
                        "test_plan": stem.with_extension("test.yaml")
                    }
                });
                println!("{}", serde_json::to_string(&result)?);
            } else {
                println!("emitted: {}", stem.with_extension("vrm"));
                println!("emitted: {}", stem.with_extension("meta.json"));
                println!("emitted: {}", stem.with_extension("test.yaml"));
            }
            Ok(())
        }
        Cmd::Describe { format } => {
            let catalog = json!({
                "name": "vrm-asset-generator",
                "version": env!("CARGO_PKG_VERSION"),
                "operations": {
                    "emit-default": {
                        "summary": "Emit a default-MToon asset triplet (.vrm + .meta.json + .test.yaml)",
                        "input_schema": {
                            "type": "object",
                            "required": ["id", "output_dir"],
                            "properties": {
                                "id": { "type": "string" },
                                "output_dir": { "type": "string" }
                            }
                        }
                    }
                }
            });
            match format {
                DescribeFormat::Json => println!("{}", serde_json::to_string_pretty(&catalog)?),
                DescribeFormat::Text => println!("{:#?}", catalog),
            }
            Ok(())
        }
    }
}
```

- [ ] **Step 3: Wire up `main.rs`**

`crates/vrm-asset-generator/src/main.rs`:

```rust
use clap::Parser;
use vrm_asset_generator::cli;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = cli::Cli::parse();
    cli::run(cli)
}
```

Add to `lib.rs`: `pub mod cli;`

- [ ] **Step 4: Run tests**

`cargo test -p vrm-asset-generator` → all pass.

- [ ] **Step 5: Smoke-test from shell**

```bash
mkdir -p /tmp/smoke && cargo run -p vrm-asset-generator -- emit-default --id smoke --output-dir /tmp/smoke --json
ls /tmp/smoke/
cargo run -p vrm-asset-generator -- describe --format json | head -20
```

Expected: JSON status, three files emitted, describe prints schema.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/
git commit -m "feat(asset-generator): structured CLI with emit-default and describe"
```

---

## Section G — MToon sweep matrix

The Phase 1 v0.1 corpus is the **MToon basic parameter sweep**: a 1-D sweep along each axis from `MToonParams::defaults()`, holding all other params at default. ~50 assets, deterministic.

Spring bones, constraints, expressions, alphaMode×outline interaction matrix, UV animation are deferred to Phase 2 (handover §5.1).

### Task G1: Sweep matrix definition

**Files:**
- Create: `crates/vrm-asset-generator/src/sweep.rs`
- Modify: `crates/vrm-asset-generator/src/lib.rs`
- Create: `crates/vrm-asset-generator/tests/sweep.rs`

- [ ] **Step 1: Failing test**

`crates/vrm-asset-generator/tests/sweep.rs`:

```rust
use vrm_asset_generator::sweep::mtoon_basic_sweep;

#[test]
fn basic_sweep_yields_expected_count() {
    let assets = mtoon_basic_sweep();
    // Spec axes (handover §5.1): shading_shift (7), shading_toony (6),
    // gi_equalization (4), rim_lighting_mix (5), outline width × mode
    // (3 modes × 4 widths) = 12, render queue offset (3), double_sided (2).
    // Plus the all-defaults baseline. Total target ≈ 40-50.
    assert!(
        assets.len() >= 40 && assets.len() <= 60,
        "sweep should be ~50 assets, got {}",
        assets.len()
    );

    // IDs must be unique.
    let mut ids: Vec<&str> = assets.iter().map(|p| p.id.as_str()).collect();
    ids.sort();
    let len = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), len, "duplicate IDs in sweep");

    // Default baseline is included.
    assert!(assets.iter().any(|p| p.id == "mtoon_default"));
}
```

- [ ] **Step 2: Implement**

`crates/vrm-asset-generator/src/sweep.rs`:

```rust
//! MToon basic parameter sweep: ~50 assets, one per axis-value pair, all
//! other parameters held at `MToonParams::defaults()`.

use crate::params::{MToonParams, OutlineWidthMode};

pub fn mtoon_basic_sweep() -> Vec<MToonParams> {
    let mut out = Vec::new();

    // Baseline.
    out.push(MToonParams::defaults("mtoon_default"));

    // shadingShiftFactor: -1.0 .. 1.0
    for v in [-1.0, -0.5, -0.2, 0.0, 0.2, 0.5, 1.0] {
        let mut p = MToonParams::defaults(&format!("mtoon_shadingShift_{}", fmt_num(v)));
        p.shading_shift_factor = v;
        out.push(p);
    }

    // shadingToonyFactor: 0.0 .. 1.0
    for v in [0.0, 0.25, 0.5, 0.75, 0.95, 1.0] {
        let mut p = MToonParams::defaults(&format!("mtoon_shadingToony_{}", fmt_num(v)));
        p.shading_toony_factor = v;
        out.push(p);
    }

    // giEqualizationFactor
    for v in [0.0, 0.5, 0.9, 1.0] {
        let mut p = MToonParams::defaults(&format!("mtoon_giEqualization_{}", fmt_num(v)));
        p.gi_equalization_factor = v;
        out.push(p);
    }

    // rimLightingMixFactor (the three-vrm v3.5.0 regression source)
    for v in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let mut p = MToonParams::defaults(&format!("mtoon_rimLightingMix_{}", fmt_num(v)));
        p.rim_lighting_mix_factor = v;
        // Pair with a non-zero rim color so the parameter actually matters
        // visually.
        p.parametric_rim_color_factor = [1.0, 0.5, 0.0];
        p.parametric_rim_fresnel_power_factor = 5.0;
        out.push(p);
    }

    // Outline mode × width
    for &mode in &[
        OutlineWidthMode::None,
        OutlineWidthMode::WorldCoordinates,
        OutlineWidthMode::ScreenCoordinates,
    ] {
        for &w in &[0.01_f32, 0.03, 0.05, 0.10] {
            let mode_str = match mode {
                OutlineWidthMode::None => "none",
                OutlineWidthMode::WorldCoordinates => "world",
                OutlineWidthMode::ScreenCoordinates => "screen",
            };
            let mut p = MToonParams::defaults(&format!(
                "mtoon_outline_{mode_str}_{w}",
                w = fmt_num(w)
            ));
            p.outline_width_mode = mode;
            // For mode == None, width is meaningless but we emit a single
            // baseline. Skip the width loop's 3 trailing variants.
            if matches!(mode, OutlineWidthMode::None) && w != 0.01 {
                continue;
            }
            p.outline_width_factor = w;
            p.outline_color_factor = [0.0, 0.0, 0.0];
            out.push(p);
        }
    }

    // renderQueueOffsetNumber
    for v in [-9_i32, 0, 9] {
        let mut p = MToonParams::defaults(&format!("mtoon_renderQueueOffset_{v}"));
        p.render_queue_offset_number = v;
        out.push(p);
    }

    // doubleSided
    for v in [false, true] {
        let mut p = MToonParams::defaults(&format!("mtoon_doubleSided_{v}"));
        p.double_sided = v;
        out.push(p);
    }

    out
}

fn fmt_num<T: std::fmt::Display + Copy + PartialOrd + Default>(v: T) -> String
where
    f64: From<T>,
{
    let f = f64::from(v);
    let s = format!("{:.3}", f).replace('.', "p").replace('-', "neg");
    s.trim_end_matches('0').trim_end_matches('p').to_string()
}
```

- [ ] **Step 3: Wire**

Add `pub mod sweep;` to `lib.rs`.

- [ ] **Step 4: Run test**

`cargo test -p vrm-asset-generator --test sweep` → green.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/
git commit -m "feat(asset-generator): MToon basic sweep matrix definition"
```

---

### Task G2: `emit-sweep` CLI subcommand + corpus regeneration

**Files:**
- Modify: `crates/vrm-asset-generator/src/cli.rs`

- [ ] **Step 1: Add `emit-sweep` subcommand**

In `Cmd` enum, add:

```rust
/// Emit the full MToon basic sweep (~50 assets) into output_dir/.
EmitSweep {
    #[arg(long)]
    output_dir: Utf8PathBuf,
    /// Emit JSON progress on stderr (NDJSON) and a final JSON summary on stdout.
    #[arg(long)]
    json: bool,
},
```

In `run()` add:

```rust
Cmd::EmitSweep { output_dir, json: emit_json } => {
    use crate::sweep::mtoon_basic_sweep;
    std::fs::create_dir_all(&output_dir)?;
    let assets = mtoon_basic_sweep();
    let total = assets.len();

    let mut emitted = Vec::new();
    for (i, p) in assets.iter().enumerate() {
        if emit_json {
            // NDJSON progress on stderr
            let evt = json!({
                "event": "progress",
                "op": "emit-sweep",
                "index": i,
                "total": total,
                "id": p.id
            });
            eprintln!("{}", serde_json::to_string(&evt)?);
        } else {
            eprintln!("[{:3}/{}] {}", i + 1, total, p.id);
        }

        let stem = output_dir.join(&p.id);
        emit_with_sidecars(p, &stem)?;
        emitted.push(stem);
    }

    if emit_json {
        let summary = json!({
            "ok": true,
            "count": emitted.len(),
            "output_dir": output_dir,
            "assets": emitted
        });
        println!("{}", serde_json::to_string(&summary)?);
    } else {
        println!("emitted {} assets to {}", emitted.len(), output_dir);
    }
    Ok(())
}
```

Also add `"emit-sweep"` to the `describe` operation catalog.

- [ ] **Step 2: Smoke-test the CLI**

```bash
mkdir -p /tmp/sweep && cargo run -p vrm-asset-generator -- emit-sweep --output-dir /tmp/sweep --json 2>/tmp/sweep.log
ls /tmp/sweep/ | wc -l   # should be 3 × ~50 = ~150 files
head -3 /tmp/sweep.log   # NDJSON progress
```

- [ ] **Step 3: Commit**

```bash
git add crates/vrm-asset-generator/
git commit -m "feat(asset-generator): emit-sweep CLI subcommand with NDJSON progress"
```

---

### Task G3: Validate the entire emitted corpus

**Files:**
- Create: `crates/vrm-asset-generator/tests/corpus_validation.rs`

- [ ] **Step 1: Integration test**

`crates/vrm-asset-generator/tests/corpus_validation.rs`:

```rust
//! Slow integration test: emits the full MToon sweep into a temp dir
//! and runs every .vrm through the validator. Marked `#[ignore]` so it
//! doesn't run on every `cargo test`; CI runs it explicitly via
//! `cargo test --ignored corpus_validation`.

use camino::Utf8PathBuf;
use vrm_asset_generator::{emit::emit_with_sidecars, sweep::mtoon_basic_sweep};
use vrm_validator_wrap::{validate, ValidatorConfig};

#[test]
#[ignore = "slow; run via cargo test -- --ignored"]
fn full_sweep_validates_clean() {
    let cfg = ValidatorConfig::from_env().expect("install validator first");

    let dir = tempfile::tempdir().unwrap();
    let out_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();

    let mut failures = Vec::new();
    for (i, p) in mtoon_basic_sweep().iter().enumerate() {
        let stem = out_dir.join(&p.id);
        emit_with_sidecars(p, &stem).expect("emission must succeed");
        let vrm = stem.with_extension("vrm");

        let report = match validate(&cfg, &vrm) {
            Ok(r) => r,
            Err(e) => {
                failures.push((p.id.clone(), format!("validator error: {e}")));
                continue;
            }
        };
        if report.issues.num_errors > 0 {
            let summary = report
                .issues
                .messages
                .iter()
                .filter(|m| m.severity == 0)
                .map(|m| format!("{}: {}", m.code, m.message))
                .collect::<Vec<_>>()
                .join("; ");
            failures.push((p.id.clone(), summary));
        }
        eprintln!("[{:3}] {} OK", i, p.id);
    }

    if !failures.is_empty() {
        for (id, msg) in &failures {
            eprintln!("FAIL: {id}: {msg}");
        }
        panic!("{} of ~50 assets failed validation", failures.len());
    }
}
```

- [ ] **Step 2: Run the integration test**

```bash
cargo test -p vrm-asset-generator --test corpus_validation -- --ignored --nocapture
```

Expected: every asset validates clean. If any fail, fix `vrm_ext.rs` and re-run.

- [ ] **Step 3: Commit**

```bash
git add crates/vrm-asset-generator/
git commit -m "test(asset-generator): full-sweep validation integration test"
```

---

## Section H — Diff engine

### Task H1: SSIM diff (TDD)

**Files:**
- Modify: `crates/vrm-diff-engine/Cargo.toml`
- Create: `crates/vrm-diff-engine/src/lib.rs`
- Create: `crates/vrm-diff-engine/src/ssim.rs`
- Create: `crates/vrm-diff-engine/tests/ssim.rs`

- [ ] **Step 1: Update deps**

`crates/vrm-diff-engine/Cargo.toml`:

```toml
[package]
name = "vrm-diff-engine"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
image.workspace = true
image-compare.workspace = true
camino.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Failing test**

`crates/vrm-diff-engine/tests/ssim.rs`:

```rust
use vrm_diff_engine::ssim::ssim_pngs;

fn make_solid_color(w: u32, h: u32, rgb: [u8; 3]) -> image::RgbImage {
    image::RgbImage::from_fn(w, h, |_, _| image::Rgb(rgb))
}

#[test]
fn identical_images_score_one() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    make_solid_color(64, 64, [128, 64, 200]).save(&a).unwrap();
    make_solid_color(64, 64, [128, 64, 200]).save(&b).unwrap();

    let score = ssim_pngs(
        camino::Utf8Path::from_path(&a).unwrap(),
        camino::Utf8Path::from_path(&b).unwrap(),
    )
    .unwrap();
    assert!((score - 1.0).abs() < 1e-6, "identical → SSIM ~ 1, got {score}");
}

#[test]
fn very_different_images_score_low() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    make_solid_color(64, 64, [0, 0, 0]).save(&a).unwrap();
    make_solid_color(64, 64, [255, 255, 255]).save(&b).unwrap();

    let score = ssim_pngs(
        camino::Utf8Path::from_path(&a).unwrap(),
        camino::Utf8Path::from_path(&b).unwrap(),
    )
    .unwrap();
    assert!(score < 0.5, "black vs white → SSIM should be low, got {score}");
}

#[test]
fn dimension_mismatch_errors() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    make_solid_color(32, 32, [0, 0, 0]).save(&a).unwrap();
    make_solid_color(64, 64, [0, 0, 0]).save(&b).unwrap();

    let err = ssim_pngs(
        camino::Utf8Path::from_path(&a).unwrap(),
        camino::Utf8Path::from_path(&b).unwrap(),
    )
    .unwrap_err();
    assert!(err.to_string().to_lowercase().contains("dimension"));
}
```

- [ ] **Step 3: Implement**

`crates/vrm-diff-engine/src/ssim.rs`:

```rust
//! SSIM (Structural Similarity) over RGB PNGs.

use camino::Utf8Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SsimError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("image decode: {0}")]
    Decode(#[from] image::ImageError),
    #[error("dimension mismatch: {0}x{1} vs {2}x{3}")]
    Dimension(u32, u32, u32, u32),
    #[error("ssim computation failed: {0}")]
    Compute(String),
}

pub fn ssim_pngs(a: &Utf8Path, b: &Utf8Path) -> Result<f64, SsimError> {
    let img_a = image::open(a.as_std_path())?.to_rgb8();
    let img_b = image::open(b.as_std_path())?.to_rgb8();

    if img_a.dimensions() != img_b.dimensions() {
        let (aw, ah) = img_a.dimensions();
        let (bw, bh) = img_b.dimensions();
        return Err(SsimError::Dimension(aw, ah, bw, bh));
    }

    let result = image_compare::rgb_hybrid_compare(&img_a, &img_b)
        .map_err(|e| SsimError::Compute(e.to_string()))?;

    Ok(result.score)
}
```

- [ ] **Step 4: Wire `lib.rs`**

`crates/vrm-diff-engine/src/lib.rs`:

```rust
//! SSIM + property-assertion diff engine for cross-renderer comparison.

pub mod ssim;
```

- [ ] **Step 5: Tests pass**

`cargo test -p vrm-diff-engine` → all green.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-diff-engine/ Cargo.lock
git commit -m "feat(diff-engine): SSIM diff via image-compare with TDD"
```

---

### Task H2: Property assertions on bbox-relative regions

**Files:**
- Create: `crates/vrm-diff-engine/src/property.rs`
- Modify: `crates/vrm-diff-engine/src/lib.rs`
- Create: `crates/vrm-diff-engine/tests/property.rs`

The avatar's screen-space bounding box is computed by finding non-background pixels (we declare a magenta `[255, 0, 255]` background sentinel for v0.1; later we infer from alpha). Region samples are taken within that bbox.

- [ ] **Step 1: Failing test**

`crates/vrm-diff-engine/tests/property.rs`:

```rust
use vrm_diff_engine::property::{eval_property, BboxRegion, PropertyAssertion};

fn make_test_image() -> image::RgbImage {
    // 100×100 with a dark gray 50×50 square centered (the "avatar") on a
    // magenta background. Avatar gray = ~0.25 luminance.
    let mut img = image::RgbImage::from_pixel(100, 100, image::Rgb([255, 0, 255]));
    for y in 25..75 {
        for x in 25..75 {
            img.put_pixel(x, y, image::Rgb([64, 64, 64]));
        }
    }
    img
}

#[test]
fn full_bbox_average_luminance() {
    let img = make_test_image();
    let pa = PropertyAssertion {
        name: "avg_lum".into(),
        region: BboxRegion::BboxFull,
        expected: 0.25,
        tolerance: 0.05,
    };
    let result = eval_property(&img, &pa).unwrap();
    assert!(
        result.passed,
        "expected pass, got actual={} tolerance band ±{}",
        result.actual, pa.tolerance
    );
}

#[test]
fn lower_left_quad_only_samples_lower_left() {
    let mut img = image::RgbImage::from_pixel(100, 100, image::Rgb([255, 0, 255]));
    // Avatar bbox: 25..75, 25..75. Lower-left quad (in image-Y-down): 50..75, 25..50.
    for y in 25..75 {
        for x in 25..75 {
            // Make lower-left quad bright, others dark.
            let bright = (50..75).contains(&y) && (25..50).contains(&x);
            let v = if bright { 200 } else { 50 };
            img.put_pixel(x, y, image::Rgb([v, v, v]));
        }
    }
    let pa = PropertyAssertion {
        name: "ll".into(),
        region: BboxRegion::BboxLowerLeftQuadrant,
        expected: 200.0 / 255.0,
        tolerance: 0.1,
    };
    let r = eval_property(&img, &pa).unwrap();
    assert!(r.passed, "lower-left quad should sample only bright pixels, actual={}", r.actual);
}
```

- [ ] **Step 2: Implement**

`crates/vrm-diff-engine/src/property.rs`:

```rust
//! Bounding-box-relative property assertions.
//!
//! v0.1 assumption: background is the magenta sentinel [255, 0, 255]. Any
//! non-magenta pixel is "avatar." The screen-space bbox is the smallest
//! rectangle containing all avatar pixels. Region samples are taken within
//! that bbox.

use image::RgbImage;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyAssertion {
    pub name: String,
    pub region: BboxRegion,
    pub expected: f32,
    pub tolerance: f32,
}

#[derive(Debug, Clone)]
pub struct PropertyResult {
    pub name: String,
    pub actual: f32,
    pub expected: f32,
    pub tolerance: f32,
    pub passed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum PropertyError {
    #[error("avatar bbox is empty (image is all-background)")]
    EmptyBbox,
}

pub fn compute_avatar_bbox(img: &RgbImage) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = img.dimensions();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;

    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x, y);
            // Magenta sentinel = background.
            if p.0 == [255, 0, 255] {
                continue;
            }
            if x < min_x {
                min_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if x > max_x {
                max_x = x;
            }
            if y > max_y {
                max_y = y;
            }
        }
    }

    if max_x < min_x || max_y < min_y {
        None
    } else {
        Some((min_x, min_y, max_x, max_y))
    }
}

pub fn region_pixel_range(
    bbox: (u32, u32, u32, u32),
    region: BboxRegion,
) -> (u32, u32, u32, u32) {
    let (x0, y0, x1, y1) = bbox;
    let mid_x = x0 + (x1 - x0) / 2;
    let mid_y = y0 + (y1 - y0) / 2;
    let strip_x = ((x1 - x0) / 4).max(1);
    let strip_y = ((y1 - y0) / 4).max(1);

    match region {
        BboxRegion::BboxFull => (x0, y0, x1, y1),
        BboxRegion::BboxUpperLeftQuadrant => (x0, y0, mid_x, mid_y),
        BboxRegion::BboxUpperRightQuadrant => (mid_x, y0, x1, mid_y),
        BboxRegion::BboxLowerLeftQuadrant => (x0, mid_y, mid_x, y1),
        BboxRegion::BboxLowerRightQuadrant => (mid_x, mid_y, x1, y1),
        BboxRegion::BboxCenterStripHorizontal => {
            (x0, mid_y - strip_y, x1, mid_y + strip_y)
        }
        BboxRegion::BboxCenterStripVertical => {
            (mid_x - strip_x, y0, mid_x + strip_x, y1)
        }
    }
}

pub fn eval_property(
    img: &RgbImage,
    pa: &PropertyAssertion,
) -> Result<PropertyResult, PropertyError> {
    let bbox = compute_avatar_bbox(img).ok_or(PropertyError::EmptyBbox)?;
    let (rx0, ry0, rx1, ry1) = region_pixel_range(bbox, pa.region);

    let mut sum = 0f64;
    let mut count = 0u64;
    for y in ry0..=ry1 {
        for x in rx0..=rx1 {
            let p = img.get_pixel(x, y);
            if p.0 == [255, 0, 255] {
                continue;
            }
            let lum = 0.2126 * (p.0[0] as f64 / 255.0)
                + 0.7152 * (p.0[1] as f64 / 255.0)
                + 0.0722 * (p.0[2] as f64 / 255.0);
            sum += lum;
            count += 1;
        }
    }
    let actual = if count == 0 { 0.0 } else { sum / count as f64 };
    let actual = actual as f32;

    let passed = (actual - pa.expected).abs() <= pa.tolerance;

    Ok(PropertyResult {
        name: pa.name.clone(),
        actual,
        expected: pa.expected,
        tolerance: pa.tolerance,
        passed,
    })
}
```

- [ ] **Step 3: Wire**

Add `pub mod property;` to `lib.rs`.

- [ ] **Step 4: Tests pass**

`cargo test -p vrm-diff-engine` → all green.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-diff-engine/
git commit -m "feat(diff-engine): bbox-relative property assertions on rendered PNGs"
```

---

### Task H3: Combined diff result + serialization

**Files:**
- Create: `crates/vrm-diff-engine/src/result.rs`
- Modify: `crates/vrm-diff-engine/src/lib.rs`

Single struct combining SSIM + property results, JSON-serializable for the runner's output and the comparison site's manifest.

- [ ] **Step 1: Implementation (no failing test needed; just shape)**

`crates/vrm-diff-engine/src/result.rs`:

```rust
use crate::property::PropertyResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    pub test_id: String,
    pub renderer: String,
    pub reference_renderer: String,

    pub ssim: f32,
    pub ssim_threshold: f32,
    pub ssim_passed: bool,

    #[serde(default)]
    pub properties: Vec<PropertyResult>,
}

impl DiffResult {
    pub fn overall_passed(&self) -> bool {
        self.ssim_passed && self.properties.iter().all(|p| p.passed)
    }
}

// Manual serde for PropertyResult (it's in another module without derives wired).
mod _serde_compat {
    use super::PropertyResult;
    use serde::{Deserialize, Serialize, Serializer};

    impl Serialize for PropertyResult {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            #[derive(Serialize)]
            struct Borrow<'a> {
                name: &'a str,
                actual: f32,
                expected: f32,
                tolerance: f32,
                passed: bool,
            }
            Borrow {
                name: &self.name,
                actual: self.actual,
                expected: self.expected,
                tolerance: self.tolerance,
                passed: self.passed,
            }
            .serialize(s)
        }
    }

    impl<'de> Deserialize<'de> for PropertyResult {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            #[derive(Deserialize)]
            struct Owned {
                name: String,
                actual: f32,
                expected: f32,
                tolerance: f32,
                passed: bool,
            }
            let o = Owned::deserialize(d)?;
            Ok(PropertyResult {
                name: o.name,
                actual: o.actual,
                expected: o.expected,
                tolerance: o.tolerance,
                passed: o.passed,
            })
        }
    }
}
```

> **Simplification:** the `_serde_compat` module above is messy. The cleaner fix is to add `#[derive(Serialize, Deserialize)]` to `PropertyResult` directly in `property.rs` (and add `serde` to its imports). Do that and delete `_serde_compat`.

- [ ] **Step 2: Add Serialize/Deserialize to `PropertyResult`**

In `crates/vrm-diff-engine/src/property.rs`, change:

```rust
#[derive(Debug, Clone)]
pub struct PropertyResult {
```

to:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyResult {
```

And add `use serde::{Deserialize, Serialize};` to `property.rs` imports.

Now delete the `_serde_compat` module from `result.rs`.

- [ ] **Step 3: Wire**

Add `pub mod result;` to `lib.rs`.

- [ ] **Step 4: Compile**

`cargo build -p vrm-diff-engine` → success.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-diff-engine/
git commit -m "feat(diff-engine): combined DiffResult struct with serde derives"
```

---

## Section I — Runner orchestrator

The runner spawns a renderer adapter as a subprocess, talks to it over stdio JSON-RPC (using `vrm-ops`), executes a test plan (load + camera + lighting + post + render + dispose), then runs the diff engine.

### Task I1: Adapter subprocess client

**Files:**
- Modify: `crates/vrm-runner/Cargo.toml`
- Create: `crates/vrm-runner/src/adapter.rs`
- Create: `crates/vrm-runner/src/lib.rs` (replace stub)

- [ ] **Step 1: Update deps**

`crates/vrm-runner/Cargo.toml`:

```toml
[package]
name = "vrm-runner"
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
camino.workspace = true
thiserror.workspace = true

vrm-ops = { path = "../vrm-ops" }
vrm-test-plan = { path = "../vrm-test-plan" }
vrm-diff-engine = { path = "../vrm-diff-engine" }

[dev-dependencies]
tempfile.workspace = true

[[bin]]
name = "vrm-runner"
path = "src/main.rs"

[lib]
path = "src/lib.rs"
```

- [ ] **Step 2: Implement adapter client**

`crates/vrm-runner/src/adapter.rs`:

```rust
//! Spawns a renderer adapter subprocess, sends JSON-RPC requests over stdin,
//! reads framed responses from stdout. The adapter binary path is given by
//! the test plan or runner CLI arg; the protocol is `vrm-ops`.

use anyhow::Result;
use camino::Utf8PathBuf;
use serde::{de::DeserializeOwned, Serialize};
use std::io::{BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use vrm_ops::{
    stdio::{read_message, write_message},
    JsonRpcRequest, JsonRpcResponse, RpcError,
};

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("frame: {0}")]
    Frame(#[from] vrm_ops::stdio::FrameError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("rpc error: {0}")]
    Rpc(#[from] RpcError),
}

pub struct Adapter {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Adapter {
    pub fn spawn(adapter_bin: &Utf8PathBuf, args: &[String]) -> Result<Self, AdapterError> {
        let mut child = Command::new(adapter_bin.as_std_path())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // adapter logs go to operator's stderr
            .spawn()?;

        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = BufReader::new(child.stdout.take().expect("stdout piped"));
        Ok(Adapter {
            child,
            stdin,
            stdout,
            next_id: 1,
        })
    }

    pub fn call<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: P,
    ) -> Result<R, AdapterError> {
        let id = self.next_id;
        self.next_id += 1;

        let req = JsonRpcRequest::new(id, method, params);
        let body = serde_json::to_vec(&req)?;
        write_message(&mut self.stdin, &body)?;
        self.stdin.flush()?;

        let resp_bytes = read_message(&mut self.stdout)?;
        let resp: JsonRpcResponse<R> = serde_json::from_slice(&resp_bytes)?;
        Ok(resp.into_result()?)
    }

    pub fn shutdown(mut self) -> Result<(), AdapterError> {
        // Closing stdin signals adapters to exit gracefully.
        drop(self.stdin);
        let _ = self.child.wait()?;
        Ok(())
    }
}
```

- [ ] **Step 3: Implement lib.rs as the orchestrator entrypoint**

`crates/vrm-runner/src/lib.rs`:

```rust
//! Conformance runner: reads test plans, drives renderer adapters, runs diff engine.

pub mod adapter;
pub mod execute;
pub mod plan_to_ops;
```

- [ ] **Step 4: Compile**

`cargo build -p vrm-runner` → success (the missing modules will be added in I2/I3).

- [ ] **Step 5: Commit (after I3 lands; placeholder until then)**

This task's commit is bundled with I2/I3 below.

---

### Task I2: Test plan → MCP-op translation

**Files:**
- Create: `crates/vrm-runner/src/plan_to_ops.rs`

Maps `vrm_test_plan` types into `vrm_ops::tools` parameter values. The two crates have parallel-but-distinct types deliberately (YAML vs JSON-RPC serialization conventions); this module is the only place they meet.

- [ ] **Step 1: Implement**

`crates/vrm-runner/src/plan_to_ops.rs`:

```rust
//! Convert a `vrm_test_plan::TestPlan` into the per-op parameter values
//! the runner sends to the adapter.

use vrm_ops::tools as ops;
use vrm_test_plan as plan;

pub fn camera_params(session_id: &str, p: &plan::Camera) -> ops::SetCameraParams {
    ops::SetCameraParams {
        session_id: session_id.into(),
        position: p.position,
        target: p.target,
        up: p.up,
        fov_degrees: p.fov_degrees,
    }
}

pub fn lighting_params(session_id: &str, p: &plan::Lighting) -> ops::SetLightingParams {
    ops::SetLightingParams {
        session_id: session_id.into(),
        directional: ops::Directional {
            dir: p.directional.dir,
            color: p.directional.color,
            intensity: p.directional.intensity,
        },
        ambient: ops::Ambient {
            color: p.ambient.color,
            intensity: p.ambient.intensity,
        },
        cast_shadows: p.cast_shadows,
        receive_shadows: p.receive_shadows,
    }
}

pub fn post_processing_params(
    session_id: &str,
    p: &plan::PostProcessing,
) -> ops::SetPostProcessingParams {
    let tone_mapping = match p.tone_mapping {
        plan::ToneMapping::None => ops::ToneMapping::None,
        plan::ToneMapping::Linear => ops::ToneMapping::Linear,
        plan::ToneMapping::Reinhard => ops::ToneMapping::Reinhard,
        plan::ToneMapping::Aces => ops::ToneMapping::Aces,
    };
    ops::SetPostProcessingParams {
        session_id: session_id.into(),
        tone_mapping,
        exposure: p.exposure,
    }
}

pub fn render_params(
    session_id: &str,
    p: &plan::Output,
    output_path: String,
) -> ops::RenderParams {
    let color_space = match p.color_space {
        plan::ColorSpace::Linear => ops::ColorSpace::Linear,
        plan::ColorSpace::Srgb => ops::ColorSpace::Srgb,
    };
    ops::RenderParams {
        session_id: session_id.into(),
        width: p.width,
        height: p.height,
        output_path,
        color_space,
        msaa: p.msaa,
        output_type: ops::OutputType::Color,
    }
}
```

- [ ] **Step 2: Compile.**

`cargo build -p vrm-runner` → success.

---

### Task I3: Test plan executor

**Files:**
- Create: `crates/vrm-runner/src/execute.rs`

The execute path: spawn adapter → load_vrm → set_camera → set_lighting → set_post_processing → render → dispose. NDJSON progress on stderr.

- [ ] **Step 1: Implement**

`crates/vrm-runner/src/execute.rs`:

```rust
//! Execute one test plan against one adapter, producing a PNG.

use crate::adapter::{Adapter, AdapterError};
use crate::plan_to_ops::*;
use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use serde_json::json;
use vrm_ops::tools as ops;
use vrm_test_plan::TestPlan;

#[derive(Debug, Clone)]
pub struct ExecuteOptions {
    pub adapter_bin: Utf8PathBuf,
    pub adapter_args: Vec<String>,
    pub asset_dir: Utf8PathBuf,
    pub output_dir: Utf8PathBuf,
    pub renderer_name: String,
    pub emit_progress_ndjson: bool,
}

#[derive(Debug, Clone)]
pub struct ExecuteResult {
    pub test_id: String,
    pub renderer: String,
    pub output_png: Utf8PathBuf,
    pub actual_color_space: ops::ColorSpace,
}

pub fn execute_plan(plan: &TestPlan, opts: &ExecuteOptions) -> Result<ExecuteResult> {
    let asset_path = opts.asset_dir.join(&plan.asset);
    if !asset_path.exists() {
        anyhow::bail!("asset not found: {asset_path}");
    }

    progress(opts, "spawn", &plan.id, json!({}));
    let mut adapter = Adapter::spawn(&opts.adapter_bin, &opts.adapter_args)?;

    progress(opts, "load_vrm", &plan.id, json!({ "asset": asset_path }));
    let load: ops::LoadVrmResult = adapter.call(
        "load_vrm",
        ops::LoadVrmParams {
            path: asset_path.to_string(),
        },
    )?;
    let session_id = load.session_id;

    progress(opts, "set_camera", &plan.id, json!({}));
    let _: ops::UnitResult =
        adapter.call("set_camera", camera_params(&session_id, &plan.camera))?;

    progress(opts, "set_lighting", &plan.id, json!({}));
    let _: ops::UnitResult =
        adapter.call("set_lighting", lighting_params(&session_id, &plan.lighting))?;

    progress(opts, "set_post_processing", &plan.id, json!({}));
    let _: ops::UnitResult = adapter.call(
        "set_post_processing",
        post_processing_params(&session_id, &plan.post_processing),
    )?;

    let png = opts
        .output_dir
        .join(format!("{}_{}.png", plan.id, opts.renderer_name));
    if let Some(parent) = png.parent() {
        std::fs::create_dir_all(parent)?;
    }
    progress(opts, "render", &plan.id, json!({ "output": png }));
    let render: ops::RenderResult = adapter.call(
        "render",
        render_params(&session_id, &plan.output, png.to_string()),
    )?;

    progress(opts, "dispose", &plan.id, json!({}));
    let _: ops::UnitResult =
        adapter.call("dispose", ops::DisposeParams { session_id })?;
    adapter.shutdown()?;

    Ok(ExecuteResult {
        test_id: plan.id.clone(),
        renderer: opts.renderer_name.clone(),
        output_png: Utf8PathBuf::from(render.output_path),
        actual_color_space: render.actual_color_space,
    })
}

pub fn load_plan(path: &Utf8Path) -> Result<TestPlan> {
    let s = std::fs::read_to_string(path.as_std_path())?;
    Ok(serde_yml::from_str(&s)?)
}

fn progress(opts: &ExecuteOptions, phase: &str, test_id: &str, extra: serde_json::Value) {
    if opts.emit_progress_ndjson {
        let mut o = json!({
            "event": "progress",
            "op": "execute_plan",
            "phase": phase,
            "test_id": test_id,
        });
        if let Some(obj) = o.as_object_mut() {
            if let Some(extra_obj) = extra.as_object() {
                for (k, v) in extra_obj {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
        eprintln!("{}", serde_json::to_string(&o).unwrap_or_default());
    }
}

// Convert AdapterError → anyhow for the bail-on-anything caller path.
impl From<AdapterError> for anyhow::Error {
    fn from(e: AdapterError) -> Self {
        anyhow::anyhow!("adapter error: {e}")
    }
}
```

> **Note:** the `From<AdapterError> for anyhow::Error` impl in this module will conflict if `anyhow` already provides a blanket impl. If `cargo build` complains, replace the impl with `.map_err(|e| anyhow::anyhow!("adapter error: {e}"))?` at each `?` call site instead.

- [ ] **Step 2: Compile + commit I1+I2+I3**

```bash
cargo build -p vrm-runner
git add crates/vrm-runner/ Cargo.lock
git commit -m "feat(runner): adapter subprocess client + plan→ops translation + executor"
```

---

### Task I4: Runner CLI surface

**Files:**
- Modify: `crates/vrm-runner/src/main.rs`
- Create: `crates/vrm-runner/src/cli.rs`
- Modify: `crates/vrm-runner/src/lib.rs` (add `pub mod cli;`)

Subcommands per agent-first contract: `execute-test-plan`, `plan-test-plan` (cost preview), `describe`.

- [ ] **Step 1: Implement CLI**

`crates/vrm-runner/src/cli.rs`:

```rust
use crate::execute::{execute_plan, load_plan, ExecuteOptions};
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;

#[derive(Debug, Parser)]
#[command(version, about = "VRM conformance runner")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Cmd,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Execute a test plan against one renderer adapter.
    ExecuteTestPlan {
        #[arg(long)]
        plan: Utf8PathBuf,
        #[arg(long)]
        adapter_bin: Utf8PathBuf,
        #[arg(long, value_delimiter = ' ', num_args = 0..)]
        adapter_args: Vec<String>,
        #[arg(long)]
        asset_dir: Utf8PathBuf,
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long, default_value = "vrm-metal-kit")]
        renderer_name: String,
        #[arg(long)]
        json: bool,
    },
    /// Cost-preview a test plan without executing.
    PlanTestPlan {
        #[arg(long)]
        plan: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Print the operation catalog.
    Describe {
        #[arg(long, value_enum, default_value_t = DescribeFormat::Json)]
        format: DescribeFormat,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DescribeFormat {
    Json,
    Text,
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Cmd::ExecuteTestPlan {
            plan,
            adapter_bin,
            adapter_args,
            asset_dir,
            output_dir,
            renderer_name,
            json: emit_json,
        } => {
            let plan_value = load_plan(&plan)?;
            let opts = ExecuteOptions {
                adapter_bin,
                adapter_args,
                asset_dir,
                output_dir,
                renderer_name,
                emit_progress_ndjson: emit_json,
            };
            let result = execute_plan(&plan_value, &opts)?;
            if emit_json {
                let summary = json!({
                    "ok": true,
                    "test_id": result.test_id,
                    "renderer": result.renderer,
                    "output_png": result.output_png,
                    "actual_color_space": format!("{:?}", result.actual_color_space)
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!("rendered {} → {}", result.test_id, result.output_png);
            }
            Ok(())
        }
        Cmd::PlanTestPlan { plan, json: emit_json } => {
            let p = load_plan(&plan)?;
            // v0.1 trivial estimate: one render.
            let preview = json!({
                "ok": true,
                "test_id": p.id,
                "estimated_renders": 1,
                "estimated_seconds": 4.0,
                "outputs": [
                    format!("{}_{{renderer}}.png", p.id)
                ]
            });
            if emit_json {
                println!("{}", serde_json::to_string(&preview)?);
            } else {
                println!("would render: {}", p.id);
            }
            Ok(())
        }
        Cmd::Describe { format } => {
            let catalog = json!({
                "name": "vrm-runner",
                "version": env!("CARGO_PKG_VERSION"),
                "operations": {
                    "execute-test-plan": {
                        "summary": "Execute a YAML test plan against one renderer adapter; emit a PNG and JSON status",
                        "input_schema": {
                            "type": "object",
                            "required": ["plan", "adapter_bin", "asset_dir", "output_dir"],
                            "properties": {
                                "plan": { "type": "string" },
                                "adapter_bin": { "type": "string" },
                                "adapter_args": { "type": "array", "items": { "type": "string" } },
                                "asset_dir": { "type": "string" },
                                "output_dir": { "type": "string" }
                            }
                        }
                    },
                    "plan-test-plan": {
                        "summary": "Cost-preview a test plan without executing"
                    }
                }
            });
            match format {
                DescribeFormat::Json => println!("{}", serde_json::to_string_pretty(&catalog)?),
                DescribeFormat::Text => println!("{:#?}", catalog),
            }
            Ok(())
        }
    }
}
```

- [ ] **Step 2: Wire main.rs**

`crates/vrm-runner/src/main.rs`:

```rust
use clap::Parser;
use vrm_runner::cli;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = cli::Cli::parse();
    cli::run(cli)
}
```

Add `pub mod cli;` to `lib.rs`.

- [ ] **Step 3: Smoke-test (build only; full execution needs a real adapter)**

```bash
cargo build -p vrm-runner
cargo run -p vrm-runner -- describe --format json | head
```

- [ ] **Step 4: Commit**

```bash
git add crates/vrm-runner/
git commit -m "feat(runner): structured CLI with execute-test-plan, plan-test-plan, describe"
```

---

## Section J — S3 manifest + push/pull

### Task J1: Manifest schema

**Files:**
- Modify: `crates/vrm-s3/Cargo.toml`
- Create: `crates/vrm-s3/src/manifest.rs`
- Create: `crates/vrm-s3/src/lib.rs` (replace stub)
- Create: `crates/vrm-s3/tests/manifest.rs`

JSON manifest at `goldens/manifest.json` lists every uploaded artifact. Each entry has the submission metadata required by RFC-0002 plus the BLAKE3 hash for content addressing.

- [ ] **Step 1: Update deps**

`crates/vrm-s3/Cargo.toml`:

```toml
[package]
name = "vrm-s3"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
camino.workspace = true
blake3.workspace = true
hex.workspace = true
tokio.workspace = true
aws-config.workspace = true
aws-sdk-s3.workspace = true
tracing.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Failing test**

`crates/vrm-s3/tests/manifest.rs`:

```rust
use vrm_s3::manifest::{Manifest, ManifestEntry, SubmissionMetadata};

#[test]
fn manifest_round_trips_json() {
    let m = Manifest {
        version: 1,
        entries: vec![ManifestEntry {
            test_id: "mtoon_default".into(),
            renderer_name: "vrm-metal-kit".into(),
            renderer_version: "0.5.2".into(),
            git_hash: "deadbeef".into(),
            metadata: SubmissionMetadata {
                os: "macos".into(),
                os_version: "14.4.1".into(),
                gpu_vendor: "Apple".into(),
                gpu_model: "M2 Pro".into(),
                driver_version: "Metal 3".into(),
                build_flags: "release".into(),
            },
            image_url: "s3://arkavo-vrm-conformance/test/mtoon_default.png".into(),
            image_blake3: "blake3:abcdef".into(),
            byte_size: 12345,
            submitted_at: "2026-05-10T12:00:00Z".into(),
        }],
    };
    let s = serde_json::to_string(&m).unwrap();
    let parsed: Manifest = serde_json::from_str(&s).unwrap();
    assert_eq!(parsed.entries.len(), 1);
    assert_eq!(parsed.entries[0].test_id, "mtoon_default");
}

#[test]
fn manifest_rejects_missing_required_fields() {
    let raw = r#"{
        "version": 1,
        "entries": [
            { "test_id": "x", "renderer_name": "r" }
        ]
    }"#;
    let result: Result<Manifest, _> = serde_json::from_str(raw);
    assert!(result.is_err(), "should reject missing required fields");
}
```

- [ ] **Step 3: Implement**

`crates/vrm-s3/src/manifest.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub entries: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub test_id: String,
    pub renderer_name: String,
    pub renderer_version: String,
    pub git_hash: String,

    #[serde(flatten)]
    pub metadata: SubmissionMetadata,

    pub image_url: String,
    pub image_blake3: String,
    pub byte_size: u64,
    pub submitted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionMetadata {
    pub os: String,
    pub os_version: String,
    pub gpu_vendor: String,
    pub gpu_model: String,
    pub driver_version: String,
    pub build_flags: String,
}

impl Manifest {
    pub fn empty() -> Self {
        Self { version: 1, entries: Vec::new() }
    }

    pub fn upsert(&mut self, entry: ManifestEntry) {
        if let Some(existing) = self.entries.iter_mut().find(|e| {
            e.test_id == entry.test_id && e.renderer_name == entry.renderer_name
        }) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }
}
```

- [ ] **Step 4: Wire `lib.rs`**

`crates/vrm-s3/src/lib.rs`:

```rust
//! S3 manifest schema + push/pull tooling for goldens.

pub mod manifest;
pub mod push_pull;
```

- [ ] **Step 5: Tests pass**

`cargo test -p vrm-s3 --test manifest` → green (we can't test push_pull yet — that's J2).

- [ ] **Step 6: Stub push_pull.rs to satisfy lib.rs**

`crates/vrm-s3/src/push_pull.rs`:

```rust
//! Placeholder; J2 fills this in.
```

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-s3/ Cargo.lock
git commit -m "feat(vrm-s3): manifest schema with submission metadata + BLAKE3 refs"
```

---

### Task J2: S3 push + BLAKE3 hashing

**Files:**
- Replace: `crates/vrm-s3/src/push_pull.rs`

Pushes a single PNG to S3, returns the manifest entry. AWS credentials come from the standard AWS SDK chain (env vars, profile, IAM role).

- [ ] **Step 1: Implement push**

`crates/vrm-s3/src/push_pull.rs`:

```rust
//! Push: upload a PNG to S3, compute BLAKE3, return a ManifestEntry.
//! Pull: download by URL into a local file path.

use crate::manifest::{ManifestEntry, SubmissionMetadata};
use anyhow::Result;
use aws_sdk_s3::primitives::ByteStream;
use camino::Utf8Path;

#[derive(Debug, Clone)]
pub struct PushOptions {
    pub bucket: String,
    pub key_prefix: String,
    pub renderer_name: String,
    pub renderer_version: String,
    pub git_hash: String,
    pub metadata: SubmissionMetadata,
}

pub async fn push_png(
    file: &Utf8Path,
    test_id: &str,
    opts: &PushOptions,
) -> Result<ManifestEntry> {
    let bytes = std::fs::read(file.as_std_path())?;
    let hash = blake3::hash(&bytes);
    let blake3_str = format!("blake3:{}", hash.to_hex());

    let key = format!(
        "{}/{}/{}_{}.png",
        opts.key_prefix.trim_matches('/'),
        opts.renderer_name,
        test_id,
        &hash.to_hex().to_string()[..16]
    );

    let cfg = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_s3::Client::new(&cfg);

    client
        .put_object()
        .bucket(&opts.bucket)
        .key(&key)
        .body(ByteStream::from(bytes.clone()))
        .content_type("image/png")
        .send()
        .await?;

    Ok(ManifestEntry {
        test_id: test_id.into(),
        renderer_name: opts.renderer_name.clone(),
        renderer_version: opts.renderer_version.clone(),
        git_hash: opts.git_hash.clone(),
        metadata: opts.metadata.clone(),
        image_url: format!("s3://{}/{}", opts.bucket, key),
        image_blake3: blake3_str,
        byte_size: bytes.len() as u64,
        submitted_at: chrono_now_iso(),
    })
}

pub async fn pull_png(url: &str, dest: &Utf8Path) -> Result<()> {
    let (bucket, key) = parse_s3_url(url)?;
    let cfg = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_s3::Client::new(&cfg);

    let obj = client.get_object().bucket(bucket).key(key).send().await?;
    let bytes = obj.body.collect().await?.into_bytes();
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent.as_std_path())?;
    }
    std::fs::write(dest.as_std_path(), bytes)?;
    Ok(())
}

fn parse_s3_url(url: &str) -> Result<(&str, &str)> {
    let stripped = url.strip_prefix("s3://").ok_or_else(|| {
        anyhow::anyhow!("expected s3:// URL, got {url}")
    })?;
    let (bucket, key) = stripped
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("malformed s3 url: missing key: {url}"))?;
    Ok((bucket, key))
}

fn chrono_now_iso() -> String {
    // Minimal RFC 3339 emission to avoid pulling chrono. system time → seconds → string.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // We only need approximate timestamps; tests don't depend on exact format.
    format!("@unix:{secs}")
}
```

> **Caveat:** `chrono_now_iso()` returns a placeholder. The implementing engineer should swap to a real RFC 3339 emitter — either pull in `time` crate (`time::OffsetDateTime::now_utc().format(...)`) or `chrono`. Add to workspace deps in F2's Cargo.toml step or here. RFC-0002's manifest schema requires real ISO 8601.

- [ ] **Step 2: Commit**

```bash
git add crates/vrm-s3/ Cargo.lock
git commit -m "feat(vrm-s3): S3 push/pull with BLAKE3 content hashing"
```

---

### Task J3: push-goldens / pull-goldens shell helpers

**Files:**
- Create: `scripts/push-goldens.sh`
- Create: `scripts/pull-goldens.sh`
- Create: `crates/vrm-s3/src/bin/push-goldens.rs` (a binary in the s3 crate that the shell scripts wrap)

Shell scripts wrap a Rust binary because the AWS SDK is async and needs tokio.

- [ ] **Step 1: Add the binary**

`crates/vrm-s3/src/bin/push-goldens.rs`:

```rust
use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use clap::Parser;
use vrm_s3::{
    manifest::{Manifest, SubmissionMetadata},
    push_pull::{push_png, PushOptions},
};

#[derive(Debug, Parser)]
struct Args {
    /// PNG file to upload.
    #[arg(long)]
    file: Utf8PathBuf,
    /// Test ID this PNG belongs to.
    #[arg(long)]
    test_id: String,
    /// S3 bucket.
    #[arg(long, env = "VRM_GOLDENS_BUCKET")]
    bucket: String,
    /// S3 key prefix.
    #[arg(long, default_value = "v0.1")]
    key_prefix: String,
    /// Renderer name.
    #[arg(long)]
    renderer_name: String,
    #[arg(long)]
    renderer_version: String,
    #[arg(long)]
    git_hash: String,
    #[arg(long)]
    os: String,
    #[arg(long)]
    os_version: String,
    #[arg(long)]
    gpu_vendor: String,
    #[arg(long)]
    gpu_model: String,
    #[arg(long, default_value = "")]
    driver_version: String,
    #[arg(long, default_value = "release")]
    build_flags: String,
    /// Path to manifest.json to update in place.
    #[arg(long, default_value = "goldens/manifest.json")]
    manifest: Utf8PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let a = Args::parse();
    let opts = PushOptions {
        bucket: a.bucket,
        key_prefix: a.key_prefix,
        renderer_name: a.renderer_name,
        renderer_version: a.renderer_version,
        git_hash: a.git_hash,
        metadata: SubmissionMetadata {
            os: a.os,
            os_version: a.os_version,
            gpu_vendor: a.gpu_vendor,
            gpu_model: a.gpu_model,
            driver_version: a.driver_version,
            build_flags: a.build_flags,
        },
    };
    let entry = push_png(&a.file, &a.test_id, &opts).await?;

    let mut m: Manifest = if a.manifest.exists() {
        serde_json::from_str(&std::fs::read_to_string(&a.manifest)?)?
    } else {
        Manifest::empty()
    };
    m.upsert(entry.clone());

    if let Some(p) = a.manifest.parent() {
        std::fs::create_dir_all(p.as_std_path())?;
    }
    std::fs::write(&a.manifest, serde_json::to_vec_pretty(&m)?)?;

    println!("{}", serde_json::to_string(&entry)?);
    Ok(())
}
```

- [ ] **Step 2: Shell wrapper**

`scripts/push-goldens.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

# Wraps the cargo-built push-goldens binary with sensible defaults pulled from
# `git rev-parse` and `uname`. Called from CI and from operator machines after
# a local render run.

cargo run --release --bin push-goldens -p vrm-s3 -- \
    --file "${1:?usage: push-goldens.sh <png> <test_id> <renderer_name> <renderer_version>}" \
    --test-id "${2:?missing test_id}" \
    --renderer-name "${3:?missing renderer_name}" \
    --renderer-version "${4:?missing renderer_version}" \
    --git-hash "$(git rev-parse HEAD)" \
    --os "$(uname -s | tr '[:upper:]' '[:lower:]')" \
    --os-version "$(uname -r)" \
    --gpu-vendor "${VRM_GPU_VENDOR:-unknown}" \
    --gpu-model "${VRM_GPU_MODEL:-unknown}" \
    --driver-version "${VRM_GPU_DRIVER:-}" \
    --build-flags "release"
```

`scripts/pull-goldens.sh`:

```bash
#!/usr/bin/env bash
# Stub — implementing engineer adds a pull-goldens binary mirroring push.
# For Phase 1 the site fetches images directly from S3 over HTTPS using
# manifest URLs, so a local pull script is only needed for offline diffing.
echo "TODO: implement pull-goldens (see vrm-s3 push_pull::pull_png)"
exit 1
```

- [ ] **Step 3: Make executable + test build**

```bash
chmod +x scripts/push-goldens.sh scripts/pull-goldens.sh
cargo build --release --bin push-goldens -p vrm-s3
```

- [ ] **Step 4: Commit**

```bash
git add scripts/ crates/vrm-s3/
git commit -m "feat(vrm-s3): push-goldens binary + shell wrappers"
```

---

## Section K — Static comparison site

Plain Vite + vanilla TS. No framework. Reads `goldens/manifest.json`, fetches PNGs over HTTPS from S3, displays side-by-side. Three view modes (side-by-side, slider overlay, image diff highlight) — Phase 1 ships side-by-side first; the others land in the same iteration if straightforward.

### Task K1: Vite + TS skeleton

**Files:**
- Create: `site/package.json`, `site/tsconfig.json`, `site/vite.config.ts`, `site/index.html`
- Create: `site/src/main.ts`, `site/src/manifest.ts`, `site/src/style.css`

- [ ] **Step 1: Scaffolding files**

`site/package.json`:

```json
{
  "name": "vrm-conformance-site",
  "private": true,
  "version": "0.0.1",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview --port 4173"
  },
  "devDependencies": {
    "typescript": "^5.4.0",
    "vite": "^5.4.0"
  }
}
```

`site/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "moduleResolution": "bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "skipLibCheck": true
  },
  "include": ["src"]
}
```

`site/vite.config.ts`:

```ts
import { defineConfig } from "vite";
export default defineConfig({
  base: "./",
  build: { outDir: "dist", sourcemap: true },
});
```

`site/index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width,initial-scale=1.0" />
    <title>VRM Conformance — Render Fidelity</title>
    <link rel="stylesheet" href="/src/style.css" />
  </head>
  <body>
    <header>
      <h1>VRM Conformance / Render Fidelity</h1>
      <p>
        Cross-renderer fidelity comparisons for VRM 1.0.
        <a href="https://github.com/arkavo-org/vrm-conformance">repo</a>
      </p>
    </header>
    <main id="app"></main>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

- [ ] **Step 2: Manifest fetcher**

`site/src/manifest.ts` — fetch `/goldens/manifest.json`, define `ManifestEntry` and `Manifest` interfaces matching the Rust types in `vrm-s3::manifest`, group entries by `test_id`, and convert `s3://bucket/key` URLs to public HTTPS URLs (with optional override via `VITE_S3_BASE_URL`). Standard browser `fetch` and `Map`-based grouping; no exotic deps.

- [ ] **Step 3: Side-by-side viewer**

`site/src/main.ts` — load manifest, group by test ID, render one `<section>` per test with a CSS grid of one cell per renderer. Each cell holds the rendered PNG (`<img loading="lazy">`) plus a metadata line (`renderer_version · os · gpu_model · git_hash[:7]`). Missing-renderer cells get a "no submission" placeholder.

`site/src/style.css` — minimal styling: system font stack, white cards on light gray, `grid-template-columns: repeat(auto-fit, minmax(280px, 1fr))`. ~30 lines.

- [ ] **Step 4: Smoke-test locally**

```bash
cd site
npm install
npm run dev
```

Open the printed URL; with no manifest yet, the page should show "Failed to load manifest" — that is fine pre-N1.

- [ ] **Step 5: Commit**

```bash
git add site/
git commit -m "feat(site): Vite + TS skeleton with side-by-side viewer (no framework)"
```

---

### Task K2: Slider overlay + diff highlight modes (defer-friendly)

Add a per-test-row mode switcher with three view modes: side-by-side (default from K1), slider overlay between two renderers (CSS `clip-path` + range input), image-diff highlight (`<canvas>` painting per-pixel `max(|r1-r2|,|g1-g2|,|b1-b2|)` as a heatmap).

> **Skip-or-defer rule:** if Phase 1 timeline is constrained, ship K1 only and open an issue for K2. The site still proves the pipeline; slider/diff are quality-of-life upgrades.

- [ ] **Step 1: Mode switcher UI** (per test row, a `<select>` with three options)
- [ ] **Step 2: Slider overlay** (foreground image with `clip-path: inset(0 X% 0 0)`, range input drives X)
- [ ] **Step 3: Diff canvas** (fetch both PNGs as `ImageBitmap`, draw to canvas, paint diff per pixel)
- [ ] **Step 4: Commit**

```bash
git add site/
git commit -m "feat(site): slider overlay and diff highlight view modes"
```

---

## Section L — VRMMetalKit Swift adapter

> **Scope note:** I cannot specify Swift integration code at the same line-by-line detail as the Rust above, because I don't have access to VRMMetalKit's actual Swift API. This section defines the **contract** the Swift dev satisfies: package layout, the operations they must implement, the test harness against which their adapter is validated. The Swift dev fills in the integration code; the runner-driven smoke test (Section N) is the integration acceptance gate.

### Task L1: Swift package skeleton

**Files:**
- Create: `adapters/vrm-metal-kit/Package.swift`
- Create: `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/main.swift`
- Create: `adapters/vrm-metal-kit/README.md`

- [ ] **Step 1: Package manifest**

```swift
// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "vrm-metal-kit-adapter",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "vrm-metal-kit-adapter", targets: ["VRMMetalKitAdapter"]),
    ],
    dependencies: [
        .package(url: "https://github.com/arkavo-org/VRMMetalKit", branch: "main"),
    ],
    targets: [
        .executableTarget(
            name: "VRMMetalKitAdapter",
            dependencies: [.product(name: "VRMMetalKit", package: "VRMMetalKit")]
        ),
        .testTarget(
            name: "VRMMetalKitAdapterTests",
            dependencies: ["VRMMetalKitAdapter"]
        ),
    ]
)
```

> **Caveat:** the actual product/target names exposed by `arkavo-org/VRMMetalKit` need to be confirmed. If the upstream package vends a target named differently, adjust `.product(name:, package:)`.

- [ ] **Step 2: Stub `main.swift`** that prints a startup line on stderr and otherwise loops idle. Real op handling lands in L2/L3.

- [ ] **Step 3: README** at `adapters/vrm-metal-kit/README.md` explaining build (`swift build --configuration release`), where the binary lands (`.build/release/vrm-metal-kit-adapter`), and how the runner invokes it. Reference `docs/operation-contract.md` for the contract.

- [ ] **Step 4: Commit**

```bash
git add adapters/vrm-metal-kit/
git commit -m "chore(adapter/vrm-metal-kit): Swift package skeleton with contract README"
```

---

### Task L2: Stdio JSON-RPC framing in Swift

The Swift dev implements LSP-style framing (Content-Length header + body) and a request dispatcher: takes a method name and a JSON `params` object, returns a JSON `result` or `error`.

**Acceptance criteria:**

- Reads `Content-Length: N\r\n\r\n` headers + N bytes of body from stdin in a loop.
- Writes responses with the same framing on stdout.
- Routes by method name; unknown methods return `-32601`.
- Reserved-but-unimplemented methods return `-32000` with `data: { "phase": "v1.x" }` or `{"phase":"Phase 2"}`.
- Logs adapter-side traces to stderr (the runner inherits stderr).

> **Reference implementation hint:** the Rust `vrm_ops::stdio::{read_message, write_message}` functions are 50 lines of pure I/O; the Swift port is straightforward.

- [ ] **Step 1: Implement `JsonRpcServer.swift`**
- [ ] **Step 2: Roundtrip test in `Tests/VRMMetalKitAdapterTests/`** — feed a synthetic request stream, assert framed responses come back.
- [ ] **Step 3: Commit**

```bash
git add adapters/vrm-metal-kit/
git commit -m "feat(adapter/vrm-metal-kit): JSON-RPC stdio server scaffolding"
```

---

### Task L3: Phase 1 op implementations

Implement `load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`, `dispose` against VRMMetalKit's actual API.

**Acceptance criteria** (in addition to the JSON shapes from `docs/operation-contract.md`):

- `load_vrm` accepts a path to a `.vrm`, returns a session ID; subsequent ops use that ID.
- `set_post_processing` with `tone_mapping: None` is a no-op pass-through (VRMMetalKit's natural default).
- `render` writes a PNG at `output_path` at the requested resolution.
  - **Background: clear color = magenta `[255, 0, 255]`** (matches the property-assertion bbox sentinel).
  - MSAA 4x.
  - `color_space` honored: linear or sRGB output PNG.
- `dispose` releases GPU resources.
- Reserved ops (`set_environment`, `set_expression`, `set_humanoid_pose`, `set_root_transform`, `animate_root_transform`, `step_physics`, `reset_physics`) return `-32000` Unimplemented with the appropriate `phase` field.

**Validation:** L3 is "done" when Section N's smoke test passes end-to-end.

- [ ] **Step 1: Implement `load_vrm`** (parse via VRMMetalKit, hold the loaded scene in a `[String: Scene]` map keyed by session ID)
- [ ] **Step 2: Implement `set_camera`, `set_lighting`, `set_post_processing`** (apply to scene state)
- [ ] **Step 3: Implement `render`** (drive Metal command buffer, encode framebuffer to PNG)
- [ ] **Step 4: Implement `dispose`** (release scene + Metal resources)
- [ ] **Step 5: Implement Unimplemented stubs for reserved ops**
- [ ] **Step 6: Commit**

```bash
git add adapters/vrm-metal-kit/
git commit -m "feat(adapter/vrm-metal-kit): Phase 1 operations against VRMMetalKit"
```

---

## Section M — CI workflows

### Task M1: Rust CI

**Files:**
- Create: `.github/workflows/rust.yml`

```yaml
name: rust

on:
  pull_request:
    paths:
      - 'crates/**'
      - 'Cargo.toml'
      - 'Cargo.lock'
      - 'rust-toolchain.toml'
      - '.github/workflows/rust.yml'
  push:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
      - name: Install validator shim
        run: ./scripts/install-validator.sh
      - name: Run validator-gated integration tests
        run: cargo test --workspace -- --ignored
```

- [ ] **Commit**

```bash
git add .github/workflows/rust.yml
git commit -m "ci: rust workflow (fmt, clippy, test, validator integration)"
```

---

### Task M2: Swift CI (macOS)

**Files:**
- Create: `.github/workflows/swift.yml`

```yaml
name: swift

on:
  pull_request:
    paths:
      - 'adapters/vrm-metal-kit/**'
      - '.github/workflows/swift.yml'
  push:
    branches: [main]

jobs:
  build:
    runs-on: macos-14
    defaults:
      run:
        working-directory: adapters/vrm-metal-kit
    steps:
      - uses: actions/checkout@v4
      - run: swift build --configuration debug
      - run: swift test
```

- [ ] **Commit**

```bash
git add .github/workflows/swift.yml
git commit -m "ci: swift workflow (macOS build + test for VRMMetalKit adapter)"
```

---

### Task M3: Site CI + GH Pages deploy

**Files:**
- Create: `.github/workflows/site.yml`

```yaml
name: site

on:
  pull_request:
    paths:
      - 'site/**'
      - 'goldens/manifest.json'
      - '.github/workflows/site.yml'
  push:
    branches: [main]
    paths:
      - 'site/**'
      - 'goldens/manifest.json'

permissions:
  contents: read
  pages: write
  id-token: write

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      - run: npm ci
        working-directory: site
      - run: npm run build
        working-directory: site
      - name: Stage manifest under site dist
        run: |
          mkdir -p site/dist/goldens
          cp goldens/manifest.json site/dist/goldens/manifest.json || echo "{}" > site/dist/goldens/manifest.json
      - uses: actions/upload-pages-artifact@v3
        with:
          path: site/dist

  deploy:
    if: github.ref == 'refs/heads/main'
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - id: deployment
        uses: actions/deploy-pages@v4
```

- [ ] **Commit**

```bash
git add .github/workflows/site.yml
git commit -m "ci: site build + GH Pages deploy on main"
```

---

### Task M4: Manifest validation gate

**Files:**
- Create: `.github/workflows/manifest-validate.yml`
- Create: `crates/vrm-s3/src/bin/validate-manifest.rs`

CI job that runs on PRs touching `goldens/manifest.json`. Verifies every entry has the full submission metadata, that BLAKE3 ref shape is valid, and that S3 URLs are well-formed. Does not re-render or re-download.

- [ ] **Step 1: Validator binary**

`crates/vrm-s3/src/bin/validate-manifest.rs`:

```rust
use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use clap::Parser;
use vrm_s3::manifest::Manifest;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "goldens/manifest.json")]
    manifest: Utf8PathBuf,
}

fn main() -> Result<()> {
    let a = Args::parse();
    if !a.manifest.exists() {
        eprintln!("manifest not present, treating as empty");
        return Ok(());
    }
    let raw = std::fs::read_to_string(&a.manifest)?;
    let m: Manifest = serde_json::from_str(&raw)?;

    let mut errors = Vec::new();
    for (i, e) in m.entries.iter().enumerate() {
        if !e.image_url.starts_with("s3://") {
            errors.push(format!("[{i}] image_url must start with s3://: {}", e.image_url));
        }
        if !e.image_blake3.starts_with("blake3:") {
            errors.push(format!("[{i}] image_blake3 must start with blake3:"));
        }
        for (name, val) in [
            ("os", &e.metadata.os),
            ("os_version", &e.metadata.os_version),
            ("gpu_vendor", &e.metadata.gpu_vendor),
            ("gpu_model", &e.metadata.gpu_model),
        ] {
            if val.trim().is_empty() {
                errors.push(format!("[{i}] metadata.{name} must be non-empty"));
            }
        }
        if e.git_hash.len() < 7 {
            errors.push(format!("[{i}] git_hash too short: {}", e.git_hash));
        }
    }

    if !errors.is_empty() {
        for e in &errors {
            eprintln!("{e}");
        }
        bail!("{} errors in manifest", errors.len());
    }
    eprintln!("manifest OK ({} entries)", m.entries.len());
    Ok(())
}
```

- [ ] **Step 2: Workflow**

```yaml
name: manifest-validate

on:
  pull_request:
    paths:
      - 'goldens/manifest.json'

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo run --release --bin validate-manifest -p vrm-s3
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/manifest-validate.yml crates/vrm-s3/
git commit -m "ci: manifest schema validator + CI gate for golden submission PRs"
```

---

## Section N — End-to-end smoke test

The v0.1 hello-world: generate one asset → render via VRMMetalKit → diff against itself → upload to S3 → display in site.

### Task N1: Wire the full pipeline locally

**Files:**
- Create: `scripts/smoke.sh`
- Create: `crates/vrm-diff-engine/examples/self_diff.rs`

Manual end-to-end smoke that exercises every component shipped in Phase 1. Not a CI test (depends on real S3 credentials and a built Swift adapter); it is the operator's "did Phase 1 actually land" check.

- [ ] **Step 1: Smoke script**

`scripts/smoke.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

# Phase 1 v0.1 hello-world end-to-end smoke. Requires:
#   - Validator shim installed (.tools/vrm-validator-cli)
#   - VRMMetalKit adapter built (adapters/vrm-metal-kit/.build/release/vrm-metal-kit-adapter)
#   - AWS credentials in env (or default profile) with VRM_GOLDENS_BUCKET set
#   - cargo, swift, node available

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

ASSETS=$ROOT/assets/generated
OUTPUTS=$ROOT/.smoke/renders
mkdir -p "$ASSETS" "$OUTPUTS"

echo "==> Generating asset"
cargo run --release -p vrm-asset-generator -- emit-default \
    --id smoke_default \
    --output-dir "$ASSETS"

echo "==> Building Swift adapter"
(cd adapters/vrm-metal-kit && swift build --configuration release)

ADAPTER=$ROOT/adapters/vrm-metal-kit/.build/release/vrm-metal-kit-adapter

echo "==> Running test plan"
cargo run --release -p vrm-runner -- execute-test-plan \
    --plan "$ASSETS/smoke_default.test.yaml" \
    --adapter-bin "$ADAPTER" \
    --asset-dir "$ASSETS" \
    --output-dir "$OUTPUTS" \
    --renderer-name vrm-metal-kit \
    --json

echo "==> Diff against self (sanity)"
PNG="$OUTPUTS/smoke_default_vrm-metal-kit.png"
[ -f "$PNG" ] || { echo "no render produced at $PNG"; exit 1; }
cargo run --release -p vrm-diff-engine --example self_diff -- "$PNG"

if [ -n "${VRM_GOLDENS_BUCKET:-}" ]; then
    echo "==> Uploading to S3"
    "$ROOT/scripts/push-goldens.sh" \
        "$PNG" smoke_default vrm-metal-kit 0.1.0
else
    echo "skipping S3 upload (set VRM_GOLDENS_BUCKET to enable)"
fi

echo "==> Building site"
(cd site && npm install && npm run build)

echo
echo "OK — smoke complete. Open site/dist/index.html in a browser to view."
```

- [ ] **Step 2: Self-diff example**

`crates/vrm-diff-engine/examples/self_diff.rs`:

```rust
use camino::Utf8PathBuf;

fn main() -> anyhow::Result<()> {
    let p = std::env::args()
        .nth(1)
        .map(Utf8PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("usage: self_diff <png>"))?;
    let score = vrm_diff_engine::ssim::ssim_pngs(&p, &p)?;
    println!("self-SSIM = {score}");
    Ok(())
}
```

Add to `crates/vrm-diff-engine/Cargo.toml`:

```toml
[[example]]
name = "self_diff"
path = "examples/self_diff.rs"

[dev-dependencies]
camino.workspace = true
anyhow.workspace = true
```

- [ ] **Step 3: Make the script executable + run**

```bash
chmod +x scripts/smoke.sh
./scripts/smoke.sh
```

The first end-to-end run is the v0.1 acceptance gate. Any failure points at a specific section to fix; iterate until green.

- [ ] **Step 4: Commit**

```bash
git add scripts/smoke.sh crates/vrm-diff-engine/
git commit -m "chore: end-to-end Phase 1 v0.1 hello-world smoke script"
```

---

## Self-Review (Plan v0.2 — full Phase 1)

**Spec coverage check (handover document):**

| § | Topic | Plan section |
|---|---|---|
| 3 | Repository layout | RFC-0001, README (A1, A2) |
| 4 | Architecture diagram | All sections |
| 5.1 | Asset corpus matrix | F1, F2a-d, F3, F4, G |
| 5.2 | render-fidelity site | K, J |
| 5.3 | Runner + tool surface | A6, E, I |
| 6 | Renderer adapters | L (VRMMetalKit only in Phase 1) |
| 7 | Methodology hazards | A5 |
| 8 | mrxz/vrm-validator | C, F2d, G3 |
| 9 | Khronos compatibility | A1, A2 |
| 11 #1 | Polyrepo question | RFC-0001 |
| 11 #7 | Anti-fraud | RFC-0002 |
| 13 | Definition of Done | distributed; N is the integration gate |
| 14 | First-week tasks | A, B, C, F1 (already shipped) |

**Placeholder scan:**

- F2d Step 6 carries an explicit "iterate until validator returns 0 errors" caveat — the validator's exact requirements may shift; the engineer adapts the JSON until validation passes.
- L1–L3 are at contract level (acceptance criteria) rather than line-by-line code, because VRMMetalKit's Swift API is unknown at plan-writing time.
- J2 has a `chrono_now_iso()` placeholder; flagged inline (swap to `time` or `chrono` crate).

**Type consistency (verified):**

- `vrm-test-plan::ColorSpace` ↔ `vrm-ops::tools::ColorSpace` — bridged in `vrm-runner::plan_to_ops`.
- `vrm-test-plan::ToneMapping` ↔ `vrm-ops::tools::ToneMapping` — same.
- `vrm-test-plan::BboxRegion` and `vrm-diff-engine::property::BboxRegion` are deliberately separate copies because the diff engine doesn't depend on the test-plan crate. Extract a shared types crate only if the duplication starts to hurt.

**YAGNI check:**

- ✅ One renderer (VRMMetalKit). No three-vrm/godot/UniVRM/Babylon adapters.
- ✅ MToon material tests only. Spring bones, constraints, expressions, animation deferred.
- ✅ Side-by-side viewer is mandatory; slider/diff (K2) is "ship if cheap, defer otherwise."
- ✅ Reserved ops are stubbed Unimplemented, not implemented.
- ✅ No HDRI, no debug-pass outputs, no consensus-mode diff.

**Risk register:**

- F2d: VRMC_vrm validation may reject our minimal humanoid stub; iterate.
- J2: AWS SDK auth assumes default credential chain; document `VRM_GOLDENS_BUCKET` and `AWS_*` env vars in CONTRIBUTING.
- L1: VRMMetalKit upstream package vending — if the import name doesn't match `VRMMetalKit`, adjust.
- M3: GH Pages base path — if deploying under a project sub-path, set `base` in `vite.config.ts` to the repo name.

---

## Execution Handoff (Plan v0.2)

Plan v0.2 is complete. Sections F2 through N are spec'd at the same task-and-step granularity as A through F1.

**Two execution options:**

1. **Subagent-Driven (best for breadth).** Dispatch a fresh subagent per section (F2, F3, F4, G, H, I, J, K, L, M, N), review between, parallelize where independent. Use `superpowers:subagent-driven-development`.

2. **Inline Execution (best for fastest first end-to-end).** Continue tasks in this session, top to bottom. Use `superpowers:executing-plans`. Section N1 is the acceptance gate.

**Critical path to v0.1 hello-world:** F2 → F3 → F4 → G1 → H1 → I → J1+J2 → L → N. Sections G2-G3, K, M, J3 can land in parallel later without blocking the first end-to-end render.








