# VMK 0.17.2 expression/morph (#333) conformance coverage — design

**Date:** 2026-06-07
**Status:** Approved (brainstorming)
**Driver:** VMK pin bump 0.17.1 → 0.17.2, a behaviour change that restores VRM 1.0 facial expressions. The suite must observe blink/visemes/emotions actually deforming the mesh, not just carry a blind pin bump.

## Background

VMK 0.17.2 (`3737e76`, tag `0.17.2`; closes upstream #333) fixes VRM 1.0 facial
expressions. The bug: a VRM 1.0 expression `morphTargetBind.node` is a glTF **node**
index, but the renderer and `VRMExpressionController` key morph weights by **mesh**
index (VRM 0.x binds already carry the mesh index). The 1.0 loader stored the raw
node index, so on any model whose face node index ≠ its mesh index, **every** morph
bind matched no primitive and the morph compute pass skipped it — blink, the five
visemes, and every emotion preset silently produced **no mesh deformation** on VRM 1.0
avatars. Bone-driven look-at was unaffected (different code path), which is why only
*expressions* looked dead, and VRM 0.x deployments never hit it. The loader now
resolves `node → nodes[node].mesh` into a resolved `meshIndex` while preserving the
authored `node` for round-trip.

The release notes name the repro: `vroid_default_F_1_0` blink bind **node=211 → mesh 0**;
`AvatarSample_A` 1.0 **node=91 → mesh 0** (also broken, hidden by its white-sclera eyes).

**Empirically confirmed for this suite (2026-06-07):**
- `vroid_default_F_1_0` binds blink/happy/sad/aa/surprised all at **node 211 → mesh 0**
  (node ≠ mesh) — the canonical #333 surface, across the full preset set.
- The **synthetic** humanoid avatar puts its mesh at **node 19 → mesh 0**: its visemes
  (aa/ih/ou/ee/oh) are bound at node 19 ≠ mesh 0, so #333 froze them too — and its
  **blink/happy/sad/emotion presets carry no morph binds at all** (empty
  `morphTargetBinds`). So the named cases (blink/happy/sad) have **zero synthetic
  coverage**, and the synthetic visemes were silently frozen.

This is the same blind-spot pattern as the lookAt work (see the 2026-06-07 "VMK 0.17.1
eye look-at #332" finding): synthetic assets don't exhibit the bug, the real VRoid
avatar does.

## Goal

Make VRM 1.0 expression deformation observable in conformance — blink, visemes, and
emotion presets — so the 0.17.1 → 0.17.2 change is a verifiable frozen→deforming
before/after, using the real `vroid_default_F_1_0` avatar.

## Decisions (from brainstorming)

- **Expression set: full preset sweep (all 11)** — blink + happy/angry/sad/relaxed/
  surprised + the 5 visemes (aa/ih/ou/ee/oh). Address the whole gap; every preset
  morph bind is exercised. Reuses the existing avatar-agnostic VRMA expression clips.
- **Vehicle: real VRoid avatar (primary)** — the only avatar carrying blink/happy/sad
  morphs, all bound node ≠ mesh. Custom expressions (smug/drowsy) out of scope — #333
  is about preset morph binds.
- **Synthetic: verify visemes only** — confirm the synthetic visemes (node 19 ≠ mesh 0)
  go frozen → deforming across the bump and stop silently passing on frozen output.
  No synthetic blink/happy/sad morph authoring (deferred follow-up; real avatar covers).
- **Whole-face camera** for all 11 plans (blink shows in the eyes, visemes/emotions in
  the mouth/brow) — one framing, not per-region.

## Components

### 1. VMK pin bump

`adapters/vrm-metal-kit/Package.swift`: revision → `3737e76b1635f9be604e4a8cb4272b5ddbedb58d`
(tag `0.17.2`). Prepend a `0.17.2` changelog comment block above the `0.17.1` entry, per
the file's convention: record the behaviour change (VRM 1.0 morph binds re-keyed
node→mesh; blink/visemes/emotions restored on 1.0 avatars where node ≠ mesh), no shader/
metallib change, and that this suite's new `vroid_default_F_expr_*` corpus is the verifier.

### 2. Expression VRMA clips

A new `emit-expression-clips` subcommand (parallel to `emit-gaze-sweep`) emitting
**only** `{id}.vrma` — the avatar is the real fixture and plans are committed manual YAML.
Reuses `crate::vrma_emit::add_expression_weight_channel` (preset weight ramp
0 → 1 → 0 as `node.translation.x`) and registers the canonical skeleton
(`register_all_humanoid_bones`, UniVRM importer invariant). 11 clips named
`expr_<name>.vrma`: blink, happy, angry, sad, relaxed, surprised, aa, ih, ou, ee, oh.
Implemented via a new `emit_expression_clip(output_dir, &VrmaExpressionParams)` and an
`expression_clip_sweep()` returning the 11 preset params (reusing the existing
`VrmaExpressionParams` type; `is_preset = true`, `duration_s = 1.0`). Wired into the CLI
`Cmd` + `describe` catalog.

### 3. Manual test plans

`test-plans/manual/humanoid/vroid_default_F_expr_<name>.test.yaml`, 11 plans pairing
`asset: vroid_default_F_1_0.vrm` with `animation.vrma.path: expr_<name>.vrma`,
`apply_at_time: 0.5` (the weight peak of the 0→1→0 ramp over duration 1.0).

- **Whole-face camera** — frames eyes + nose + mouth so any preset's deformation lands
  in-frame (eyes ≈ y 1.30, mouth lower). Nominal: target `[0, 1.27, 0.02]`, position
  `[0, 1.27, 0.55]`, fov 24 — tuned against the avatar at bootstrap.
- Methodology pins: `tone_mapping: none`, `cast_shadows: false`, `receive_shadows: false`.
- `output` 1024², srgb, msaa 4. `diff`: ssim, reference_renderer `three-vrm` (deforms
  correctly), `pose_tolerance` with `per_preset_expression`, `conformance_status: included`.

### 4. Synthetic viseme verification

The existing `vrma_expression_preset_{aa,ih,ou,ee,oh}` corpus renders the visemes on the
synthetic avatar (node 19 ≠ mesh 0), so #333 froze them. Verify across the bump: render a
synthetic viseme (e.g. `aa`) through 0.17.1 and 0.17.2 and confirm **frozen → deforming**
(the viseme render must differ from a neutral synthetic render; on 0.17.1 they are
near-identical). Record in findings. If the diff engine supports a "differs from neutral"
deformation guard cheaply, add it to the synthetic viseme plans; otherwise note as a
follow-up so the corpus stops silently passing on frozen output.

### 5. Verification + findings

`dump_expression_weights` reports only that the *weight* was applied (controller state,
upstream of the #333 keying), so it cannot catch a frozen mesh — the **rendered face is
the signal** (same as gaze). Primary signal: each expression render **differs from a
neutral render** of the same avatar (0.17.1 frozen ≈ neutral → caught; 0.17.2 deformed).
Cross-renderer: three-vrm reference (deforms correctly). Run the real before/after locally
(macOS 26): 0.17.1 frozen-face → 0.17.2 deforming blink/happy/sad/visemes; log it in
`docs/findings.md`.

## Scope boundaries (YAGNI)

- **No synthetic blink/happy/sad morph authoring** — deferred follow-up.
- **No new emitter primitives** — reuse `VrmaExpressionParams` + `add_expression_weight_channel`.
- **Custom expressions out of scope** — #333 is preset morph binds; the 11 presets are the gap.

## Risks / verify during implementation

- **Apply path.** The adapter's `handleApplyVrmaAtTime` must drive the expression weight
  through the controller's preset path and trigger the morph compute before the render.
  If an expression render is byte-identical to neutral on 0.17.2 too, inspect the apply
  ordering — the asset (real avatar, authored morphs) is known-good.
- **Clip ↔ plan naming.** Plan `animation.vrma.path` (`expr_blink.vrma`) must match the
  clip id emitted by `expression_clip_sweep()` exactly.
- **Rendering is local-only** — VMK needs macOS 26 / Xcode 26; CI build-validates only.
  Expression re-bootstrap and the 0.17.1/0.17.2 before/after run on an M-series Mac.

## Out-of-band follow-ups (tracked, not this change)

- Synthetic generator: author blink/happy/sad morph targets for a fully parametric
  expression sweep on synthetic avatars.
- A first-class "differs from neutral" deformation assertion in the diff engine, so any
  morph-bearing plan fails loudly on frozen output rather than relying on a reference PNG.
