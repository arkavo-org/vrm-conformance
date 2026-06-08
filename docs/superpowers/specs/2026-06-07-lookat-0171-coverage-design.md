# VMK 0.17.1 lookAt (#332) conformance coverage — design

**Date:** 2026-06-07
**Status:** Approved (brainstorming)
**Driver:** VMK pin bump 0.17.0 → 0.17.1, which is a behavior change to bone-driven eye look-at. The suite must observe the fix end-to-end rather than carry a blind pin bump.

## Background

VMK 0.17.1 (`421232b`, tag `0.17.1`; release closes upstream #332) corrects two
bugs in **bone-driven eye look-at**, both behavior changes to rendered eye direction:

- **(A) Head-local gaze resolution.** `updateTargetAngles` computed yaw/pitch in
  **world space** and wrote them as a **local** eye-bone rotation, so any rig whose
  head is turned (body rotated at the root) drove the eyes off by the head's yaw.
  Targets now resolve through the head's inverse world matrix (`.headLocalPoint`
  was equally affected). **Only observable with a turned head.**
- **(B) Eye-bone rest composition.** `applyToBones` / `applyToAnimationState`
  overwrote the eye bones with a bare gaze quaternion, discarding the authored rest.
  VRoid rigs (`J_Adj_*_FaceEye`) carry a large **mirrored outward** eye rest (~±22°);
  discarding it splayed the eyes **wall-eyed at center** and inverted gaze directions.
  Now composes `gaze * initialRotation` so the rest cancels in the skinning delta.
  **Only observable on a rig with an authored eye rest.**

The release notes name `vroid_default_F_1_0.vrm` as the validation avatar and note
`AvatarSample_A` hid the bug (white sclera). **This suite's synthetic humanoid corpus
has no eye bones at all** — which is exactly why the entire `vrma_lookat_*` history
(VMK#286 → #294 → #297) could only ever verify the gaze *parse*, never the rendered
eye direction. See `docs/findings.md` "VMK lookAt rotation-channel gap" and
`docs/upstream/VMK-vrma-lookat-renderer-propagation.md`.

## Goal

Cover both #332 sub-bugs in conformance using the real `vroid_default_F_1_0.vrm`
avatar (already in the manual corpus for spring-bone), so the 0.17.0 → 0.17.1 change
is a verifiable before/after rather than an unobserved pin bump.

## Decisions (from brainstorming)

- **Vehicle: real VRoid avatar (primary).** The wall-eye bug is fundamentally a
  property of VRoid's authored mirrored eye rest; the real avatar is the faithful
  repro and the one VMK validated against. Extending the synthetic generator with
  parametric eye bones is a **noted follow-up**, not in this scope.
- **Coverage: both sub-bugs.** Gaze sweep at neutral body (bug B) **and** turned-head
  variants (bug A). The turned head comes from a VRMA hips/spine rotation channel
  (the plan schema's `root_transform` is translation-only and cannot rotate the root).

## Components

### 1. VMK pin bump

`adapters/vrm-metal-kit/Package.swift`: revision → `421232b75c77d65d8d2bd827a36159936b68db23`
(tag `0.17.1`). Add a `0.17.1` changelog comment block above the existing `0.17.0`
entry, per the file's convention: record both behavior changes (head-local resolution;
eye-rest composition), that they affect bone-driven eye look-at on VRoid-style rigs and
any turned head, that no shader/metallib changed from 0.17.0, and that this suite's new
`vroid_default_F_gaze_*` corpus is the suite-side verifier closing the long-standing
"asset coverage gap" follow-up.

### 2. Generated gaze VRMA clips

A new sweep (`crates/vrm-asset-generator/src/sweep.rs`) + an `emit-gaze-sweep`
subcommand (mirroring `emit-vrma-lookat-sweep`) composing existing emitter primitives
in `vrma_emit.rs`:

- `register_all_humanoid_bones` (satisfies importer invariants for loaders)
- `add_look_at_channel` (gaze quaternion → lookAt node rotation)
- `add_humanoid_bone_rotation_channel` (spine/hips yaw, for the turned-head clips)

Clips:

| clip id | gaze (yaw, pitch) | body yaw (spine) | targets bug |
|---|---|---|---|
| `gaze_center`      | (0, 0)    | 0    | B (wall-eye at center) |
| `gaze_left`        | (+30, 0)  | 0    | B (inverted gaze) |
| `gaze_right`       | (−30, 0)  | 0    | B (inverted gaze) |
| `gaze_up`          | (0, +20)  | 0    | B |
| `gaze_down`        | (0, −20)  | 0    | B |
| `gaze_center_bodyL`| (0, 0)    | +35  | A (head-local) |
| `gaze_center_bodyR`| (0, 0)    | −35  | A (head-local) |
| `gaze_right_bodyL` | (−30, 0)  | +35  | A (gaze and turn opposed — hardest) |

Gaze/body angles are nominal and tunable during bootstrap against the avatar's lookAt
`rangeMap` (VRoid eye range is small; angles that exceed `inputMaxValue` clamp). Sign
convention follows the existing `vrma_lookat_*` corpus (gaze rotation vs world frame).
Body yaw is applied to `spine` (so head + eyes inherit it) — verified during
implementation that this turns the head in the rendered pose.

### 3. Manual test plans

`test-plans/manual/humanoid/vroid_default_F_gaze_<clip>.test.yaml`, one per clip,
pairing `asset: vroid_default_F_1_0.vrm` with `animation.vrma.path: <clip>.vrma`.

- **Face/eye-tight camera** — the load-bearing methodology choice. Gaze "barely moves
  pixels at 1024²" (findings); a tight eye-region crop is what turns iris splay/shift
  into a real SSIM signal. Frame on the eyes (small FOV, camera close, target at eye
  height ~1.4 m for this avatar; exact framing tuned at bootstrap).
- Methodology pins: `tone_mapping: none`, `cast_shadows: false`, `receive_shadows: false`.
- `pose_tolerance` block with `look_at_yaw_pitch_degrees` and
  `per_bone_quaternion_radians` set (so the pose-diff layer asserts too).
- `conformance_status: included`. Reference renderer: `three-vrm` (composes eye rest
  correctly), consensus with godot as third.

### 4. Verification signals (priority order)

The 19-bone pose-dump reference list excludes the eyes, so the humanoid pose dump
alone cannot capture the eye-bone splay. Signals, strongest first:

1. **Tight-crop image SSIM (primary).** Wall-eye splay and inverted gaze are visible
   at the iris. 0.17.0 → 0.17.1 is a visible flip on `gaze_center` (wall-eyed →
   parallel) and on the `*_body*` clips (offset → head-relative).
2. **Eye-bone pose-dump extension (in scope, recommended).** Add `leftEye` / `rightEye`
   to the pose-dump reference humanoid-bone list in **both** the VMK adapter
   (`Operations.swift::referenceHumanoidBones`) and the three-vrm host
   (`renderer-host.html` `HUMANOID_BONES`) so the cross-renderer pose diff carries a
   **numeric** eye-bone signal — the composed eye rotation should be `gaze * rest`,
   directly catching bug B. Synthetic avatars lacking eye bones simply report them
   absent (skipped by the diff), so existing corpora are unaffected.
3. **`dump_look_at_state` yaw/pitch consistency (tertiary).**
4. **Oracle:** consensus VMK ↔ three-vrm ↔ godot. Real-1.0 UniVRM oracle is currently
   blocked by the known `execute-test-batch` manual-plan limitation (suite tooling, not
   VMK) — note it, do not block on it.

### 5. Findings entry

`docs/findings.md`: record the 0.17.0 (wall-eyed / head-yaw-offset gaze) vs 0.17.1
(parallel at center, head-relative when turned) before/after across the new corpus,
per the findings-as-deliverable convention. Mark the
`VMK-vrma-lookat-renderer-propagation.md` "suite-side asset coverage follow-up" closed.

## Scope boundaries (YAGNI)

- **No synthetic eye-bone generator extension** — deferred follow-up.
- **No plan-schema body-yaw field** — turned head via VRMA hips/spine rotation channel.
- **Expression-driven lookAt out of scope** — VRoid default is bone-driven and #332 is
  bone-path-only; the synthetic `vrma_lookat_*_expr` corpus already covers the
  expression parse.

## Risks / verify during implementation

- **Apply order.** `applyImmediately()` must run *after* the VRMA spine rotation is
  baked into the head's world matrix, or the head-local resolution reads a stale head
  transform and the `*_body*` clips won't move on 0.17.1 either. Check the adapter
  wiring in `handleApplyVrmaAtTime`.
- **VRMA loadability across adapters.** Earlier findings (issue #8) showed UniVRM
  rejecting lookAt-only VRMAs (`TransferOwnership` null). The `*_body*` clips carry a
  humanoid rotation channel and should load; the neutral-body gaze clips rely on
  `register_all_humanoid_bones` satisfying the invariant. Confirm at bootstrap; if
  UniVRM still rejects, the VMK/three-vrm/godot consensus stands.
- **Rendering is local-only.** VMK needs macOS 26 / Xcode 26; CI build-validates only.
  The gaze re-bootstrap and 0.17.0/0.17.1 before/after run on an M-series Mac.

## Out-of-band follow-ups (tracked, not this change)

- Synthetic generator: parametric `leftEye`/`rightEye` with mirrored rest, for a fully
  parametric gaze × rest sweep ("address the whole gap").
- Suite tooling: real-1.0 UniVRM oracle via `execute-test-batch` manual-plan support.
