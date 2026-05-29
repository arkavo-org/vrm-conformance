# VRM 0.x leaf-tail rest-stability conformance (gap area; surfaced by VMK #306)

**Date:** 2026-05-29
**Status:** Design — awaiting review
**Relates to:** [VRMMetalKit #306](https://github.com/arkavo-org/VRMMetalKit/issues/306), RFC-0006 (VRM 0.x conformance)

## Framing

This is a **conformance gap**, not a single-library bug. VMK #306 (a collapsed
bust in the VRM 0.x path) is one symptom; the underlying spec behavior the suite
fails to exercise is *VRM 0.x leaf-tail synthesis across the space of chain
orientations and lengths*. We characterize the whole gap area so the suite flags
**any** renderer that mis-reconstructs the leaf-tail rest in **any** direction —
not just VMK, not just the bust. #306 is a single labeled cell in the resulting
sweep.

## Spec under test

The VRM 0.x leaf-tail rule is authoritative and unambiguous:

> `docs/upstream-specs/vrm-specification/specification/VRMC_springBone-1.0/README.md:137-153`
> For `vrm0`: when a chain's final joint has no child node, add a SpringJoint
> **7 cm out at the end** — extend a virtual tail 7 cm along the bone's own local
> rest axis.

Reference implementation confirms direction + magnitude
(`adapters/godot-vrm/addons/vrm/vrm_spring_bone.gd:103`):

```gdscript
var delta: Vector3 = skel.get_bone_rest(bone_idx).origin
pos = delta.normalized() * 0.07   # 7 cm along the bone's local rest axis
```

Two spec consequences define the gap:

1. **Rest invariance under zero gravity.** With `gravityPower=0` and no
   animation, a conformant solver settles the leaf to
   `bone_world_pos + normalize(local_rest_axis) * 0.07` — zero net deformation
   from the authored rest, in *every* orientation.
2. **0.x↔1.0 parity.** A 0.x chain whose leaf has no child (tail *synthesized*)
   must render the same as the identical geometry authored in 1.0 with the
   explicit `_end` tail node placed 7 cm along the same axis. The 0.x synthesis
   is defined to reproduce the explicit tail. This is exactly #306's own
   invariant ("same model, 0.x vs 1.0, should render identically"), generalized.

The error class is **direction-dependent**: a vertical chain's leaf axis ≈ the
gravity/default axis, so a wrong-direction synthesis lands near-correct
(tolerated); off-vertical chains make the wrong direction produce a large rest
error (collapse). Length modulates visibility (short chains amplify the per-joint
error; long chains average it out). The gap is precisely the
**orientation × length interaction**, so the corpus must span both.

## Why the current corpus misses the entire gap

Verified against source:

- One-axis-at-a-time sweep never combines short + zero-gravity:
  `springbone_joints_2` runs at default `gravity=0.5` (`spring_bone.rs:80`);
  `springbone_gravity_0` runs at default 4 joints (`spring_bone.rs:76`).
- Chain geometry is hard-coded straight down −Y (`humanoid.rs:209`) — the *only*
  orientation tested is the one the spec error tolerates. The off-vertical
  region of the gap is entirely unexercised.
- No 0.x↔1.0 parity check on identical geometry exists.
- Pass/fail is consensus/reference-based, so the defect can only surface as a
  rendered divergence — and today no asset produces a frame where it appears.

## Goals

1. Exercise VRM 0.x leaf-tail synthesis across the **orientation × length** input
   space at zero gravity, so any renderer's direction-dependent synthesis error
   is caught.
2. Add a **0.x↔1.0 parity** axis: same geometry, synthesized tail vs explicit
   tail, must render alike.
3. Keep it parametric / CC0; preserve every existing asset byte-for-byte.
4. Label the bust-analog cell (`+Z`, short, gravity=0) so #306 has a named
   regression anchor without the sweep being VMK-specific.

## Non-goals

- An absolute, renderer-independent rest-position assertion (needs cross-renderer
  bone-transform readout absent from the op catalog). #306 already proposes that
  as a **VMK-side unit test**; the conformance layer provides the cross-impl
  signal, not VMK's internal check.
- Per-joint / non-uniform geometry (true bust mesh). Chain *orientation* captures
  the direction-dependence; per-joint offsets add nothing to the gap (YAGNI).
- Routing the real `AvatarSample_K` VRoid asset (non-parametric, not
  CC0-generatable).

## Design

### 1. New geometry primitive: `chain_axis`

One field on `SpringBoneParams` (`spring_bone.rs`):

```rust
/// Unit direction the chain extends from its root, in the root bone's local
/// space. Default [0,-1,0] (straight down) reproduces all pre-existing assets
/// byte-for-byte. Off-vertical axes exercise the direction-dependent leaf-tail
/// synthesis the VRM 0.x spec mandates.
#[serde(default = "default_chain_axis")]
pub chain_axis: [f32; 3],
```

`default_chain_axis() -> [f32;3] { [0.0,-1.0,0.0] }`, same default in
`SpringBoneParams::defaults`. Thread through the three hard-coded −Y sites:

- **`humanoid.rs:209`** — joint translation = `chain_axis * segment_length_m`.
- **`chain_mesh.rs`** — cylinder generated along canonical +Y, rotated to align
  with `chain_axis` (vertices + normals); ring→joint weighting unchanged.
- **`emit.rs`** — per-joint inverse-bind matrices generalized from cumulative −Y
  to cumulative `chain_axis * segment_length_m`.

The leaf stays a real childless node (`humanoid.rs:222`) so the 7 cm synthesis is
forced. **Invariant:** `chain_axis=[0,-1,0]` ⇒ byte-identical to today (regression
test).

### 2. Explicit-tail emit for the 1.0 parity axis

Add an opt-in `explicit_tail: bool` (default `false`) to the 1.0 emit path: when
set, append one extra `_end` node 7 cm along `chain_axis` past the leaf and list
it in `VRMC_springBone.springs[].joints` (matching how VRoid 1.0 exports the
bust). Default `false` keeps existing 1.0 assets unchanged. This lets us emit the
*same* geometry two ways — 0.x synthesized vs 1.0 explicit — for the parity axis.

### 3. Sweep family `spring_bone_v0_leaftail_sweep()`

Neutral, spec-oriented prefix `sb0_leaftail_` (not a library name). Doc comment
ties it to the gap and cites #306 as the motivating symptom. All variants:
`gravity_power=0.0`, no animation, 30-step settle, `tone_mapping: none`, 0.x emit
(root-only `boneGroups` → forces synthesis) unless noted.

**Axis A — orientation** (core of the gap; short chain, 2 joints):
sample the orientation sphere — 6 cardinals + 2 diagonals.

| variant                       | chain_axis              |
|-------------------------------|-------------------------|
| `sb0_leaftail_axis_negY`      | `[0,-1,0]` (control)    |
| `sb0_leaftail_axis_posY`      | `[0,1,0]`               |
| `sb0_leaftail_axis_posZ`      | `[0,0,1]`  ← **#306 anchor** |
| `sb0_leaftail_axis_negZ`      | `[0,0,-1]`              |
| `sb0_leaftail_axis_posX`      | `[1,0,0]`               |
| `sb0_leaftail_axis_negX`      | `[-1,0,0]`              |
| `sb0_leaftail_axis_diagYZ`    | `[0,0.707,0.707]`       |
| `sb0_leaftail_axis_diagXZ`    | `[0.707,0,0.707]`       |

**Axis B — length interaction** (fixed off-vertical axis `+Z`):

| variant                    | joints |
|----------------------------|--------|
| `sb0_leaftail_len_2`       | 2 (== axis_posZ; emit once, alias) |
| `sb0_leaftail_len_4`       | 4      |
| `sb0_leaftail_len_8`       | 8      |

**Axis C — 0.x↔1.0 parity** (same geometry, two emits): for `+Z` short and `+Z`
long, additionally emit the 1.0 explicit-tail twin.

| variant                         | spec | tail        |
|---------------------------------|------|-------------|
| `sb0_leaftail_parity_short_v0`  | 0.x  | synthesized |
| `sb0_leaftail_parity_short_v1`  | 1.0  | explicit    |
| `sb0_leaftail_parity_long_v0`   | 0.x  | synthesized |
| `sb0_leaftail_parity_long_v1`   | 1.0  | explicit    |

~16 distinct geometries / ~18 assets — within the methodology's ~20-cell per-PR
budget.

> **Methodology exception (must be recorded in `docs/methodology.md`):** the basic
> sweeps are one-axis-at-a-time to avoid confounding. This family is a deliberate
> **2-factor grid** (orientation × length) because the gap *is* that interaction;
> the confound is the object of study. Document it as an explicit exception per the
> CLAUDE.md methodology-pin rule.

> Implementation note: chains attach to the `head` bone (~1.16 m). An off-vertical
> chain protrudes from the head — a clean SSIM silhouette. The plan verifies the
> generated camera (sidecar) keeps each orientation in-frame and that spec-correct
> vs collapsed poses are visually separable; if `+Z` foreshortens against the
> default camera, the camera is widened/repositioned rather than dropping axes.

### 4. Signal

Two cross-impl checks, both on the existing trust model (UniVRM = oracle,
consensus-diff flags outliers):

- **Consensus per cell** — render every `sb0_leaftail_*` through {UniVRM,
  godot-vrm, three-vrm, VMK}; `consensus-diff` per cell. Spec-correct renderers
  cluster; a synthesis-error renderer pops as outlier, and the *pattern* across
  orientation/length localizes the failure (e.g. VMK expected to diverge on
  off-vertical short cells, agree on `negY` and on long cells).
- **0.x↔1.0 parity** — per renderer, SSIM(`parity_*_v0`, `parity_*_v1`) must be
  high. A renderer whose 0.x synthesis ≠ its own 1.0 explicit tail fails parity
  even if it happens to be self-consistent across cells.

Log the full orientation×length result table to `docs/findings.md` (a
deliverable), citing #306 as the surfacing symptom and pointing VMK at their own
rest-stability unit test for the fix.

## Components & boundaries

- `SpringBoneParams` + `default_chain_axis`, `explicit_tail` (`spring_bone.rs`).
- `append_spring_chain` (`humanoid.rs`) — axis-parameterized placement.
- chain mesh builder (`chain_mesh.rs`) — axis-aligned cylinder.
- inverse-bind computation (`emit.rs`) — axis-parameterized IBMs; explicit `_end`
  node for the 1.0 parity twin.
- `spring_bone_v0_leaftail_sweep` (`spring_bone.rs`) + `emit-springbone-leaftail-sweep`
  subcommand.
- sidecar/test-plan emit (`sidecar.rs`) — pins gravity=0, settle 30, no animation,
  tone_mapping none.
- `docs/methodology.md` — record the 2-factor-grid exception.

Each unit independently testable: geometry has a byte-identity regression test +
off-axis placement test; sweep has a registry/count test; parity twins have a
"same axis/length, differing only in tail representation" test; rendered consensus
is a separate manual bootstrap.

## Testing

1. **Byte-identity regression** — `chain_axis=[0,-1,0]`, `explicit_tail=false`
   emits output identical to a committed pre-change asset.
2. **Off-axis placement** — `chain_axis=[0,0,1]` places joint *i* at cumulative
   `+Z * segment_length_m * i`; leaf has no children (synthesis forced).
3. **Explicit-tail emit** — `explicit_tail=true` (1.0) appends one `_end` node
   7 cm along `chain_axis` and lists it in `springs[].joints`.
4. **Sweep registry** — `spring_bone_v0_leaftail_sweep()` returns the expected
   axis/length/parity cells with gravity=0.
5. **Sidecar pins** — generated `.test.yaml` carries gravity=0, `settle_steps:30`,
   no `animation:`/`render_sequence:`, `tone_mapping:none`.
6. **Manual** — bootstrap goldens through the four real adapters; confirm the
   consensus table localizes per-renderer synthesis errors and the parity SSIMs.

## Rollout

1. Geometry primitives (`chain_axis`, `explicit_tail`) + tests (items 1–3).
2. Sweep + subcommand + sidecar pins + methodology note + tests (items 4–5).
3. Bootstrap goldens, run consensus + parity, write `findings.md` table (item 6).

Steps 1–2 are pure-Rust, CI-gated, no renderer dependency. Step 3 is the
cross-renderer evidence and the VMK-facing deliverable.
