# Synthetic-collider validation corpus (VMK #309/#313 augment-on/off)

**Status:** Design — pending user review
**Date:** 2026-06-06
**Author:** Claude (with Paul Flynn)

## Purpose

VRMMetalKit 0.17.0-rc.2 ships *synthetic spring-bone collider augmentation* (#309/#311/#312) and *continuous swept collision* on that synthetic group (#313). Both are now **closed upstream**, so this corpus is **post-hoc independent confirmation**, not a release gate: it gives the conformance suite a falsifiable, quantitative measurement that rc.2's synthetic colliders actually deflect a fast/ resting chain.

This is **single-renderer feature validation, not cross-renderer conformance.** Synthetic collider augmentation is a VMK invention, not VRM spec — UniVRM / godot / three-vrm do not generate synthetic colliders, so there is **no oracle**. The baseline is therefore **VMK augment-ON vs augment-OFF in the same rc.2 build** (one variable).

## Success criteria

A new corpus + pipeline that, run against the VMK rc.2 adapter, reports for each variant:

1. **Augment OFF:** the hair chain penetrates the region where the synthetic collider would be (penetration depth > 0).
2. **Augment ON:** the hair chain is held at/outside the synthetic collider surface (penetration depth ≈ 0, within epsilon).
3. The **ON−OFF delta** is the validation signal: augmentation measurably deflects the chain on both the static (#309) and fast-rotation (#313 swept) excitations.

If ON == OFF (identical trajectories), augmentation did not fire — a negative result the corpus must surface clearly, not hide.

## Non-goals

- Cross-renderer comparison (no oracle; VMK-only).
- Validating the *visual* quality of the deflection (we measure joint geometry, not pixels).
- Hand/finger colliders (#321, open — out of scope here).
- Re-deriving the synthetic collider geometry suite-side; we read it back from VMK.

## Step 0 — augmentation-fires spike (gating, throwaway)

Before building anything, confirm VMK generates synthetic colliders for our **parametric** humanoid (the generator already emits the full 20-bone humanoid the augmentor keys off; the augmentor reads bone transforms only, no mesh/skin — so it *should* fire). Verify by reading `model.springBone.syntheticColliders.count > 0` for a generated humanoid asset (debug-physics build dump, or the augment-on/off flag once it exists).

- **If it fires:** proceed with the parametric humanoid (preferred — parametric control, CC0).
- **If it does not:** fall back to the `AvatarSample_A_1.0` fixture (already symlinked at `assets/humanoid/`), and document the corpus as fixture-backed rather than parametric.

This spike is discardable; it does not ship.

## Architecture

Five components, each independently testable, following existing suite patterns.

### 1. Asset (generator)

A new emit path producing a parametric humanoid + a single **hair** spring-chain hung off the head node, positioned to intersect the synthetic skull sphere / head capsule region under excitation. The spring must be named so VMK/our extraction recognizes it as hair (the augmentor and our metric both key off chain identity).

- One asset, rendered two ways (ON/OFF) × two excitations (static settle, fast root rotation).
- Excitations reuse existing plan blocks: static = `reset_physics` settle only; swept = `animate_root_transform` fast rotation (the swing-sweep mechanism — fast root rotation makes the head-attached synthetic collider whip through the lagging hair).
- Emits the paired triplet (`.vrm` + `.meta.json` + `.test.yaml`) per the single-source-of-truth rule. The plan carries `render_sequence` with `capture_positions: true` and the new `capture_synthetic_colliders: true`.

### 2. Adapter: augment on/off flag

Thread VMK's `VRMLoadingOptions.augmentSpringBoneColliders` (default `true`) through the adapter's `load_vrm` as an optional `augment_colliders: bool`. Default `true` to match VMK shipping behavior. This flag is the ON/OFF baseline.

### 3. Adapter: synthetic-collider dump

Because synthetic colliders are bone-attached, they **move every frame** under the swept excitation. So the dump is **per-frame**, captured alongside `spring_positions` in `render_sequence`. Extend the frame output with an optional `synthetic_colliders` list, populated when `capture_synthetic_colliders` is set:

- Read `model.springBone.syntheticColliders` (local-space: node + shape offset/radius/tail).
- Transform to **world space** using the node's `worldMatrix` at that frame (same frame/convention as `spring_positions`, already validated for VMK capture).
- Emit each as a world-space sphere `{center, radius}` or capsule `{a, b, radius}` — the existing `ColliderWorldSpec` shape.

### 4. Runner: persist per-frame colliders

Mirror the positions-JSON plumbing. After `render_sequence`, persist `<id>_<renderer>_colliders.json` — a per-frame array `[{frame_index, colliders: [ColliderWorldSpec...]}]`. Per-op path in `execute.rs`; the batch path (UniVRM) is N/A here (VMK is per-op).

### 5. diff-engine: per-frame (moving) penetration

Current `penetration` measures joints vs **world-fixed** colliders. Extend to **per-frame** colliders: zip frame N joints with frame N colliders by `frame_index`, run the existing signed-distance math, report deepest penetration across all frames. Expose via `penetration-diff --colliders <per-frame-json>` (new optional source; falls back to the plan's static `ccd_colliders` when absent). The signed-distance functions are unchanged — only the collider source becomes per-frame.

## Data flow

```
emit (humanoid + hair chain, plan: capture_positions + capture_synthetic_colliders)
  → adapter render_sequence  (×2: augment_colliders ON / OFF)
       per frame: spring_positions  +  synthetic_colliders (world space)
  → runner persists  <id>_<r>_positions.json  +  <id>_<r>_colliders.json
  → penetration-diff (per-frame: joints[N] vs colliders[N])
       ON  → max_penetration ≈ 0
       OFF → max_penetration > 0
  → report ON vs OFF delta
```

OFF has no synthetic colliders to dump, so its penetration is measured against the **ON run's** captured collider geometry (the colliders that *would* exist) — i.e., colliders JSON from the ON run, positions JSON from each run. This is geometrically valid because the synthetic colliders are attached to the head/leg bones, whose trajectory is driven by the (root) animation, **not** by spring physics — so the head-bone path (and thus the synthetic collider path) is identical in the ON and OFF runs. Only the hair chain differs. This makes "did the chain enter the synthetic volume" a fair ON-vs-OFF comparison.

## Coordinate frames

VMK node world positions are already glTF-world (validated by the existing `capture_positions_vmk` test — joints lag root under inertia). Synthetic colliders transform through the same node `worldMatrix`, so collider and joint geometry share one frame. No conversion in the runner.

## Testing (TDD)

Each component lands test-first:

- **diff-engine:** unit test — per-frame moving collider; joint inside frame-3 collider but outside frame-0 collider → reports frame-3 penetration. (Pure Rust, no toolchain.)
- **runner:** fast test with a mock adapter fixture emitting `synthetic_colliders` → asserts `<id>_<r>_colliders.json` written with per-frame shape. (Mirrors `batch_capture_positions_writes_positions_json`.)
- **adapter (toolchain-gated, `--ignored`):** render a humanoid with `augment_colliders` ON → `synthetic_colliders` non-empty and move across frames under rotation; OFF → empty.
- **end-to-end (toolchain-gated):** the corpus asset through rc.2 → ON penetration ≈ 0, OFF > 0, on both excitations.

## Risks / open questions

1. **Step-0 linchpin** (augmentation fires for parametric humanoid) — gates parametric vs fixture path. Resolved before implementation.
2. **Chain placement** — the hair chain must actually enter the synthetic volume when OFF (else OFF penetration is also ~0 and there's no signal). May need to tune chain length / attach point / rotation speed so OFF clearly penetrates. Iterative.
3. **Root-joint exclusion** — VMK's own tests exclude kinematically-driven root joints from collision; our metric should likewise exclude joint 0 of each chain (matches `HairHeadCollisionTests`).
4. **Negative result is valid output** — if rc.2's augmentation is so effective that even OFF barely penetrates (small chain), the delta shrinks; document thresholds so a true null is distinguishable from a wiring bug.

## Findings deliverable

On completion, a `docs/findings.md` entry: the ON/OFF penetration table per excitation, stating whether rc.2's synthetic augmentation measurably deflects the chain — the suite's independent confirmation of the #309/#313 gated behavior.
