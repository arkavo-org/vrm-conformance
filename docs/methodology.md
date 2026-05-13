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

### Open methodology question: directional intensity convention

three.js since r155 uses physically-correct intensity scaling — a `DirectionalLight` with intensity `1.0` is dimmer by a factor of π than the same setting in legacy three.js or in Unity URP. three-vrm's reference output for "intensity 1 directional light + sRGB output" implicitly assumes intensity is scaled by `Math.PI`. The conformance corpus currently declares `directional.intensity = 1.0` without specifying which convention applies. Tracking this as a follow-up — moving to a `Math.PI`-scaled convention would re-baseline three-vrm renders and likely move the corpus mean again. Filed as part of the [#1838](https://github.com/pixiv/three-vrm/issues/1838) close-out.

## Tone mapping

Host engines apply tone mapping at varying defaults. three.js's `WebGLRenderer.toneMapping` defaults to `NoToneMapping`; Godot 4 `Environment.tone_mapper` defaults to `Linear`; Unity URP/HDRP applies tone mapping via Volume settings.

**MToon math tests pin `tone_mapping: none`.** MToon is non-PBR. ACES, Filmic, or Reinhard mangle the intended output cross-renderer. Integration tests opt into other tone-mapping modes with relaxed tolerances.

## Engine shadow noise

Differences in shadow bias, PCF filtering, and cascade resolution between Unity / Godot / three.js / Metal create shadow-edge noise that SSIM flags as failures even when MToon math is correct.

**MToon math tests pin `cast_shadows: false` and `receive_shadows: false`.** Shadow-on integration tests are a separate category with renderer-pair tolerance bands.

## Outline antialiasing

Outlines render via separate pass (most), geometry shader (some), or screen-space (rare). Aliasing differs.

**v1.0 standardizes on MSAA 4x.** SSIM uses a wider local tolerance band on outline regions.

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

## Render queue / transparency ordering

Z-write behavior under `transparentWithZWrite=true` plus `renderQueueOffsetNumber` is the most common source of real-world MToon visual bugs.

A dedicated test category covers `outline × alphaMode × transparentWithZWrite × renderQueueOffsetNumber` interactions; coverage there is disproportionately heavy on purpose.

## Tangent space

The spec allows ignoring stored TANGENT and recomputing via MikkTSpace. Recomputation differs subtly across libraries. v1.0 generates assets both with and without explicit tangents.

## Apple Silicon vs other GPUs

VRMMetalKit is Metal-only; cross-GPU pixel-exact comparison is a non-goal. SSIM thresholds are tuned per-pair, with stricter intra-family thresholds (same GPU vendor, same color space) and looser cross-family thresholds. **Property assertions remain strict across all pairs.**
