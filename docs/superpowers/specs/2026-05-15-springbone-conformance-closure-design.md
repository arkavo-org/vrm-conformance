# VRMC_springBone full conformance gap closure — design

Author: paul@arkavo.com (with Claude)
Date: 2026-05-15
Status: design — implementation plan to follow via `writing-plans`

## Goal

Close the spec-coverage gap for `VRMC_springBone` and `VRMC_springBone_extended_collider`. Surface the class of immersion-breaking bugs the 0.14.0 collider parse fix exposed (silent-zero collider geometry) plus the parameter-coupling regression in VMK#162. Add a precision-grade comparison signal — 3D bone positions, not just SSIM — so collision-response divergence between two valid renderers stops registering as a test failure.

## Motivation

`VRMC_springBone` is the entire collision surface of VRM 1.0 — there is no other physics in the spec. Today the conformance corpus covers five spring axes (`joint_count`, `segment_length`, `stiffness`, `drag`, `gravity_power`), all on single chains with no colliders. Real failures users encounter — chain clipping through head/face, hair popping on first frame, tuned-model coupling — sit in untested territory:

- The 0.14.0 collider parse bug ([Double] vs [Float] silent zero) would not have been caught by any plan we own. It surfaced through humanoid renders.
- VMK#233 zero-settle was filed against a humanoid asset, not a synthetic; we wrote `avatarA_bosom_zerosettle` after the fact.
- VMK#162 ("fixing one parameter breaks the rest on tuned models") has no test framework that can express the claim.
- `VRMC_springBone_extended_collider` (planes, inverted sphere/capsule, joint angle limits) has zero coverage. VMK#67 ("verify and document angle limits") lives there.

## Architecture overview

One design, seven phases. Each phase is one PR. Phase 1 is pure infrastructure; phases 2-7 are asset/sweep work that consumes phase-1 infrastructure unchanged.

| phase | content |
|---|---|
| 1 | Infrastructure: `dump_bone_positions` op, manifest extension, position-diff math |
| 2 | Colliders: sphere + capsule (base `VRMC_springBone`) |
| 3 | Extended colliders: planes, inverted sphere/capsule, joint angle limits |
| 4 | `gravityDir` variation |
| 5 | Per-joint parameter taper (scalar → JointVec refactor) |
| 6 | Multi-chain emission |
| 7 | VMK#162 regression — humanoid + self-comparison parameter coupling |

The "infrastructure-first" sequencing is deliberate: the op API benefits from being designed in one shot with full downstream context. Phases 2-7 then become small, predictable, asset-only PRs.

## Phase 1 — Infrastructure

### 1.1 New op: `dump_bone_positions`

Defined in `crates/vrm-ops/`:

```rust
DumpBonePositions {
    session_id: String,
    spring_index: Option<usize>,   // None = all springs
}
// → DumpBonePositionsResult {
//     springs: Vec<SpringPositions>,
// }
// where SpringPositions = { name: String, joint_positions: Vec<[f32; 3]> }
```

**Semantics**: positions are world-space, captured immediately after the most recent state-advancing op (`render` or `step_physics`). The op does not advance physics itself. If called before any state-advancing op, returns the post-load rest-pose positions.

**Adapter implementation cost (estimates):**

| adapter | mechanism | LOC |
|---|---|---|
| vrm-metal-kit | wrap existing `BoneTrajectoryDumper` internals into op result | ~50 |
| three-vrm | per joint: `springBone.joints[i].bone.getWorldPosition(target)` | ~30 |
| godot-vrm (shim) | per joint: `Node3D.global_position` on retained bone-node-path map | ~40 |
| mock | deterministic zeros (no error, contract requires success) | ~10 |
| univrm | returns `Unimplemented` at L3; revisit when L4 PlayMode batch lands | 0 |

**JSON-RPC error envelope**: standard. `-32602` for unknown `session_id`, `-32000` `Unimplemented` for univrm.

### 1.2 Manifest schema extension

Manifest entries gain optional `positions_url` and `positions_blake3`:

```json
{
  "renderer": "three-vrm",
  "test_id": "springbone_collider_sphere_offset_y0",
  "image_url": "s3://.../sphere_y0_three-vrm.png",
  "image_blake3": "...",
  "positions_url": "s3://.../sphere_y0_three-vrm.positions.json",
  "positions_blake3": "..."
}
```

`vrm-s3 validate-manifest` learns both new fields: presence-optional, BLAKE3-validated when present. Schema-level: backward-compatible additive change.

### 1.3 Position diff in `vrm-diff-engine`

New module `positions.rs`:

```rust
pub struct PositionDiffReport {
    pub per_joint_max_drift_m: f32,
    pub chain_summed_drift_m: f32,
    pub per_joint_tolerance_m: f32,
    pub chain_max_drift_m: f32,
    pub passed: bool,
    pub worst_joint_index: usize,
}

pub fn diff_positions(
    actual: &SpringPositions,
    reference: &SpringPositions,
    per_joint_tolerance_m: f32,    // default 0.005 (5 mm)
    chain_max_drift_m: f32,        // default 0.020 (2 cm summed across chain)
) -> PositionDiffReport;
```

Two thresholds because single-joint outliers and chain-wide drift are different failure modes. A chain that lands 1 mm off at every joint is a different bug from one that has a single joint 10 mm out.

### 1.4 Runner extensions

`vrm-runner execute-test-plan` learns `--reference-positions <name>=<positions.json>` (parallel to `--reference` for images). Output JSON adds a `position_diff` block alongside `diff`. `overall_passed` becomes:

```
overall_passed = ssim_passed AND (position_diff.passed OR no positions reference provided)
```

`vrm-runner consensus-diff` learns N-way position consensus the same way it does N-way SSIM.

## Phase 2 — Colliders (sphere + capsule)

### 2.1 Generator changes

In `crates/vrm-asset-generator/src/spring_bone.rs`:

```rust
pub enum ColliderShape {
    Sphere { radius: f32 },
    Capsule { radius: f32, tail_offset: [f32; 3] },
}

pub enum ColliderAttach {
    Head,
    NewIntermediateNode { y_offset: f32, z_offset: f32 },
}

pub struct ColliderParams {
    pub shape: ColliderShape,
    pub offset: [f32; 3],
    pub attach: ColliderAttach,
}

pub struct ColliderGroupParams {
    pub name: String,
    pub collider_indices: Vec<usize>,
}

pub struct SpringBoneSceneParams {
    pub springs: Vec<SpringBoneParams>,         // Vec for forward-compat with phase 6
    pub colliders: Vec<ColliderParams>,
    pub collider_groups: Vec<ColliderGroupParams>,
    pub spring_collider_groups: Vec<Vec<usize>>, // per-spring colliderGroup index lists
}
```

`vrm_ext.rs` `vrmc_spring_bone()` is rewritten to take `SpringBoneSceneParams`. Emits `colliders`, `colliderGroups`, and per-spring `colliderGroups` when non-empty. The "validator rejects empty arrays" rule already documented in the file is respected — empties get omitted.

### 2.2 Sweep — `emit-springbone-collider-sweep`

Chain points down (gravity -Y) toward the collider. Chain-skinned cylinder visibly deflects on contact.

| axis | values | count |
|---|---|---:|
| `collider_shape` | sphere, capsule (tail = chain tangent) | 2 |
| `collider_offset_y` (along chain) | -0.08, -0.04, 0 (in chain path), +0.04 | 4 |
| `collider_radius` | 0.03, 0.05, 0.10 | 3 |

Cartesian = **24 assets**. One-axis-at-a-time gives ~9 assets but collider-vs-chain interactions aren't separable on a single axis: the same radius produces different deflection at different offsets, and the same offset produces different penetration at different radii. The Cartesian cost (24 vs 9) is acceptable given each asset is ~30 KB and rendering is one-shot per renderer.

Each plan exists in both **settle** (60 settle steps, no animation) and **swing** (30 settle + animate_root_transform 15 cm lateral over 0.25 s @ 60 Hz) forms. **24 × 2 = 48 plans**.

### 2.3 Humanoid plan

`test-plans/manual/humanoid/avatarA_bosom_collider.test.yaml` — same framing as the existing bosom corpus, on a small avatar variant that adds an authored head-mounted sphere collider intersecting the bust chain swing path. The avatar variant (`avatarA_collider_1_0.vrm`) is a one-off Blender export; authoring is outside this spec's scope but the plan lands here.

## Phase 3 — Extended colliders

### 3.1 Generator changes

`ColliderShape` gains three variants:

```rust
Plane { normal: [f32; 3] },
InsideSphere { radius: f32 },
InsideCapsule { radius: f32, tail_offset: [f32; 3] },
```

JSON path: top-level `extensions.VRMC_springBone_extended_collider.shape` on each collider entry (parallel to the base `shape` field, which extended-shape colliders omit per spec).

`SpringBoneParams` gains `joint_angle_limit_deg: Option<f32>` — when set, emitted as `extensions.VRMC_springBone_extended_collider.angleLimit` per joint.

### 3.2 Sweep — `emit-springbone-extended-sweep`

| axis | values | count |
|---|---|---:|
| `ext_shape` | plane, inverted_sphere, inverted_capsule | 3 |
| placement | plane: y = -0.04 / -0.08 / -0.15 m below chain root; inverted: radius = 0.10 / 0.20 / 0.40 m centered on head | 3 |
| `angle_limit_deg` | none, 30, 60, 90 | 4 |

Shape × placement = 9 plans. Default-placement shape × non-none angle limits (3 limits × 3 shapes) = 9 more. **18 assets, 36 settle+swing plans.**

## Phase 4 — gravityDir

`gravity_dir` sweep:

| value | semantic |
|---|---|
| `[0, -1, 0]` | baseline (existing) |
| `[0, +1, 0]` | anti-gravity (chain floats up) |
| `[1, 0, 0]` | sideways +X |
| `[0.7, -0.7, 0]` | oblique |

4 settle + 4 swing = **8 plans**. Verifies that adapters don't hard-code -Y in shortcuts.

## Phase 5 — Per-joint taper

### 5.1 SpringBoneParams refactor

```rust
pub enum JointVec<T> {
    Uniform(T),
    PerJoint(Vec<T>),
}

pub struct SpringBoneParams {
    pub joint_count: u32,
    pub segment_length_m: f32,
    pub stiffness:      JointVec<f32>,
    pub drag_force:     JointVec<f32>,
    pub gravity_power:  JointVec<f32>,
    pub gravity_dir:    [f32; 3],
    pub hit_radius:     JointVec<f32>,
    // ...
}
```

Existing sweeps stay scalar via `Uniform(x)`. Sidecar metadata (`.meta.json`) records per-joint vectors when used.

### 5.2 Sweep — `emit-springbone-taper-sweep`

| axis | shape | count |
|---|---|---:|
| `stiffness_taper` | flat, linear high→low, linear low→high, exponential decay | 4 |
| `drag_taper` | flat, linear high→low, exponential decay | 3 |

7 settle + 7 swing = **14 plans**.

## Phase 6 — Multi-chain

`SpringBoneSceneParams.springs: Vec<SpringBoneParams>` from phase 2 makes this trivial generator-side. Each chain attaches to a separate intermediate node off the head, radial-spaced.

### 6.1 Sweep — `emit-springbone-multichain-sweep`

| axis | values | count |
|---|---|---:|
| `chain_count` | 2, 3, 5 | 3 |
| inter-chain radial spacing (m) | 0.02, 0.05 | 2 |
| `collider_group_sharing` | all chains share one group, separate groups, alternating | 3 |

18 settle + 18 swing = **36 plans**.

## Phase 7 — VMK#162 regression

VMK#162 is a single-renderer self-consistency claim: changing one tuned parameter must not silently shift the equilibrium that other parameters establish. The cross-renderer model doesn't fit. Phase 7 adds a new runner mode for self-comparison.

### 7.1 Test pattern

For a tuned humanoid (avatarA bosom config tuned so chains don't pop or sag), render N variants where exactly one parameter shifts by ±10% of its baseline. Off-axis position drift — joints that move when an unrelated parameter is perturbed — must stay within a threshold.

### 7.2 Runner mode: `execute-test-plan-matrix`

Input: a base plan + a perturbation YAML.

```yaml
# avatarA_bosom_tuned_coupling.matrix.yaml
base_plan: avatarA_bosom_tuned.test.yaml
perturbations:
  - { stiffness:     +0.1 }
  - { stiffness:     -0.1 }
  - { drag_force:    +0.1 }
  - { drag_force:    -0.1 }
  - { gravity_power: +0.1 }
  - { gravity_power: -0.1 }
coupling_threshold_m: 0.015
```

Runner spawns the adapter 7 times (1 baseline + 6 perturbations), captures positions via `dump_bone_positions`, computes the off-axis position-delta matrix, asserts each cell ≤ threshold. Failure mode: changing stiffness drifts joints whose drift "should" be drag-induced.

Deliverable: 1 humanoid asset (or reuse `avatarA_1_0.vrm` with a YAML-side tuning override) + 1 perturbation matrix YAML + 7 render passes per matrix execution.

### 7.3 Threshold calibration

`coupling_threshold_m: 0.015` is an opening guess. Phase 7's first task is to render the matrix on three-vrm and godot-vrm, observe their coupling magnitudes (assumed small for well-tuned tuned models), and set the threshold above their max but below the VMK divergence reported in the bug. If three-vrm/godot-vrm coupling is itself >0.015, the threshold rises; if VMK is the only renderer exceeding ~0.005, threshold drops.

## Methodology updates (`docs/methodology.md`)

- **Spring-bone collider parsing**: collider geometry must round-trip through both `[Double]` and `[Float]` JSON encodings (named bug class from the 0.14.0 fix).
- **Position-diff thresholds**: per-joint 5 mm default, chain-summed 2 cm default. Wider bands for swing tests under animation (10 mm / 4 cm).
- **Parameter coupling**: documents the VMK#162 framing and points at `execute-test-plan-matrix`.
- **Settle convergence assumption**: collider tests assume the 30-step settle (60-step for collider sweeps because contacts settle slower) has converged. Plans with mid-settle contacts that release later are out of methodology scope.

## Asset / plan count summary

| phase | settle | swing | matrix | total |
|---|---:|---:|---:|---:|
| 2 collider sweep | 24 | 24 | — | 48 |
| 2 humanoid | 1 | — | — | 1 |
| 3 extended-collider sweep | 18 | 18 | — | 36 |
| 4 gravityDir | 4 | 4 | — | 8 |
| 5 per-joint taper | 7 | 7 | — | 14 |
| 6 multi-chain | 18 | 18 | — | 36 |
| 7 VMK#162 regression | — | — | 1 (×7 renders) | 1 |
| **total** | **72** | **71** | **1** | **144** |

Asset corpus roughly doubles (current synthetic = ~80; this adds ~143). All new synthetic assets ship chain-skinned for real visual signal.

## Risks and known unknowns

1. **VMK `dump_bone_positions` timing**: VMK's spring system advances inside `render()`. The op must capture post-render state, not pre-render. Phase 1 spike to confirm timing before locking the op semantics.
2. **godot-vrm shim positions**: GDScript-side `Node3D.global_position` access requires the bone-node-path map to persist in the session. The shim is expected to have this for skinning lookups already; verify in the phase 1 spike before pinning the LOC estimate.
3. **Extended collider validator coverage**: `mrxz/vrm-validator` may not validate `VRMC_springBone_extended_collider` yet. If absent, our manifest validator is the only check — flag in methodology and consider PR-ing the upstream validator.
4. **VMK#162 threshold tuning**: 0.015 m is a guess. First Phase 7 deliverable is calibration (see 7.3).
5. **Collider-humanoid avatar authoring**: outside generator scope. Blender export of avatarA + one head-mounted sphere collider sized to intersect the bust chain swing path. ~half a day of one-off authoring; not code work, but it blocks the `avatarA_bosom_collider` plan only. The 48-plan synthetic collider sweep does not depend on it and ships independently.
6. **Capacity / golden bootstrap cost**: 144 new plans × 3 real renderers = ~432 new golden renders. Bootstrap walltime grows accordingly; not blocking but worth noting for the next `scripts/bootstrap-goldens.sh` run.

## What this does not do

- Doesn't touch MToon coverage (already separate).
- Doesn't add `VRMC_node_constraint` coverage (separate spec; constraints are not collision).
- Doesn't change the 60 Hz / settle_steps=30 methodology pin; collider tests use 60 settle steps locally but the global default stays.
- Doesn't introduce cross-platform GPU-vendor SSIM tolerance — existing per-pair tolerance model continues.
- Doesn't add a body-collision or cloth-simulation surface; VRM 1.0 has no spec for either.

## Open questions for review

- Phase 1 op as adapter-side read of internal state, vs adapters dumping trajectories to file the runner ingests — design picks the op; file-based is fallback if op proves too costly across adapters.
- Cartesian 24-asset collider sweep vs trimmed one-axis-at-a-time — design picks Cartesian; justification in 2.2.
- VMK#162 framing — design picks self-comparison perturbation matrix; alternative framing (cross-renderer comparison on the same tuned avatar) is not what the issue describes.
