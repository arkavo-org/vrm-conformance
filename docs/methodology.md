# Methodology

This document records the cross-renderer comparison hazards every test plan must account for. These are not opinions; they are observable sources of pixel-level divergence that have nothing to do with whether a renderer correctly implements the VRM spec.

## Color management

MToon's spec is silent on the renderer's display-encoding workflow. Each render submission **must** declare its `color_space` field, and the meaning is pinned here:

- `color_space: Srgb` — **the v1.0 default for every MToon math test.** Shading runs in linear color space; the sRGB OETF is applied on output. The PNG is sRGB-encoded.
- `color_space: Linear` — shading runs in linear color space; **no** OETF on output. The PNG carries raw linear values. Diagnostic-only — use to inspect a renderer's pre-display values; do not compare across renderers.

The two conventions are **not interchangeable** under SSIM. Test plans must not mix them within a corpus, and adapters must honor the declared field exactly.

### Why `Srgb` is the default

When pixiv/three-vrm clarified [#1838](https://github.com/pixiv/three-vrm/issues/1838), the contributor noted that three-vrm's MToon implementation only produces spec-intended output when `THREE.SRGBColorSpace` is set (which applies the sRGB OETF after shading). `THREE.LinearSRGBColorSpace` is explicitly unsupported for MToon — three.js's "Linear" output corresponds to the *absence* of the display-encoding step, not Unity's "Linear" workflow (which means "linear shading + sRGB output"; the three.js equivalent is `SRGBColorSpace`).

The conformance suite's prior default (`color_space: Linear`) therefore asked three-vrm to render in an unsupported mode, producing a corpus baseline that under-represented three-vrm's MToon math by the sRGB OETF. The v1.0 default is now `Srgb` to:

1. Align with three-vrm's documented expectation.
2. Match godot-vrm's PNG output (Godot writes sRGB-encoded PNGs unconditionally; the renderer can't be asked for raw-linear without a custom export path).
3. Keep VRMMetalKit in the same regime via `rgba8Unorm_srgb`.

### Adapter contract for `color_space`

| Adapter | `Srgb` (default) | `Linear` |
|---|---|---|
| three-vrm | `renderer.outputColorSpace = THREE.SRGBColorSpace` | `THREE.LinearSRGBColorSpace` — diagnostic only, MToon math not spec-conformant |
| vrm-metal-kit | `rgba8Unorm_srgb` | `rgba8Unorm` |
| godot-vrm | sRGB-encoded PNG (native) | request honored at the API, but Godot still writes sRGB-encoded PNG; `actual_color_space: "Srgb"` is reported back |

Adapters return `actual_color_space` in their render response so the runner can flag any deviation from the declared plan.

### Directional intensity convention (resolved in run 10)

three.js since r155 uses physically-correct intensity scaling — a `DirectionalLight` with intensity `1.0` is dimmer by a factor of π than the same setting in legacy three.js or in Unity URP. three-vrm's spec-intended MToon baseline ([pixiv/three-vrm#1838](https://github.com/pixiv/three-vrm/issues/1838)) assumes `intensity = Math.PI`.

**Convention** (v1.0): Test plans declare `directional.intensity = 1.0` as the canonical value. Per-adapter compensation:

| Adapter | Scaling applied |
|---|---|
| three-vrm | `state.directional.intensity = d.intensity * Math.PI` — required because three.js's `DirectionalLight` uses lux (physically-correct). |
| vrm-metal-kit | None — VRMMetalKit's lighting model already produces output consistent with intensity 1.0 = "Unity URP intensity 1.0" semantics. |
| godot-vrm | None — Godot 4's `DirectionalLight3D.light_energy` defaults match the same convention. |
| univrm (in design) | TBD — verify at L3 implementation time against UniVRM's lighting setup. |

Resolved in run 10 (`docs/findings.md`) after measuring that applying `Math.PI` in the three-vrm adapter moved `mtoon_default` centerline from `(126,126,126)` to `(195,195,195)`, the `godot-vrm vs three-vrm` corpus pair mean from 0.8398 to 0.8972 (+0.0574), and the corpus max SSIM above 0.99 for the first time on non-outline tests. The scaling is renderer-specific compensation — done at the adapter boundary, not in the test plan — so adapters for renderers that don't use physically-correct intensity (most non-three.js renderers) require no change.

## Tone mapping

Host engines apply tone mapping at varying defaults. three.js's `WebGLRenderer.toneMapping` defaults to `NoToneMapping`; Godot 4 `Environment.tone_mapper` defaults to `Linear`; Unity URP/HDRP applies tone mapping via Volume settings.

**MToon math tests pin `tone_mapping: none`.** MToon is non-PBR. ACES, Filmic, or Reinhard mangle the intended output cross-renderer. Integration tests opt into other tone-mapping modes with relaxed tolerances.

## Engine shadow noise

Differences in shadow bias, PCF filtering, and cascade resolution between Unity / Godot / three.js / Metal create shadow-edge noise that SSIM flags as failures even when MToon math is correct.

**MToon math tests pin `cast_shadows: false` and `receive_shadows: false`.** Shadow-on integration tests are a separate category with renderer-pair tolerance bands.

## Outline antialiasing

Outlines render via separate pass (most), geometry shader (some), or screen-space (rare). Aliasing differs.

**v1.0 standardizes on MSAA 4x.** SSIM uses a wider local tolerance band on outline regions.

### Outline at parameter extremes — methodology exclusion (per vrm-conformance#3)

For outline tests at width ≥ 0.05 m on the 30-cm-radius test sphere, the spec-mandated inverted-hull rendering produces a near-fully-flooded mesh. Whole-frame SSIM measures *only* silhouette anti-aliasing on these renders — there is no main-mesh interior signal to compare. Empirical evidence: three-vrm and UniVRM (the consortium reference) agree at **SSIM 0.9988** on `mtoon_outline_world_0p1` (essentially pixel-identical), while VRMMetalKit sits at 0.6315 on the same test — entirely from MSAA sample-pattern differences. Reading 0.63 as "VMK fails outline rendering" misreads the metric.

**`mtoon_outline_world_0p05`, `_world_0p1`, `_screen_0p05`, and `_screen_0p1` are emitted with `diff.conformance_status: Excluded`.** They render (so visual regressions are still caught) but don't count toward the corpus's headline conformance pass-rate. Outline tests at width ≤ 0.03 (where the outline is a thin band and the main mesh interior is visible) remain `Included`.

A ring-band SSIM mode (compare only the annulus between expected main-mesh silhouette and expected outline-shell silhouette) is the proper long-term fix; until then, exclusion + visual inspection is the methodology-correct call.

## Conformance threshold

The v1.0 conformance threshold is **SSIM ≥ 0.85 against the consortium reference (UniVRM)**, per-test, with `mtoon_rimLightingMix_*` at the tighter 0.95 (no engine-residual cluster on those tests). Per [vrm-conformance#2](https://github.com/arkavo-org/vrm-conformance/issues/2).

**This is not the old 0.985.** That earlier threshold was scoped for "this renderer produces byte-stable output across runs" (self-diff stability). It is not achievable cross-renderer between independent MToon implementations — even three-vrm and UniVRM (the closest pair) only cross 0.985 on 12% of tests. The 0.85 operational threshold reflects the engine-level residual cluster (silhouette AA + sRGB OETF rounding + projection-matrix float differences) that every cross-renderer corpus carries by construction.

Future tightening (post-1.0) can ratchet the MToon-material threshold toward 0.90 once dedicated engine-alignment work (MSAA sample-pattern matching, color-pipeline alignment) is in scope. The threshold for a 1.0 conformance claim is 0.85; tighter would over-claim.

## Spring bone determinism

`VRMC_springBone` does not pin a fixed time-step. Adapters must guarantee deterministic stepping at 60 Hz with reset between tests.

**The golden (UniVRM) does not substep** — its FastSpringBone runs a *single* Verlet step per frame at frame-rate `deltaTime` (`FastSpringBoneScheduler.Schedule(Time.deltaTime)`, no accumulator, no clamp; our harness drives it at 1/60). Consequence for collision conformance: stiff-chain collision is a large-`dt` instability, and at 60 Hz single-step the reference is *more* susceptible to it, not less. A renderer that substeps (e.g. 240/480 Hz) will resolve a stiff-chain–vs-collider contact more stably than UniVRM does — that is a **divergence *above* the reference**, a deliberate quality choice to be logged and frozen-baselined, **not** a conformance win. Matching UniVRM means matching its single-step behavior, catapult included. See `docs/findings.md` 2026-06-06 (#313 Track 2) for the worked case.

**Asset emission (Phase 2D-a)**: the asset generator's `emit-springbone` subcommand produces VRM 1.0 assets with `VRMC_springBone` chains attached to the head bone. Each chain is parametrized by `SpringBoneParams` (joint_count, segment_length_m, stiffness, drag_force, gravity_power, gravity_dir, hit_radius). The renderer-side `step_physics` / `reset_physics` / `animate_root_transform` ops that exercise these assets land in 2D-b.

**Sweep emission (Phase 2D-c)**: `emit-springbone-sweep` produces the full one-axis-at-a-time parameter sweep (~18 assets) so a regression in any single dimension (joints, stiffness, drag, gravity, segment length) can be pinned without confounding. Default MToon is held constant across all variants; the spring-bone axis is what is under test.

## Spring bone initial state

Renderers initialize spring positions differently from a fresh load. The `reset_physics(settle_steps)` operation pins the convention: every spring-bone test runs N settling steps from rest pose before measurement begins.

**v1.0 default: 30 settle steps at 60 Hz (0.5 s).**

## Spring bone excitation

Static avatars under `step_physics` only exercise gravity settling. Testing inertia, drag, stiffness requires moving the avatar through space. The `animate_root_transform(start, end, duration_seconds, fps)` operation drives this.

**Implemented in Phase 2D-d**: the op is real in the three-vrm adapter (linear-interp root translation, `vrm.update(1/fps)` between samples) and acknowledged as a no-op by the mock. Test plans with an `animation.root_transform` block trigger the op between `reset_physics` and `render`, so the rendered frame captures the chain in motion rather than at the static settle.

**Sweep corpus (Phase 2D-e)**: `emit-springbone-swing-sweep` emits a parallel 18-asset corpus where every plan carries an `animation.root_transform` block (15 cm sideways translation over 0.25 s @ 60 Hz, after the standard 30-step settle). The corpus exercises every spring-bone axis under inertia rather than only at equilibrium — a regression in any renderer's drag, stiffness, or chain-length handling now surfaces against the swing reference.

## Spring bone position-diff thresholds

Cross-renderer SSIM is necessary but not sufficient for spring-bone tests: two valid renderers can produce visibly different chain poses because collision response and time-integration are not pinned by the spec. The `dump_bone_positions` op exposes per-joint world coordinates so position divergence can be measured directly.

Two thresholds — single-joint outliers and chain-wide drift are different bug shapes:

| context | per-joint tolerance | chain-summed tolerance |
|---|---|---|
| settle (no `animate_root_transform`) | 5 mm | 20 mm |
| swing (with `animate_root_transform`) | 10 mm | 40 mm |

Settle thresholds reflect that two correctly-converged renderers should agree to within sub-cm at equilibrium. Swing thresholds widen because sub-frame stepping divergence accumulates during animation.

`vrm-runner execute-test-plan --reference-positions <renderer>=<positions.json>` runs the diff. `vrm-runner consensus-diff --render-positions <name>=<path>` produces N-way outlier flagging.

These thresholds are operational, not spec-defined. Future tightening follows the same trajectory as the cross-renderer SSIM thresholds.

### CCD / `penetration-diff` position-capture support (per-adapter)

The position-based collision metric (`vrm-runner penetration-diff`, the `ccd_colliders` plans, and the `spring_bone_ccd_sweep` corpus) consumes per-frame joint world positions, captured via `render_sequence` with `capture_positions: true` (→ `SequenceFrame.spring_positions`).

Adapter support:

- **`vrm-mock-renderer`** — implements capture, but as a hardcoded **static** 4-joint chain (`handlers.rs::synthetic_spring_chain`): no loader, no shader, no physics (`render.rs`: *"Not a renderer in any meaningful sense"*); ignores the chain's stiffness/drag/gravity, the colliders, and `root_translation`. A mock penetration run validates **pipeline wiring only** — it cannot observe tunneling or tunnel-prevention (no integrator to tunnel; the static chain at x=0 never approaches the x=0.10 swept collider). A "0 penetration" mock result is structural, not evidence CCD works.
- **`godot-vrm`** — **implements capture against the real L4 solver** (as of 2026-06-06; `session.gd::_collect_spring_positions`, shared with `dump_bone_positions`). First real-engine CCD measurement: `docs/findings.md` 2026-06-06 (chain passes through the world-fixed colliders undeflected; depth ∝ radius). This is the proof the metric works against a real spring-bone simulation, not just the static mock.
- **UniVRM (golden)** — **implements capture in the PlayMode batch** (2026-06-06; `BatchRunner.CaptureSpringPositions`). Establishes the oracle CCD baseline: the golden **deflects fast sweeps** (radius-responsive push-out) but **penetrates slow sustained-contact sweeps** (single-step positional collision vs gravity load). So the absolute `penetration-diff` `passed` flag is *not* the conformance bar — UniVRM fails it on slow cells; conformance is "match UniVRM." See `docs/findings.md` 2026-06-06 (golden baseline). Adapter wire shape is flat `float[]` (JsonUtility can't nest); runner reshapes to canonical `[[x,y,z]]`.
- **three-vrm, VMK** — implement single-frame `dump_bone_positions` but do **not** yet populate per-frame `spring_positions` in `render_sequence`. Cheap follow-ups (reuse their existing extraction per frame, the godot pattern).

The image-SSIM consensus path (settle/swing sweeps, `docs/findings.md` 2026-05-29) is independent of all this and drives the real engines for fidelity; the position/penetration path is the one whose real-engine backing is being filled in adapter by adapter.

### Exception: VRM 0.x leaf-tail sweep is a 2-factor grid

The basic sweeps are one-axis-at-a-time to keep regressions un-confounded. The
`sb0_leaftail_*` family (`spring_bone_v0_leaftail_sweep`) is a deliberate
**orientation × length** grid: the VRM 0.x leaf-tail (7 cm) synthesis error this
family targets is *defined* by that interaction (off-vertical short chains
collapse; vertical or long chains tolerate the same error). The confound is the
object of study. All cells are zero-gravity static (settle 30, no animation,
`tone_mapping: none`) so the measured signal is pure synthesis rest, not gravity.
Spec: `docs/upstream-specs/vrm-specification/specification/VRMC_springBone-1.0/README.md:137-153`.
Surfacing symptom: [VRMMetalKit#306](https://github.com/arkavo-org/VRMMetalKit/issues/306).

Generate with `emit-springbone-leaftail-sweep --spec-version 0.x` (synthesized
tail) and again `--spec-version 1.0` (explicit 7 cm `_end` on the two
`*_parity_*` cells) for the 0.x↔1.0 parity twins.

**Known limitation:** the `chain_axis` orientation knob is exact for single-chain
assets (the chain hangs from `head`, which sits at X=Z=0). For the *multichain*
emit path, chains hang from an intermediate node offset radially around `head`;
the inverse-bind root ignores that XZ offset, so a non-default `chain_axis` on a
multichain asset has an approximate (offset) rest. This is pre-existing behavior
generalized, not used by the leaftail sweep (which is single-chain), and is noted
only so a future multichain+off-axis test is not built on it unaware.

## Render queue / transparency ordering

Z-write behavior under `transparentWithZWrite=true` plus `renderQueueOffsetNumber` is the most common source of real-world MToon visual bugs.

A dedicated test category covers `outline × alphaMode × transparentWithZWrite × renderQueueOffsetNumber` interactions; coverage there is disproportionately heavy on purpose.

## Tangent space

The spec allows ignoring stored TANGENT and recomputing via MikkTSpace. Recomputation differs subtly across libraries. v1.0 generates assets both with and without explicit tangents.

## glTF-core `occlusionTexture` is not applicable to MToon materials

Confirmed 2026-05-23 against the UniVRM reference (`docs/findings.md` "glTF-core PBR textures on MToon"). The PBR-textures sweep's three occlusion variants (`mtoon_pbrtex_baseline`, `mtoon_pbrtex_occlusion_default`, `mtoon_pbrtex_occlusion_strength_half`) render to a byte-identical PNG on **all four reference renderers** (UniVRM `9ed71e6798c4`, vrm-metal-kit `5d8cf1789282`, three-vrm `6ff1f5687375`, godot-vrm `4587bf323df1`).

UniVRM is the consortium reference and its behavior is authoritative for ambiguous spec questions. The MToon spec (`docs/upstream-specs/vrm-specification/specification/VRMC_materials_mtoon-1.0/README.md`) explicitly declares MToon a non-PBR toon shader; the absence of `occlusionTexture` honoring is intentional ecosystem-wide rather than a per-renderer bug.

**Conformance treatment.** The `mtoon_pbrtex_occlusion_*` sweep variants remain in the corpus as tripwires (a renderer that suddenly starts honoring `occlusionTexture` should be flagged as having diverged from the consortium reference), but the consensus pass-rate does not penalize renderers for omitting the AO multiplier on MToon. Asset emission keeps the binding in the corpus so the suite documents what the spec allows; conformance evaluation treats per-renderer agreement-on-omission as the expected behaviour.

Note: this does NOT extend to `normalTexture`. UniVRM and three-vrm both apply the normal-map `scale` field correctly on MToon materials, so `normalTexture` is on the conformance hook even though it's also a glTF-core PBR binding. See [VMK#290](https://github.com/arkavo-org/VRMMetalKit/issues/290) for the open VMK gap (texture read, but `scale` ignored).

## Apple Silicon vs other GPUs

VRMMetalKit is Metal-only; cross-GPU pixel-exact comparison is a non-goal. SSIM thresholds are tuned per-pair, with stricter intra-family thresholds (same GPU vendor, same color space) and looser cross-family thresholds. **Property assertions remain strict across all pairs.**

## Sequence captures (multi-frame `render_sequence`)

Adopted by [RFC-0004](../rfcs/0004-render-sequence-op.md). These pins apply to any test plan with a `render_sequence:` block; single-frame `render:` tests are unaffected.

**Physics floor.** `physics_dt_seconds <= 1.0 / 60.0`. Anything coarser violates the spring-bone determinism pin. Adapters SHOULD reject coarser values with `-32602 invalid params`; the runner SHOULD pre-validate before dispatch.

**Sampling clock.** Sequence captures with `apply_vrma` set MUST sample the `.vrma` at `t = start_seconds + (i / frame_hz)` — display clock drives sampling; the physics clock is internal to the adapter. This decoupling exists so a test can capture at 30 Hz while running spring-bone physics at 60 Hz (two physics steps per captured frame).

**No temporal alignment.** Per-frame SSIM compares same-index frames only. The runner does NOT attempt temporal alignment (no DTW, no frame-offset search). If two adapters produce equivalent trajectories at different timings, the test's `physics_dt_seconds` or `frame_hz` is wrong, not the diff.

**Pass criteria.** Default: `mean_ssim >= temporal_ssim_threshold AND min_ssim >= temporal_ssim_threshold - 0.05 AND frame_count_match == true`. The 0.05 single-frame relaxation acknowledges that a one-frame transient (e.g. a settle-tick offset by a single physics step) shouldn't fail an otherwise-conforming sequence; a frame-count mismatch between renders fails the test unconditionally regardless of SSIM. Per-test thresholds via the existing `vrm-conformance#2` mechanism.

**Worst-frame reporting.** Every sequence diff result MUST surface `worst_frame_index` so site reviewers can land on the divergent frame directly. A single bad frame in a 60-frame sequence is fine if mean SSIM holds; the threshold relaxation handles this.

**Output format.** PNG sequence is the canonical contract format. MP4/MOV are convenience formats for site display and reviewer ergonomics — the diff engine consumes the per-frame PNGs regardless. Adapters that emit only PNG sequences are spec-compliant; the bootstrap script can mux post-hoc via `ffmpeg`.

## VRM 0.x conformance

The corpus exercises VRM 0.x assets in parallel with VRM 1.0. Spec-version metadata threads through the manifest, test plan, and runner (`spec_version: "0.x" | "1.0"`); the runner enforces the version-specific methodology pins below.

### Camera convention (per-spec-version)

The two spec versions specify **opposite** default avatar orientations in glTF coordinates:

- **VRM 0.x:** avatar faces -Z per `specification/0.0/README.md:238` ("Model faces towards -Z direction"). Test plans place the camera at -Z (target = origin) to see the front of a spec-conformant render.
- **VRM 1.0:** avatar faces +Z per `specification/VRMC_vrm-1.0/tpose.md` Definition 1.1. Test plans place the camera at +Z.

The runner enforces this — a test plan declaring `spec_version: "0.x"` with a camera at positive Z is rejected with a clear error (see `validate_camera_convention` in `crates/vrm-runner/src/execute.rs`).

### Coordinate-frame normalization at adapter load time

Empirical finding from slice 1 (recorded in `docs/findings.md` 2026-05-26): two of the four real adapters perform **load-time coordinate normalization** of VRM 0.x assets into VRM 1.0 / glTF coordinate space:

- **VRMMetalKit (VMK):** `VRMModel.buildNodeHierarchy()` conjugates 0.x TRS into VRM 1.0 / glTF right-handed space when `isVRM0`, with a companion `applyVRM0InverseBindMatrixConjugation()` for skin inverse-bind-matrix consistency. Intentional and load-bearing.
- **UniVRM:** `Vrm10.LoadPathAsync(path, canLoadVrm0X: true, …)` enables an in-library 0.x → 1.0 migration path. Similar normalization shape.
- **three-vrm + godot-vrm:** preserve the source spec's coordinate frame; no load-time normalization.

This is **not a conformance defect.** The suite's camera placement uses VRM 0.x's spec-correct -Z convention. Adapters that normalize internally still render correctly through this camera because "forward" is preserved across the normalization. Adapters that don't normalize render correctly directly. All four adapters should converge on the same visual output for a 0.x asset with a -Z-placed camera.

The conformance signal that this design surfaces is **whether all four adapters agree** on the rendered output, not whether any specific adapter applies or skips a rotation.

### `source_spec_version` reporting contract

Every adapter dump response (`dump_humanoid_pose`, `dump_expression_weights`, `dump_look_at_state`) carries a required `source_spec_version: "0.x" | "1.0"` field, echoing what the adapter parsed from the loaded asset's `extensionsUsed` array. Adapters that normalize internally (VMK, UniVRM) still report the **original** spec version, not the post-normalization shape — the field documents what the asset **was**, not what the renderer is rendering it **as**.

The runner cross-checks the adapter-reported `source_spec_version` against the test plan's declared `spec_version`. Mismatch aborts the run with a clear error — the third hard-error gate in the three-way `spec_version` cross-check:

1. Test plan ↔ manifest (Task 5: `validate-manifest` cross-checks test_id naming against `spec_version` field).
2. Test plan camera ↔ `spec_version` (Task 25: `validate_camera_convention`).
3. Test plan ↔ adapter-reported `source_spec_version` (Task 26: `cross_check_source_spec_version`).

### Normalization is one-directional and lossy

The runner can normalize 0.x dumps to a 1.0-equivalent shape via the `vrm-normalize` crate (called by the runner via `apply_normalization_if_requested`; adapters do not implement normalization themselves — single bug surface per the design).

Normalization is requested via the optional `as_spec_version` request param on dump ops:

- **Absent (default)**: adapter returns the dump in its **native** spec-version shape — never normalize unless asked.
- **`"1.0"` against a 0.x asset**: runner normalizes via `vrm-normalize` (joy → happy preset mapping, weight 0–100 → 0–1, etc.).
- **`"0.x"` against a 1.0 asset**: rejected with error `-32001 NormalizationDirectionUnsupported`. v1 → v0 has no lossless mapping for some v1-only presets (`surprised`, etc.).
- Custom blendshapes without a v1 preset equivalent pass through with `custom:<name>` markers, never dropped.

The canonical v0 → v1 preset mapping table:

| v0 (`blendShapeMaster.presetName`) | v1 (`VRMC_vrm.expressions.preset`) |
|---|---|
| `joy` | `happy` |
| `angry` | `angry` |
| `sorrow` | `sad` |
| `fun` | `relaxed` |
| `neutral` | `neutral` |
| `a`, `i`, `u`, `e`, `o` | `aa`, `ih`, `ou`, `ee`, `oh` |
| `blink` / `blink_l` / `blink_r` | `blink` / `blinkLeft` / `blinkRight` |
| `lookup` / `lookdown` / `lookleft` / `lookright` | `lookUp` / `lookDown` / `lookLeft` / `lookRight` |
| custom (any other) | `custom:<original-name>` |

Weight range conversion: v0 uses Unity-convention 0–100; v1 uses glTF-convention 0–1. Normalization divides by 100.

### Sweep registry symmetry

Every `*_v0` sweep entry has a 1.0 counterpart in the registry, OR is registered with a structured `NotApplicable { reason: <NotApplicableReason> }` when the axis doesn't apply to 0.x. The structured reason enum (defined in `crates/vrm-asset-generator/src/lib.rs`) makes absence queryable rather than free-text. Slice 1's MToon `mtoon_basic_v0_outline_lighting_mix` is the canonical example: registered as `NotApplicable { reason: OutlineLightingMixV1Only }` because 0.x has no `_OutlineLightingMix` Unity-shader key.

A compile-time invariant test (`sweep_registry_symmetric_across_versions` in `sweep.rs`) enforces this — every `Applicable` v0 entry must have a v1 counterpart, OR be explicitly `NotApplicable` with a reason.

### v0-specific quirk sweeps (slice 2+)

The `_v0_quirk_*` sweep prefix is reserved for slice 2's intentional probes of 0.x spec corners that adapters sometimes silently correct:

- `stiffinessForce` — canonical typo in the 0.x spec spring-bone field. An adapter that "fixes" the typo by also accepting `stiffness` is silently non-conformant.
- centerNode-as-transform vs centerNode-ignored.
- Single-bone-per-group spring-bone topology.
- Sphere-collider-only enforcement (capsule colliders must be rejected on 0.x, not silently handled).
- 0.x `firstPerson` flagging semantics.
- 0.x meta schema (`licenseName: CC0` as a string vs 1.0's structured `meta.licenseUrl`).

These exist explicitly to surface adapter behavior on the weird parts of 0.x.

### Spring-bone cross-version triage order (read within-renderer cross-version first)

When a spring-bone sweep diverges between the 0.x and 1.0 corpora, triage in the
**reverse** of the usual cross-renderer-first reflex: read **within-renderer,
cross-version first** (e.g. VMK 0.x vs VMK 1.0 on the same axis), and only then
read cross-renderer.

Rationale: spring-bone simulation is integrator-sensitive — renderers legitimately
differ in integration scheme (Verlet vs semi-implicit Euler), sub-stepping, and
the order damping is applied. So a *cross-renderer* disagreement at a fixed spec
version is often just integrator variance, not a conformance defect. But a
*within-renderer, cross-version* disagreement — the same engine, same integrator,
fed our 0.x `secondaryAnimation` emit vs our 1.0 `VRMC_springBone` emit of the
**same** `SpringBoneParams` — isolates a coordinate/unit/field-mapping bug in one
of our two emit paths (e.g. a `gravityDir` sign flip, the `stiffiness`-vs-`stiffness`
field-name mismatch, or a degrees/radians error), because the only thing that
varied is the extension we emitted. That is the cleaner falsification; read it first.

See `docs/superpowers/specs/2026-05-26-vrm-0x-conformance-design.md` (Slice 2).

### What slice 1 does NOT cover

Out of scope for slice 1 (per `docs/superpowers/specs/2026-05-26-vrm-0x-conformance-design.md`):

- Round-tripping (parsing 0.x assets). Trigger to revisit the single-crate generator decision if this becomes a goal.
- Side-channel "native orientation" render as a supplementary artifact. Deferred to v2 — the back-of-head failure mode (if it materializes) is already legibly diagnostic on its own.
- VRM 1.1 plumbing. The `SpecVersion::{V0, V1}` enum extends cleanly when 1.1 lands; sweep registry stays.
- VRMA × 0.x. Slice 3.
- Spring-bone v0 parametric (`secondaryAnimation` emit). Slice 2 — **now implemented** (`spring_bone_v0.rs`; settle/swing/collider-sphere/multichain/sequence sweeps route through `--spec-version 0.x`).
- Full MToon parametric parity (44 variants × 0.x). Slice 2 — **now implemented** (all applicable MToon/texture sweeps accept `--spec-version 0.x`; the v1-only axes reject it with a structured `NotApplicableReason`).

## Face culling honors `material.doubleSided`, not material name

glTF 2.0 and VRM make `material.doubleSided` the **sole authority** on back-face
culling. A conformant renderer MUST NOT change culling, depth bias, or render
category based on substrings of the *material name* (e.g. `cloth`, `skirt`,
`tops`, `body`). Material name is metadata, not a rendering directive.

**Why this is a pin.** VRMMetalKit classifies any material whose name contains a
clothing token as `faceCategory = "clothing"` (`VRMRenderer.swift:1866`), then
unconditionally forces `effectiveDoubleSided = true`
(`VRMRenderItemBuilder.swift:216`) — ignoring the glTF `doubleSided` flag — and
applies an overlay depth bias (`slopeScale 2.0`) intended for layered
VRChat-style avatars. On a single-material VRM 0.0 outfit named `Vita_clothing`
this produces silhouette z-fighting fringe (the slope-scaled bias explodes
edge-on) and dark backface bleed (inward-normal faces drawn by the forced
double-siding). This is the same defect *class* as orientation-from-heuristic
(VMK 180° flip, VMK#299): a name/heuristic overriding declared spec data.

**How the suite catches it.** The `material_name_classification` sweep
(`emit-material-name-classification-sweep`) emits one MToon material under
heuristic-tripping names (`matname_clothing_*`, `matname_skirt_*`) and control
names (`matname_plain_*`, `matname_body_*`) crossed with the glTF `doubleSided`
flag. Every variant is byte-identical except its material name and `doubleSided`.
**Conformant output is invariant to the material name at a fixed `doubleSided`**
(consensus SSIM ≈ 1.0 among conformant renderers); a name-heuristic renderer
diverges on the trip-token variants. This isolates the name-classification
defect deterministically on the standard sweep sphere — no GPU, model file, or
humanoid geometry required, and distinct from the orientation finding (a sphere
is rotationally symmetric, so the 180° flip does not confound this comparison).

Renderers MAY use material name for *non-visible* optimizations only (e.g.
batching hints) — never for anything that changes pixels.
