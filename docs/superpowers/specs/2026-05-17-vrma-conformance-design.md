# VRMA conformance test suite — design

**Date:** 2026-05-17
**Status:** Approved
**Author:** drafted via brainstorming session
**Spec reference:** [VRMC_vrm_animation-1.0](https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_vrm_animation-1.0)

## Purpose

Add VRMA (`VRMC_vrm_animation`) conformance coverage to the suite. VRMA is the VRM 1.0 extension that defines a portable animation file format — a `.vrma` file applied to any conformant VRM 1.0 avatar should produce the same humanoid pose, expression weights, and gaze direction across renderers. The suite has zero VRMA coverage today; this design closes that gap.

Mirrors the seven-phase spring-bone closure pattern: paired-triplet assets, parametric generator + manual humanoid plans, new ops in `crates/vrm-ops/`, pose-vector primary diff with rendered-frame SSIM as corroborating signal, UniVRM as the consortium reference.

## Architecture overview

The load-bearing claim: **the `.vrma` file is the input under test, not the `.vrm`.** The avatar provides a target rig; the `.vrma` provides the spec-defined animation channels we exercise. A test plan binds one `.vrm` to one `.vrma` at one time-instant `t` and asserts on the pose vector after `apply_vrma_at_time(t)`. Per-bone quaternion diffs, per-expression weight diffs, and lookAt angle diffs are the three pass/fail axes. Rendered-frame SSIM runs alongside as a regression net.

No new crate. `vrm-ops/`, `vrm-asset-generator/`, `vrm-diff-engine/`, `vrm-runner/`, `vrm-test-plan/` all gain VRMA features in place. Same trust model as existing artifacts: `.vrma` binaries go to S3 with BLAKE3 refs and live in `goldens/manifest.json` next to `.vrm` and `.png` entries. The committed file is the manifest pointer; the actual `.vrma` lives at the referenced URL.

The VRMA spec (per the canonical README) confirms three structural facts that shape this design:

1. **Three optional channels at the extension root** (`humanoid`, `expressions`, `lookAt`); only `specVersion: "1.0"` is required. Each channel is independently optional, so the suite must handle .vrma files that exercise any subset.
2. **VRMA reuses glTF core animation channels.** No separate keyframe model. Spec recommends 30 FPS as a guideline; "the animation is interpolated linearly in the implementation" is the implementer's responsibility. Time sampling at any `t` is deterministic.
3. **`animations[0]` is the canonical clip.** Multiple animations in one .vrma file are allowed but only animations[0] is portable. The generator emits single-animation .vrma files.

## Scope

V1 covers **all three VRMA channel types**: humanoid bones, expressions, and lookAt. Adapter coverage is **cross-renderer day one**, with two real implementations (UniVRM, three-vrm) and two `Unimplemented` stubs (VMK, godot-vrm) — the absence of support in the latter is itself the conformance signal that drives upstream movement.

## Asset model

Two emission paths under `vrm-asset-generator`, mirroring how the spring-bone closure separated parametric sweeps from humanoid behavior plans.

### Parametric sweeps

New subcommands, each emits paired triplets (`.vrm` + `.vrma` + `.test.yaml`):

| subcommand | what it sweeps | size |
|---|---|---|
| `emit-vrma-humanoid-sweep` | single-bone rotation curves. Each variant rotates one of N humanoid bones (hips, spine, leftUpperArm, …) through an axis-aligned arc over a 1.0 s clip. Axis isolation. | ~15 plans |
| `emit-vrma-expression-sweep` | single-expression weight ramps. Each variant animates one preset expression (happy, blink, …) from 0→1→0 over 1.0 s. Plus 2–3 custom-blendshape variants. Weight encoded as the X-component of translation per spec. | ~12 plans |
| `emit-vrma-lookat-sweep` | yaw/pitch direction sweeps over a 1.0 s clip. Same .vrma corpus tested against two avatar configurations (`VRMC_vrm.lookAt.type: bone` and `: aim`) since the application path is an avatar property, not a VRMA property. | ~5 directions × 2 avatar configs = 10 plans |

Sweeps share a canonical synthetic avatar (generated minimal humanoid rig — same role as the bust-chain rig in spring-bone sweeps). Each sweep variant changes one parameter against a baseline; **no confounding axes** per the existing methodology rule.

### Manual humanoid plans

Hand-authored under `test-plans/manual/humanoid/`. Small set of avatarA-paired clips representing realistic humanoid behavior:

- `avatarA_wave_hello.vrma` — right arm raise + wave gesture
- `avatarA_nod_yes.vrma` — head yaw + pitch
- `avatarA_idle_breathing.vrma` — subtle hips + chest motion
- `avatarA_blink_sequence.vrma` — eye expression timing

These exercise the integration between humanoid bones + expressions + spring-bone physics + lookAt simultaneously. Smaller set (4–6) but each plan asserts on multiple channels and runs SSIM corroboration against the consortium reference. Hand-authored in Blender and exported via UniVRM's VRMA exporter so the .vrma files are spec-valid by construction.

### Test plan schema extension

`vrm-test-plan` adds an optional `animation.vrma` block:

```yaml
animation:
  vrma: ../assets/generated/vrma_humanoid_leftUpperArm_yaw_30.vrma
  apply_at_time: 0.5
diff:
  pose_tolerance:
    per_bone_quaternion_radians: 0.010
    hips_translation_m: 0.005
    per_preset_expression: 0.005
    per_custom_expression: 0.005
    look_at_yaw_pitch_degrees: 1.0
    offset_from_head_bone_m: 0.001
  threshold: 0.95  # corroborating SSIM, optional gate
  reference_renderer: univrm
```

`animation.root_transform` (existing) and `animation.vrma` (new) are independent — one drives world-space root translation, the other drives internal pose; a plan can use either or both.

Total v1 corpus footprint: ~37 parametric sweep plans + 4–6 manual plans ≈ 41–43 new plans, on top of the existing 222.

## Op surface additions

Five new ops in `crates/vrm-ops/`, all following the existing describe → JSON-RPC stdio contract. Each lands in the op catalog with a JSON Schema and gets the standard `Unimplemented` envelope (`-32000`, `data: { phase: "vrma-v1" }`) for adapters that haven't shipped support.

### 1. `load_vrma`

Parse a `.vrma` file, return an opaque handle.

```
params: { vrma_path: string | blake3_ref }
result: {
  vrma_handle: u32,
  channel_summary: {
    humanoid_bones: u32,        // count of bones referenced
    expressions: u32,           // count across preset + custom
    has_look_at: bool,
    duration_seconds: f32
  }
}
```

Mirrors `load_vrm`'s parse-then-handle pattern. `channel_summary` lets agents preview what they're about to apply without touching scene state. Loads `animations[0]` per spec.

### 2. `apply_vrma_at_time`

Sample the loaded clip at `t` and write the resulting pose onto the avatar.

```
params: { vrma_handle: u32, time_seconds: f32, vrm_handle: u32 }
result: {
  channels_applied: {
    humanoid_bones: u32,
    expressions: u32,
    look_at: bool
  }
}
```

State-advancing. Must run after `load_vrm` + `load_vrma`. The dump ops below capture state as of the most recent `apply_vrma_at_time`. Linear interpolation is the spec-mandated default.

### 3. `dump_humanoid_pose`

Return per-bone local rotations plus the hips translation.

```
result: {
  bones: [
    { name: "leftUpperArm", local_rotation_quat: [x, y, z, w] },
    ...
  ],
  hips_translation: [x, y, z],     // only hips carries translation per spec
  bones_missing: ["leftThumbDistal", ...]   // bones declared in .vrma but absent from .vrm
}
```

Local rotations, not world — local is what .vrma encodes, so the diff stays in the input coordinate space. Missing bones don't crash the dump (they appear in `bones_missing` and are excluded from per-bone diff). The spec forbids scales on humanoid bones and translations on bones other than hips, so the dump intentionally has no `local_translation` per bone or `local_scale` field.

### 4. `dump_expression_weights`

Return per-expression scalar weights, keyed by preset name and custom name.

```
result: {
  presets: {
    happy: 0.83,
    blink: 0.02,
    aa: 0.0,
    ...
  },
  custom: {
    "<custom-name>": 0.5,
    ...
  }
}
```

The preset/custom split matches the spec's structure. 14 canonical presets defined: `happy, angry, sad, relaxed, surprised, aa, ih, ou, ee, oh, blink, blinkLeft, blinkRight, neutral` (note: `lookUp/lookDown/lookLeft/lookRight` are explicitly excluded from VRMA expressions — driven by LookAt instead).

Per the spec, weights are encoded as the X-component of the bound node's translation animation, clamped to `[0, 1]`. The dump returns the clamped value the renderer actually applies.

### 5. `dump_look_at_state`

Return the current lookAt state with both raw and spec-converted forms.

```
result: {
  gaze_direction_quat: [x, y, z, w],    // raw quaternion from .vrma node rotation
  yaw_deg: f32,                          // extrinsic ZXY around Y, per spec
  pitch_deg: f32,                        // extrinsic ZXY around X, per spec
  applied_via: "bone" | "expression" | "off",  // from avatar's VRMC_vrm.lookAt.type
  offset_from_head_bone: [x, y, z]       // from .vrma; head-local origin
}
```

The `applied_via` field reports how the *avatar* is configured to apply gaze — independent of what the .vrma encoded. This separation matches the spec: VRMA declares gaze direction; the avatar's `VRMC_vrm.lookAt.type` declares the application path. There is no `lookAtType` enum inside VRMA.

The yaw/pitch conversion is spec-mandated: "The rotation order of the Euler angle must be interpreted as Extrinsic ZXY, and the rotation around the Y axis is yaw and the rotation around the X axis is pitch."

### Op sequence

```
load_vrm → set_camera → set_lighting → set_post_processing
        → load_vrma → apply_vrma_at_time(t)
        → dump_humanoid_pose → dump_expression_weights → dump_look_at_state
        → [reset_physics → step_physics]
        → render → dispose
```

The dumps run *before* physics steps so they capture the pure VRMA-applied pose, uncontaminated by spring-bone settling. `animate_root_transform` is orthogonal and composes — one drives root translation (world-space), the other drives internal pose.

## Time sampling

Each test plan samples the clip at one discrete `time_seconds`. Multiple plans cover multiple times within the same clip when needed (e.g. `avatarA_wave_t0p25.test.yaml`, `avatarA_wave_t0p5.test.yaml`). Same shape as the existing settle/swing pattern; no sequence-of-frames concept introduced. The spec's linear interpolation rule plus the `animations[0]` portability rule make any `t` deterministic.

## Diff math

Pass = all per-channel deltas within tolerance. Per-channel tolerance fields settable per-plan; v1 defaults below.

| signal | metric | v1 default tolerance | rationale |
|---|---|---|---|
| per-bone rotation | quaternion geodesic distance: `2·acos(|q_actual · q_ref|)` (radians) | **0.010 rad** (~0.57°) | tight enough to surface keyframe interpolation drift; loose enough to absorb float-precision noise |
| hips translation | Euclidean distance (meters) | **0.005 m** (5 mm) | matches spring-bone settle position-diff floor |
| preset expression weight | scalar `abs(delta)` | **0.005** | half a percent — surfaces routing bugs without flagging float noise |
| custom expression weight | scalar `abs(delta)` | **0.005** | same |
| lookAt yaw / pitch | abs-delta in degrees (extrinsic ZXY) | **1.0°** | per-axis; ZXY conversion is spec-mandated |
| `offsetFromHeadBone` | Euclidean (meters) | **0.001 m** (1 mm) | bone-anchored geometry; near-static datum |

The runner extends `vrm-diff-engine/` with a new `pose_diff` module (parallel to the existing `positions` module from spring-bone phase 1):

```rust
struct PoseDiffReport {
    per_bone_rotation_max_rad: f32,
    per_bone_rotation_worst_bone: Option<String>,
    hips_translation_m: f32,
    per_preset_expression_max_delta: f32,
    per_preset_expression_worst: Option<String>,
    per_custom_expression_max_delta: f32,
    look_at_yaw_delta_deg: f32,
    look_at_pitch_delta_deg: f32,
    offset_from_head_bone_m: f32,
    overall_passed: bool,
}
```

Rendered-frame SSIM runs alongside; it goes in `ExecuteResult` next to `PoseDiffReport` and does not gate pass/fail unless the plan's `diff` block explicitly sets the SSIM `threshold` field (same affordance as the spring-bone plans).

## Reference baseline

**UniVRM is the consortium reference.** Same role it plays for MToon and spring-bone in the existing methodology: when renderers disagree, UniVRM is the named oracle. Pose-vector diff is computed as `<renderer> vs univrm` for each test_id; `conformance_status` in the plan declares the threshold per `vrm-conformance#2/#3`.

Two escape hatches match the existing methodology:

1. **`conformance_status: excluded`** for tests where UniVRM has a known defect or interprets a spec edge case in a renderer-specific way. Reason recorded in the plan; test still renders for visibility but doesn't count against pass-rate. Same mechanic as outline-extreme exclusions.
2. **Optional hand-authored golden override** at `<plan>.golden.json` — a pose vector authored to spec-correct values, used in place of UniVRM's output when UniVRM is the deviant. Rare; only relevant if we surface a UniVRM bug while building this. Stored next to the plan, never on S3.

Runner default is "diff against UniVRM"; override path activates when `golden.json` is present.

## Adapter staging

| adapter | v1 status | new code | issue surface |
|---|---|---|---|
| **UniVRM** | **real** | bind ops to existing `VrmAnimationImporter` + `Vrm10AnimationInstance` in `com.vrmc.vrm@4a17eb92884b`. No library work needed. | none |
| **three-vrm** | **real** | add `@pixiv/three-vrm-animation` to `adapters/three-vrm/package.json`; implement 5 ops in our adapter shim using `VRMAnimationLoaderPlugin` + `VRMAnimation` runtime. Real upstream library exists; pure adapter-side wiring. | none |
| **godot-vrm** | `Unimplemented` (`-32000`, `phase: "vrma-v1"`) | none | file issue against `V-Sekai/godot-vrm` requesting the empty `addons/vrm/1.0/VRMC_vrm_animation.gd` stub be completed |
| **VRMMetalKit** | `Unimplemented` (`-32000`, `phase: "vrma-v1"`) | none | [VMK#165](https://github.com/arkavo-org/VRMMetalKit/issues/165) already open; comment with our spec test surface once published |

Two real adapters from v1 gives us cross-renderer pose-vector diff signal from the start. The two `Unimplemented` adapters surface their absence in the consensus report as "no entry for this test_id from renderer X" — visibility drives upstream movement, same dynamic as spring-bone closures.

## Methodology hazards specific to VRMA

Added to `docs/methodology.md` as a new section. Each is observable divergence that is not a renderer correctness issue.

1. **T-pose mismatch in the source avatar.** Spec mandates T-pose as humanoid rest pose; `how_to_transform_human_pose.md` defines retargeting when a model is not in T-pose. If avatarA's authored rest pose drifts, all bone rotations diff against UniVRM in a *systematic* way that looks like a renderer bug but isn't. **Mitigation:** v1 sweep corpus uses the generator's canonical-T-pose synthetic rig; manual humanoid plans use avatarA after a one-time T-pose audit logged in findings.
2. **Hips translation accumulator vs. set.** Renderers may interpret a hips translation channel as "set hips position absolutely" or "add to rest pose hips position." Both are spec-compliant if the source data matches the convention. **Test plans MUST author hips translations as deltas from rest** (zero at t=0) to keep the convention unambiguous.
3. **Missing humanoid bones.** A `.vrm` without `leftThumbDistal` paired with a `.vrma` that animates it — the missing bone goes into `dump_humanoid_pose.bones_missing` and is excluded from per-bone diff. Renderers must not crash but may apply or ignore differently. Excluded bones don't contribute to pass/fail.
4. **Expression preset routing.** A renderer may route a preset name (`happy`) to a different blendshape than the asset's authored mapping. Diff at the *weight* level (per the spec's "X-component of translation" rule) catches application; doesn't catch routing semantics. Custom-expression sweeps probe routing semantics by binding non-preset names.
5. **LookAt avatar config invariance.** The same `.vrma` against `VRMC_vrm.lookAt.type: bone` vs `: aim` produces identical `dump_look_at_state.gaze_direction_quat` but different rendered frames (and different applied head/eye bone rotations). Pose-vector diff catches the avatar's effective applied gaze; rendered-frame SSIM catches the visual result. Both avatar configurations are valid and tested as paired corpus.
6. **Animation index ambiguity.** Spec mandates `animations[0]` for portability. Multi-animation `.vrma` files are allowed but only `animations[0]` is portable. The generator emits single-animation files; manual humanoid plans must verify single-animation export (Blender → UniVRM VRMA exporter does this by default).
7. **Quaternion shortest-path conventions.** A renderer comparing rotations as `q` and `-q` (same orientation, opposite signs) will produce different geodesic distances against UniVRM if they don't canonicalize. **Mitigation:** the diff math uses `2·acos(|dot|)` (absolute value), which is sign-invariant by construction.
8. **30 FPS guideline drift.** Spec recommends 30 FPS but allows any keyframe spacing. The generator emits at 30 FPS; tests sample at non-keyframe times to exercise interpolation. Expect ≤0.005 rad per-bone divergence at non-keyframe times — that's why the 0.010 rad tolerance is set where it is.

## Out of scope for v1

- **Animation retargeting between non-T-pose source and T-pose target.** Spec defines this in `how_to_transform_human_pose.md`; deferred to a future phase.
- **Multi-animation `.vrma` files.** Spec allows; suite emits and tests only single-animation files.
- **Animation blending / mixing.** A real-world consumer concern but not a conformance question.
- **Streaming `.vrma` (network/realtime).** Out of scope; tests load complete files.
- **Multi-instant sequence tests.** Each plan samples one `t`. If a follow-up phase needs sequence assertions, that's additive work — not a v1 architectural shift.

## Out of scope for this spec

- **Implementation plan.** This spec defines what gets built; the implementation plan (work breakdown, phasing, dependency ordering, test fixtures) is a separate document produced via the writing-plans skill after this design is approved.
- **Tolerance calibration.** The v1 defaults in the diff math table are starting points. Calibration against real cross-renderer data (UniVRM vs three-vrm baseline) is part of implementation — adjust thresholds based on observed noise floor before the corpus ships.

## Forward

After this design is approved and the implementation plan is written, the closure shape mirrors the spring-bone closure: ship infrastructure, ship two real adapters, file two upstream issues (godot-vrm stub completion, VMK#165 progress comment), bootstrap goldens through both real adapters, run consensus-report, log findings, drive upstream movement on the two `Unimplemented` adapters.
