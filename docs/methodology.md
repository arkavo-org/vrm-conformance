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

## Render queue / transparency ordering

Z-write behavior under `transparentWithZWrite=true` plus `renderQueueOffsetNumber` is the most common source of real-world MToon visual bugs.

A dedicated test category covers `outline × alphaMode × transparentWithZWrite × renderQueueOffsetNumber` interactions; coverage there is disproportionately heavy on purpose.

## Tangent space

The spec allows ignoring stored TANGENT and recomputing via MikkTSpace. Recomputation differs subtly across libraries. v1.0 generates assets both with and without explicit tangents.

## Apple Silicon vs other GPUs

VRMMetalKit is Metal-only; cross-GPU pixel-exact comparison is a non-goal. SSIM thresholds are tuned per-pair, with stricter intra-family thresholds (same GPU vendor, same color space) and looser cross-family thresholds. **Property assertions remain strict across all pairs.**

## Sequence captures (multi-frame `render_sequence`)

Adopted by [RFC-0004](../rfcs/0004-render-sequence-op.md). These pins apply to any test plan with a `render_sequence:` block; single-frame `render:` tests are unaffected.

**Physics floor.** `physics_dt_seconds <= 1.0 / 60.0`. Anything coarser violates the spring-bone determinism pin. Adapters SHOULD reject coarser values with `-32602 invalid params`; the runner SHOULD pre-validate before dispatch.

**Sampling clock.** Sequence captures with `apply_vrma` set MUST sample the `.vrma` at `t = start_seconds + (i / frame_hz)` — display clock drives sampling; the physics clock is internal to the adapter. This decoupling exists so a test can capture at 30 Hz while running spring-bone physics at 60 Hz (two physics steps per captured frame).

**No temporal alignment.** Per-frame SSIM compares same-index frames only. The runner does NOT attempt temporal alignment (no DTW, no frame-offset search). If two adapters produce equivalent trajectories at different timings, the test's `physics_dt_seconds` or `frame_hz` is wrong, not the diff.

**Pass criteria.** Default: `mean_ssim >= temporal_ssim_threshold AND min_ssim >= temporal_ssim_threshold - 0.05`. The 0.05 single-frame relaxation acknowledges that a one-frame transient (e.g. a settle-tick offset by a single physics step) shouldn't fail an otherwise-conforming sequence. Per-test thresholds via the existing `vrm-conformance#2` mechanism.

**Worst-frame reporting.** Every sequence diff result MUST surface `worst_frame_index` so site reviewers can land on the divergent frame directly. A single bad frame in a 60-frame sequence is fine if mean SSIM holds; the threshold relaxation handles this.

**Output format.** PNG sequence is the canonical contract format. MP4/MOV are convenience formats for site display and reviewer ergonomics — the diff engine consumes the per-frame PNGs regardless. Adapters that emit only PNG sequences are spec-compliant; the bootstrap script can mux post-hoc via `ffmpeg`.
