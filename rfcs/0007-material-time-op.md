# RFC 0007: `set_material_time` — deterministic MToon UV-animation clock

- **Status:** Accepted
- **Author(s):** Paul Flynn
- **Date:** 2026-11-01

## Summary

Add one operation, `set_material_time { session_id, time_seconds }`, and one optional test-plan field, `material_time_seconds`. When a plan sets the field, the runner calls `set_material_time` after any physics ops and before `render`. Every later `render` in the session then draws MToon UV animation at exactly that time, whatever physics stepping or earlier renders did to the adapter's own clock. An adapter that cannot pin the clock returns `-32000 Unimplemented` (or `-32601`), and the runner fails the plan with a message naming the op. A plan that asks for a time never silently renders at some other time.

## Motivation

MToon 1.0 UV animation (`uvAnimationScrollX/YSpeedFactor`, `uvAnimationRotationSpeedFactor`, `uvAnimationMaskTexture`) is time-based ("automatic and perpetual", spec README §UV Animation). Until now the suite could not test it, for two reasons:

1. A single-frame `render` draws at an adapter-defined time. Measured on 2026-11-01:
   - three-vrm advances its material clock by 1/60 s on every `render` and every physics step, so a plan with `reset_physics(settle_steps=30)` draws at 31/60 s.
   - VMK ≤ 1.2.0 never advances its clock, so UV animation is a no-op there (reported by the VMK maintainers on 2026-11-01).
   - VMK 1.2.1 advances `VRMRenderer.uvAnimationTime` once per simulated draw.

   Rendering the same plan through these adapters therefore compares different moments.
2. `docs/backlog.md` recorded the gap: every UV-animation variant would render the same frame.

VMK 1.2.1 fixes UV animation (`uvAnimationTime`, B-channel mask, spec transform order) and asks for a suite re-gate before stable. The re-gate needs cells that can see the fix.

## Detailed design

### Operation

```text
set_material_time
  params: SetMaterialTimeParams { session_id: string, time_seconds: f32 }   // finite, >= 0
  result: {}                                                                  // empty object
  errors: -32602 when time_seconds is negative or non-finite
          -32000 Unimplemented { phase: "material-time" } when the adapter cannot pin the clock
```

Semantics: until the session is disposed or the op is called again, every `render` draws MToon UV animation as if `time_seconds` had elapsed since the animation started at zero. The value affects every MToon material in the session, including outline passes. Physics, VRMA sampling and spring bones are unaffected. The op sets one clock and nothing else.

`render_sequence` is out of scope: frames advance the adapter's own clock. `TestPlan::validate` rejects a plan that sets both `material_time_seconds` and `render_sequence`.

### Test plan

```yaml
material_time_seconds: 1.0   # optional; absent = today's behaviour (no op call)
```

Plans without the field are byte-identical to today's and never call the op, so the existing corpus and every adapter are unaffected.

### Runner

The runner follows `load_vrm → set_camera → set_lighting → set_post_processing → [reset_physics → animate_root_transform]` and, when the plan sets the field, then calls `set_material_time` before `render`. An adapter error with code `-32000` or `-32601` becomes a plan failure whose message starts `set_material_time unsupported by <renderer>:`, so it cannot be mistaken for a render. `execute-test-batch` (the UniVRM filesystem path) skips timed plans with a warning before it launches Unity, because the batch protocol has no way to pass the time. The rest of the batch still runs.

### Adapters

| adapter | behaviour |
| --- | --- |
| three-vrm | Implements it. At render, after `vrm.update`, it sets each MToon material's `uvAnimationScrollX/YOffset = speed × t` and `uvAnimationRotationPhase = speed × t`. |
| VRMMetalKit | Implements it when compiled with `-DVMK_UV_ANIMATION_TIME`, which needs VMK ≥ 1.2.1: sets `renderer.uvAnimationTime` immediately before the draw. Without the flag it returns Unimplemented. `Package.swift` defines the flag once the pin reaches a release that ships the API. |
| mock | Returns Unimplemented: it does not sample textures, so honouring the op would be a silent no-op. |
| UniVRM, Godot, Babylon | Not implemented. Godot and Babylon return `-32601`; the UniVRM batch path skips timed plans with a warning. |

### Reference semantics the cells encode

From the MToon 1.0 spec (README §UV Animation › Transform Order; §Compatibility with KHR_texture_transform; §uvAnimationMaskTexture):

- The animated UV is `uv' = Scroll · T(0.5) · Rot · T(−0.5) · uv`: rotation about (0.5, 0.5) first, then scroll.
- The animation runs before each texture's `KHR_texture_transform`: `KHR(anim(uv))`.
- `uvAnimationMaskTexture` scales time by its **B** channel.
- The animated textures are base colour, shade multiply, rim multiply, outline width multiply, emissive and normal, plus `shadingToonyMultiplyTexture`. That last one is the README's own wording and appears to be a stale draft name for `shadingShiftTexture`. Matcap is not animated.

Known reference divergences, from source reading on 2026-11-01:

- **UniVRM v0.131.0** (`GetMToonGeometry_Uv`) computes `rot(TRANSFORM_TEX(uv) + scroll − 0.5) + 0.5`. That applies `_MainTex_ST` before the animation and scrolls before rotating, both against the spec. Its outline width mask uses raw `texcoord0`, with no animation and no transform.
- **three-vrm** follows the spec order for fragment textures. It samples the outline width mask at `KHR_mask(raw uv)`, without animation.
- **VMK 1.2.1** (per its maintainers) matches three-vrm, including the unanimated width mask.

Because those divergences are known, the sweep includes cells that separate them: combined rotation and scroll, an animation × KHR-order cell, and an animated outline width mask. Per the suite's spec-first rule, a renderer that departs from the spec text is reported as divergent, even when every renderer agrees.

### Assets

`emit-uv-animation-sweep` (VRM 1.0 only) emits the checkerboard sphere with `baseColorTexture` and the UV-animation factors set. Each plan carries `material_time_seconds`, and the sweep includes speed-zero and t = 0 controls.

## Alternatives considered

- **A `capture_time_seconds` field on `RenderParams`.** Rejected. Adapters deserialize params loosely, so an adapter that predates the field would ignore it and render at its own time, and the runner would record that as a pass.
- **Drive time through `render_sequence` frames.** Rejected for single-moment cells. Frame-to-time mapping differs per adapter (three-vrm advances on every `update`, VMK on every simulated draw), and the first frame is not at t = 0 on every adapter.
- **Capability discovery through `describe`.** Deferred. The runner does not query adapter catalogs today, and a failing op call gives the same guarantee with no new negotiation step.

## Open questions

- Whether `set_material_time` should also drive time-dependent features outside MToon if any appear (none exist in VRM 1.0 today).
- The UniVRM batch path could implement this by writing the time into the batch manifest and setting MToon10's `_Time` through a material property block. That is deferred until the Unity licence is restored.

## References

- `docs/upstream-specs/vrm-specification/specification/VRMC_materials_mtoon-1.0/README.md` — §UV Animation, §uvAnimationMaskTexture, §Compatibility with KHR_texture_transform.
- `docs/findings.md` 2026-11-01 (VMK 1.2.0-rc.2 re-gate, 1.2.1 plan).
- `docs/backlog.md` (UV-animation capture-time item).
- RFC 0004 (`render_sequence`), the precedent for a reserved-then-real op.
