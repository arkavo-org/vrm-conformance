# Backlog

Work surfaced by the conformance suite that isn't tracked as a discrete GitHub issue. For concrete, smaller tasks see [the GH issue list](https://github.com/arkavo-org/vrm-conformance/issues). For per-finding history see [`docs/findings.md`](findings.md); for cross-renderer methodology decisions see [`docs/methodology.md`](methodology.md).

This file is the holding pen for: RFC-level work, methodology questions still open, and dependencies waiting on upstream actions. Items move from here to either GH issues (once they're concrete enough to act on) or to RFCs (under `rfcs/`).

## RFC-level work

### Camera-mode op for first-person rendering

**Context.** [Gap #3 closure](https://github.com/arkavo-org/vrm-conformance/commit/fba4a69) added a 4-variant `firstPerson` sweep covering `meshAnnotations[*].type` (auto / both / thirdPersonOnly / firstPersonOnly). The sweep only exercises the **third-person rendering path** because the runner's `set_camera` op has no concept of camera mode — the asset declares which meshes are first-vs-third, but the camera always renders as third-person.

The reverse case (first-person camera, where `thirdPersonOnly` should cull and `firstPersonOnly` should be visible) needs an explicit camera-mode field on `set_camera`. Either:
- Add `first_person: bool` (default false) to `SetCameraParams` in `crates/vrm-ops/src/tools.rs`, and wire each adapter to switch culling masks based on the flag.
- Or detect from camera position relative to the head bone (brittle; doesn't allow first-person from arbitrary positions).

The change requires a small RFC (op contract addition) + 4 adapter updates. After it lands, the firstPerson sweep doubles to 8 variants (4 types × 2 camera modes) and the conformance check becomes "expected visible / culled" per quadrant.

### `VRMC_node_constraint` coverage

**Context.** [Original gap analysis](https://github.com/arkavo-org/vrm-conformance/issues?q=node_constraint) flagged this as gap #2 — zero conformance coverage for VRMC_node_constraint-1.0 (aim / roll / rotation constraints between bones).

Needs:
1. New ops in the runner contract — currently no op drives the source bone of a constraint. Options:
   - Promote `set_humanoid_pose` from `Unimplemented` (currently reserved across all adapters) — would let test plans drive arbitrary source-bone poses and observe the constraint propagation to the constrained bone.
   - Or new dedicated `step_constraints` op that takes a per-bone pose dict and returns the post-constraint pose.
2. Asset-generator additions for constraint scenes (2-node rigs with aim/roll/rotation constraints).
3. New `dump_node_pose` or extension to `dump_humanoid_pose` to expose constrained bone outputs.

RFC-level — touches ops contract + adapter implementations + asset generator + dump path. Multi-session.

### UV-animation sweep needs a clock contract

**Context.** MToon's `uvAnimationScrollX/YSpeedFactor` and `uvAnimationRotationSpeedFactor` are time-dependent — rendering at time T applies a UV shift of `(speed * T) mod 1`. The current `render` op renders a single frame at an unspecified time; without a "capture time" parameter, all UV-animation sweep variants would render at t=0 (the same frame) regardless of speed values.

Two paths:
- Add `capture_time_seconds: f32` to `RenderParams`. Adapters that ignore it render at their current default (t=0). Adapters that honour it advance the clock by exactly that delta before rendering.
- Use `render_sequence` (which already has start_seconds + frame_hz) for UV-animation tests. Lossy in conformance value (single-frame sweep is more focused) but doesn't need a new op.

The first option is cleaner but RFC-level (op contract addition).

### `apply_vrma` in `render_sequence` (Phase 5/6 deferral)

**Context.** RFC-0004 deferred `apply_vrma` + MP4/MOV mux on `render_sequence` to a follow-up. Currently the runner rejects `apply_vrma:` in a `render_sequence:` block (`crates/vrm-runner/src/execute.rs::run` validation). Adapters return `Unimplemented` for the apply path even when they implement render_sequence.

After this lands, the VRMA sweep can be rendered via render_sequence to capture motion across the animation duration — significantly richer signal than the current single-frame `apply_at_time` approach.

Adapter-side wiring task; not RFC-level (RFC-0004 already covers the design). Touch points: each of the 4 real adapters' `handleRenderSequence` (Swift, TS, GDScript, C#).

## Methodology questions still open

### `outlineColorFactor` × `outlineLightingMixFactor` sweep

**Context.** The current MToon outline sweep (`mtoon_basic_sweep`'s outline section in `sweep.rs`) varies `outline_width_mode` × `outline_width_factor` × `outline_color_factor` partially. Neither `outline_color_factor` (varying outline colour) nor `outline_lighting_mix_factor` (mixing scene lighting into outline colour) is independently swept.

Probably 2-3 small sweeps (cross black / red / coloured outlines × lighting-mix 0/0.5/1). Should land as another `mtoon_outline_*` sweep family. Concrete + small — could promote to a GH issue.

### Schema-conformance probe

**Context.** The generator emits glTF + VRM JSON; validators in `crates/vrm-validator-wrap/` and `crates/vrm-s3/` check manifest schema, but nothing validates emitted asset JSON against the upstream JSON Schemas under `docs/upstream-specs/.../schema/`.

A probe would:
- Walk `goldens-cache/_assets*/*.vrm`
- Extract JSON chunk
- Validate against the matching `VRMC_*.schema.json` files
- Catches generator drift early (e.g., would've caught the missing-`expressions.custom` registration that was the [emissive `smug` finding](findings.md#custom-expression-caveat) before it shipped)

Quality-of-life tooling, not conformance signal per se. Could be a CI step.

### Emissive multiplier methodology — does UniVRM resolve VMK#287?

**Context.** [VMK#287](https://github.com/arkavo-org/VRMMetalKit/issues/287) was filed against VMK's `emissiveMultiplier` no-op behaviour. The methodology lesson from [the PBR-textures sweep](findings.md#glTF-core-PBR-textures-on-MToon) says: confirm with UniVRM before treating it as a real upstream issue. If UniVRM ALSO ignores `emissiveMultiplier` on MToon, this is a [methodology entry](methodology.md), not a bug.

Tracked in issue #14 (re-validate sweeps through UniVRM). If UniVRM honours emissive multiplier, VMK#287 stays; if not, it's reclassified.

Same question for VMK#288 (`KHR_texture_transform` on baseColorTexture) and VMK#289 (`outlineWidthMultiplyTexture` degraded pipeline). All three need UniVRM data before they can be treated as authoritative upstream bugs.

### VRM 0.0 legacy corpus — keep or skip?

**Context.** [Original gap analysis](https://github.com/arkavo-org/vrm-conformance/issues?q=vrm+0.0) noted zero conformance coverage for the legacy VRM 0.0 format. The spec tree at `docs/upstream-specs/vrm-specification/specification/0.0/` exists; we could generate 0.0 assets if it's a project goal.

Project-level decision: is VRM 0.0 in scope, or are we VRM 1.0-only? Today every adapter targets VRM 1.0; the synthetic corpus emits 1.0; the validator targets 1.0. Adding 0.0 is a deliberate scope expansion.

Recommend: defer until a real consumer asks. The 1.0 surface alone has plenty of work remaining (see other items in this file). If/when added, it'd be a parallel `_assets_v0_*` corpus tree.

## Upstream-blocked items

### VMK rc.3 verification

Once VMK ships 0.16.0-rc.3 with #283 (animated swing non-determinism) + #286 (lookAt rotation channel) + ideally #287/#288/#289/#290 fixes:
- Bump the VMK pin in `adapters/vrm-metal-kit/Package.swift`.
- Re-run the determinism reproducer (5+ runs of `swing_springbone_joints_16` — should all hash byte-identical).
- Re-run the PBR sweep — confirm `mtoon_pbrtex_normal_scale_2x` differs from `_default` on VMK (proves #290 fix).
- Re-run the texture-transform sweep — confirm 8 distinct hashes on VMK (proves #288 fix).
- Re-run the outline-multiply sweep — confirm 3 distinct textured-variant hashes on VMK (proves #289 fix).
- Re-run the emissive sweep — if #287 lands, confirm 7 distinct multiplier hashes; if reclassified as methodology, document.
- Re-run the lookat sweep with pose-level diff — confirm yaw/pitch dumps are non-zero on VMK (proves #286 fix).
- Update `docs/findings.md` with a "VMK 0.16.0-rc.3 verification" entry.

### three-vrm follow-ups

Three-vrm's extended-collider load failure (72 sweep failures in today's peer bootstrap; `spring-bone-extended` corpus rejected at the plugin level). Worth filing upstream once a clean reproducer is isolated from the broader spring-bone story. Currently absorbed under the spring-bone gap in [issue #11](https://github.com/arkavo-org/vrm-conformance/issues/11)-adjacent territory.

### `dump_humanoid_pose` / `dump_look_at_state` on UniVRM batch path

[Issue #6](https://github.com/arkavo-org/vrm-conformance/issues/6) reports UniVRM's batch path applies the head bone but leaves limb bones at identity for VRMA-driven poses. Open since before today's work. Worth verifying whether today's VRMA wiring on the VMK adapter shifted the picture (probably not — different adapter).

## Quality-of-life tooling

### Pose-level diff in `consensus-report.sh`

**Context.** Mentioned in [the VMK#286 findings entry](findings.md#vmk-lookat-rotation-channel-gap-new-upstream-finding-surfaces-in-pose-dump-but-not-in-ssim) as a future layer. Currently consensus is image-SSIM-only; the pose.json dumps (`dump_humanoid_pose` / `dump_expression_weights` / `dump_look_at_state`) aren't compared cross-renderer. Adding this layer would:
- Convert today's `vrma_lookat_*` tests from "passes consensus because gaze barely shifts pixels at 1024²" to actual yaw/pitch tolerance comparison.
- Surface VMK#286 (lookAt rotation channel) as a hard conformance failure instead of a quiet correctness gap.
- Make VMK#290 (`normalTexture.scale` ignored) and similar partial-broken findings catchable from pose data even when image SSIM passes.

Mid-size lift: new tolerance schema in `crates/vrm-test-plan/`, new comparator in `crates/vrm-diff-engine/`, integration in `scripts/consensus-report.sh`. Could promote to a GH issue when prioritised.

### MP4/MOV mux on `render_sequence`

RFC-0004 deferral; adapter-side wiring. Lower priority than `apply_vrma` in render_sequence above. Useful for site reviewer ergonomics but not on the conformance critical path.

## How items leave this file

- **To a GH issue**: when the item is concrete enough to scope (acceptance criteria writable in a paragraph), small enough for one contributor (or one short PR), and someone is likely to pick it up. The RFC-level items above generally aren't ready yet.
- **To an RFC under `rfcs/`**: when the item needs design discussion before implementation. Camera-mode op and VRMC_node_constraint both qualify when they become priorities.
- **To `docs/methodology.md`**: when the question resolves to "this is intentional spec behaviour, document and stop testing as a conformance hook". The occlusionTexture finding is the worked example.
- **Closed**: when the item is no longer relevant. Items don't expire from this file automatically; periodic review.
