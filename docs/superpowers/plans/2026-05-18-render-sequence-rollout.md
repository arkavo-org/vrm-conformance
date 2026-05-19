# `render_sequence` — Phased Rollout Plan

> **For agentic workers:** This is a META-PLAN. Each numbered phase below will be expanded into its own task-by-task plan file (`2026-05-18-render-sequence-phaseN-<topic>.md`) once RFC-0004 is Accepted. Do not start implementation from this file alone.

**Goal:** Land RFC-0004 (`render_sequence` op + temporal diff + sequence manifest entries) end-to-end across all four real adapters plus the site, in 8 incremental phases that each ship behind back-compat defaults and never block existing single-frame tests.

**Spec:** [`rfcs/0004-render-sequence-op.md`](../../../rfcs/0004-render-sequence-op.md)

**Architecture (one-line per layer):**
- **Op surface** in `crates/vrm-ops` (Phase 1): types + JSON Schema + Unimplemented stubs everywhere.
- **Diff + manifest + runner** in `crates/vrm-diff-engine`, `crates/vrm-s3`, `crates/vrm-runner` (Phase 2): consume sequences end-to-end against goldens, with `validate-manifest` learning the new `kind`.
- **Mock renderer** in `crates/vrm-mock-renderer` (Phase 3): deterministic reference so the diff engine and runner can be E2E-tested without GPU.
- **Asset generator + test plan schema** in `crates/vrm-asset-generator`, `crates/vrm-test-plan` (Phase 4): sequence-capable plans + `emit-sequence-sweep` subcommand.
- **First real adapter** (Phase 5): `adapters/vrm-metal-kit` — closes the swing-sweep information-loss gap.
- **Remaining EditMode-friendly adapters** (Phase 6): `adapters/three-vrm`, `adapters/godot-vrm`.
- **UniVRM PlayMode** (Phase 7): bundled with the existing L4-PlayMode follow-up (FastSpringBone already requires PlayMode; sequence loop is the only new code).
- **Site** (Phase 8): `site/` gets a frame scrubber + worst-frame highlight + SSIM heatmap.

---

## Phase ordering rationale

The phases are ordered so each one is **independently mergeable** and **independently revertable**:

- Phase 1 is pure additions to the op contract; nothing breaks if Phase 2 never lands.
- Phase 2 reads sequences but only operates on tests that opt in via the new `render_sequence:` plan block; existing single-frame tests are untouched.
- Phase 3 (mock renderer) is the reference implementation the diff engine tests against. Lands before any real adapter to give the diff engine a deterministic baseline.
- Phase 4 (asset generator) gates **when** sequence-capable test plans start being emitted. Until Phase 4 lands, the new op + diff machinery have no production consumer.
- Phase 5 picks **vrm-metal-kit** as the first real adapter for two reasons: (a) it's the adapter with the biggest payoff (swing-sweep currently shows VMK's largest cluster of divergence with the consortium reference per `docs/findings.md`), (b) Metal offscreen rendering is the easiest path to a per-frame loop (no Playwright, no headless flag, no PlayMode lifecycle).
- Phase 6 follows because three-vrm and godot-vrm both **already step physics per frame** in their render loop; the change is minimal.
- Phase 7 is bundled with the deferred L4-PlayMode work for UniVRM because the two share infrastructure (FastSpringBone, `PhysicsDriver.Process(dt)`, `EditorApplication.EnterPlaymode()`).
- Phase 8 (site) can ship anytime after Phase 5, but lands last so the UI work has real sequences from at least one renderer to display.

If any phase reveals a design flaw, the RFC gets a Superseded-by amendment and downstream phases adjust. The non-breaking-defaults posture means this never blocks an active release.

---

## Phase 1 — Op surface + Unimplemented stubs

**Mirrors:** `docs/superpowers/plans/2026-05-17-vrma-phase1-op-surface.md` exactly. This is a well-trodden pattern.

**Scope:**
- `crates/vrm-ops/src/tools.rs`: `RenderSequenceParams`, `RenderSequenceResult`, `SequenceFrame`, `SequenceFormat`, `RootTransformAnimation`, `VrmaPlaybackSpec` types with serde round-trip tests.
- Every adapter declares `render_sequence` in its reserved-phase map returning `-32000 Unimplemented` with `data: { phase: "v1.x-sequence" }`.
- `describe --format json` exposes the new op across every binary.
- `docs/operation-contract.md` documents the op alongside `render`.
- `docs/methodology.md` adds the sequence-specific pins (60 Hz physics floor, no temporal alignment, etc.).

**Done when:** all adapters return Unimplemented for `render_sequence`, serde round-trips pass, describe catalog exposes it, fmt+clippy+test workspace green.

**Estimated:** ~12 tasks, 1 sitting. Mostly mechanical.

---

## Phase 2 — Diff engine + manifest schema + runner integration

**Scope:**
- `crates/vrm-diff-engine`: new `temporal_diff` module. Per-frame SSIM, mean/p95/min aggregation, worst-frame tracking, BLAKE3 short-circuit. Unit tests against synthetic frame pairs (identical, single-bad-frame, gradual drift, length mismatch).
- `crates/vrm-test-plan`: optional `render_sequence:` block in `TestPlan`. Validator rejects plans with both `render:` and `render_sequence:` set.
- `crates/vrm-runner/src/execute.rs` + `plan_to_ops.rs`: dispatch `render_sequence` when the plan declares it. Surface `TemporalDiffResult` in the runner's JSON output.
- `crates/vrm-s3`: `ManifestEntry` gains `kind: ImageEntry | SequenceEntry` discriminator (default `image` for back-compat). `validate-manifest` checks frame_count consistency, BLAKE3 well-formedness, and S3 presence for every frame URL.
- New `vrm-runner consensus-diff` mode: per-frame pairwise SSIM across N renderers.

**Done when:**
- Synthetic temporal_diff unit tests cover the four aggregation modes.
- A handcrafted `render_sequence:`-style test plan can drive the runner end-to-end against the existing mock renderer (which will Unimplemented-fail at this phase — that's expected; the runner just needs to handle the error envelope cleanly).
- `validate-manifest` accepts both image and sequence kinds.
- CI guards green (no manifest regressions on existing entries).

**Estimated:** ~18 tasks.

---

## Phase 3 — Mock renderer reference implementation

**Scope:**
- `crates/vrm-mock-renderer`: implement `render_sequence` deterministically. Each frame's pixel content is a function of `(frame_index, frame_hz, physics_dt_seconds, animation_state)` — same inputs ⇒ byte-identical PNGs ⇒ identity short-circuit makes self-diff 1.0 by construction.
- Mock honors `output_format`. PNG sequence is real; MP4/MOV mux uses `ffmpeg` shell-out (skipped at runtime with a clear log line if ffmpeg absent — falls back to PngSequence).
- Integration test: drive the runner against the mock with a sequence plan, assert per-frame BLAKE3 match across two runs.
- Smoke script `scripts/smoke.sh` gains a `--sequence` mode exercising the new pipeline.

**Done when:** self-diff against the mock returns mean_ssim=1.0, min_ssim=1.0, identity_match=true for every frame.

**Estimated:** ~10 tasks. The mock's existing single-frame render path provides most of the infrastructure.

---

## Phase 4 — Asset generator + sequence-capable test plans

**Scope:**
- `crates/vrm-asset-generator`: new `emit-sequence-sweep` subcommand. Initial corpus: the 18 swing-sweep variants regenerated as sequence plans (60 frames @ 30 Hz, root-transform animation preserved from the existing single-frame plans). Existing single-frame plans for the same variants stay until Phase 5 deletes them, so there's no flag-day.
- New paired-triplet convention: `<asset>.vrm` + `<asset>.meta.json` + `<asset>.test.yaml` with `render_sequence:` block. The schema-derived golden methodology already enforces single-source-of-truth between asset and plan.
- `docs/methodology.md` gains a "Sequence captures" section documenting the swing-sweep migration rationale.

**Done when:** `cargo run -p vrm-asset-generator -- emit-sequence-sweep --output-dir /tmp/seq-sweep` produces 18 triplets that pass the plan schema validator AND can be consumed by the runner driving the mock renderer (Phase 3).

**Estimated:** ~8 tasks.

---

## Phase 5 — First real adapter: vrm-metal-kit

**Scope:**
- `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`: implement `render_sequence`. Frame loop wraps the existing Metal offscreen render pass. `springBoneSystem.update(dt)` between captures; `vrmAnimationInstance.sample(t)` if `apply_vrma` is set.
- Per-frame PNG via existing `MTLTexture → PNG` path; muxed format support gated on `ffmpeg` availability (shell-out from the adapter is acceptable — RFC-0003 permits engine-idiom-fit, and ffmpeg-as-subprocess is idiomatic for offscreen pipelines).
- VRMMetalKit dependency bump if any upstream API is missing (file a VMK issue first per the existing pin-bumping protocol in `Package.swift`).
- Promote `render_sequence` out of the reservedPhases map.
- `scripts/bootstrap-goldens.sh` learns the sequence path: per-frame PNG push to S3 + manifest entry kind=sequence.

**Done when:**
- 18 swing-sweep sequence tests render through vrm-metal-kit end-to-end.
- Manifest entries for those tests use `kind: sequence`.
- Consensus-diff against mock renderer produces a real (non-Unimplemented) temporal_diff result.
- `docs/findings.md` gets an entry comparing post-Phase-5 swing-sweep numbers against the previous single-frame pass-rate. Expected: the headline number will *change*, and that change is the actual signal we've been missing.

**Estimated:** ~14 tasks.

---

## Phase 6 — three-vrm + godot-vrm

**Scope (parallel tracks):**

### 6a — three-vrm
- `adapters/three-vrm/src/operations.ts`: frame loop wrapping existing render/screenshot path. `vrmManager.update(dt)` between captures (already exists at L4).
- Playwright `page.screenshot` per frame; PNG sequence canonical, MP4 mux via `ffmpeg-static` (already in `package.json` for VRMA work).
- Promote `render_sequence` out of the Unimplemented set.

### 6b — godot-vrm
- `crates/vrm-godot-shim/src/bridge.rs`: extend the dispatch to forward `render_sequence` to GDScript.
- `adapters/godot-vrm/scripts/conformance.gd` (or equivalent): frame loop with manual `VRMSecondary.do_process(dt)` between captures. The L4 work already disabled VRMSecondary's auto-stepping so this is the natural extension.
- `ImageTexture.get_image().save_png()` per frame; `ffmpeg` shell-out for mux.

**Done when:**
- Both adapters produce sequence goldens for the 18 swing tests.
- Three-way consensus-diff (VMK + three-vrm + godot-vrm) runs across the sequence corpus.
- The consensus report flags real divergence (not Unimplemented placeholders).

**Estimated:** ~10 tasks each track; can run in parallel sessions per `superpowers:dispatching-parallel-agents`.

---

## Phase 7 — UniVRM PlayMode (bundled with L4-PlayMode follow-up)

**Scope:**
- This phase is **co-scoped with the deferred L4-PlayMode plan** referenced in `docs/findings.md` (the spring-bone stepping work that FastSpringBone gates on `Application.isPlaying`).
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/BatchRunner.cs`: PlayMode batch entry point via `EditorApplication.EnterPlaymode()`. The scaffolding is already in `PhysicsDriver.cs` per `docs/findings.md`.
- `Capture.cs`: extend single-frame capture to a frame loop driven by `WaitForEndOfFrame`.
- Methodology check: Unity coroutine timing under `-batchmode` may not deliver exact 1/60 s — Open Question #2 in RFC-0004. Resolution path: drive the loop via `EditorApplication.update` or a tighter physics callback if `WaitForEndOfFrame` jitter is observable. Profile during this phase.

**Done when:**
- The 18 swing-sweep tests render through UniVRM PlayMode batch end-to-end.
- Four-way consensus-diff is real (no rest-pose placeholders for UniVRM).
- `docs/findings.md` headline number for swing-sweep conformance gets a third honest revision — the L3 → L4-PlayMode drop is the same kind of correction the original 90% → 68% revision documented; the sequence corpus then gives us the truthful number for the first time.

**Estimated:** ~16 tasks (PlayMode batch lifecycle is non-trivial; bulk of cost is the L4-PlayMode portion not the sequence portion).

---

## Phase 8 — Site

**Scope:**
- `site/src/`: new sequence viewer route. Frame scrubber timeline with hover-preview, worst-frame highlight from `TemporalDiffResult.worst_frame_index`, SSIM heatmap (per-frame SSIM as a color strip beneath the scrubber).
- Lazy frame fetching: only the visible frame + adjacent frames load from S3; full-sequence loading is opt-in for power users.
- Manifest reader extension: site detects `kind: sequence` and routes to the new viewer.
- Existing single-frame goldens render unchanged.

**Open question to resolve in this phase (RFC-0004 OQ #3):** default scrubber behavior — auto-play vs worst-frame freeze. Lean toward worst-frame freeze.

**Done when:**
- Site builds + deploys via existing `site.yml`.
- A sequence golden can be reviewed end-to-end (open test → scrub → land on worst frame → see per-frame SSIM).
- Khronos glTF-Render-Fidelity comparison: visual parity with their sequence presentation patterns (the donation-readiness check).

**Estimated:** ~12 tasks.

---

## Phase-zero gate

Before Phase 1 starts: **RFC-0004 must be Accepted.** That means at minimum a review pass by the project owner, a check that Open Questions 1–4 either have resolution paths or are explicitly deferred to a named phase, and a back-of-envelope storage cost check (RFC OQ #1 — confirm ~1 GB per snapshot is sustainable before Phase 5 commits us to it).

If the RFC is Accepted with amendments, this plan gets a corresponding revision before the first phase plan is written.

---

## Cross-phase guardrails

- **Back-compat**: every phase MUST land without breaking existing single-frame tests. CI runs the existing corpus through Phases 1–4 with zero changes to the pass-rate; the swing-sweep migration in Phase 5 is the *one* place where numbers move, and that movement is the explicit goal documented in `docs/findings.md`.
- **No flag-day**: existing `render:` plans coexist with new `render_sequence:` plans until Phase 5 deliberately migrates the swing-sweep corpus. Manual plans in `test-plans/manual/` may opt in earlier for testing.
- **fmt + clippy zero-warning** at every commit boundary. This is the project's hard merge gate.
- **Methodology pins** are checked at each phase boundary — Phase 4 adds the sequence pin to `docs/methodology.md`; later phases must respect it.
- **`docs/findings.md` updates** are required in Phases 5 and 7 (the two phases where conformance numbers will visibly change). Findings entries follow the existing template — what was observed, why it moved, what closure path exists.

---

## What's NOT in scope

- **Audio.** VRMA doesn't carry audio; sequences are silent.
- **Variable frame-rate sequences.** Sequences are fixed `frame_hz`. If a future test needs VFR, it gets a new op.
- **Perceptual video metrics (VMAF etc).** Rejected in RFC-0004 alternatives. Per-frame SSIM with worst-frame tracking is the contract.
- **Live preview mode.** No real-time playback in the adapter; render-to-disk only.
- **Frame-by-frame golden review tooling.** Phase 8 site is the review surface; a separate CLI tool is out of scope for this rollout.
