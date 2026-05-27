# VMK — VRM 0.x load-time 180° Y-rotation: load-bearing coordinate normalisation, not a bug

**Status**: documented 2026-05-26 as a slice 1 empirical finding. **Not filed upstream** — the implementation is intentional and spec-consistent from VRMMetalKit's perspective. This stub records the suite's investigation result for future reference.

---

**Title:** VMK applies a 180° Y-rotation to every VRM 0.x node TRS at load time — location, rationale, and conformance implications

**Labels:** vrm-0x, coordinate-system, load-time, upstream-investigation

**Body:**

## Finding

During slice 1 VRM 0.x conformance work, the suite investigated whether VMK's empirical 180° rotation on VRM 0.x avatars was an adapter-shim artefact or an upstream library behaviour.

**Result: UPSTREAM_LIBRARY, LOAD_BEARING.**

The rotation is implemented in `VRMMetalKit.Core.VRMModel.buildNodeHierarchy()` at revision `392d94926619bcb59401f49b29e82d2a575d4d15` (0.16.0 stable). It is guarded by `if isVRM0` and conjugates every node's local TRS into VRM 1.0 / glTF right-handed space at model-load time. A companion pass `applyVRM0InverseBindMatrixConjugation()` left-multiplies every skin's `inverseBindMatrices` by `Ry180` to keep skinning consistent with the rotated joint world matrices.

The adapter shim (`adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`) contains no orientation-flipping code. The `.pi` references in the shim are all FOV degree-to-radian and lookAt yaw/pitch conversions.

## Code location (revision 392d949)

```
Sources/VRMMetalKit/Core/VRMModel.swift:980–1011   buildNodeHierarchy() — if isVRM0 block
Sources/VRMMetalKit/Core/VRMModel.swift:881–897    applyVRM0InverseBindMatrixConjugation()
```

The 0.14.0 release notes (recorded in `adapters/vrm-metal-kit/Package.swift`) describe the history:

> Load-time VRM 0.x → 1.0 coordinate conversion: the 180° Y rotation that used to live on `VRMRenderer` is now conjugated into VRM 0.x node TRS and inverse bind matrices at load. Physics, animation, and culling share a single coordinate space — closes a long-standing left/right limb handedness gap between formats.

## Is this a conformance problem?

**No, for the current conformance suite setup.** The suite's `set_camera` operation positions the camera at +Z facing −Z, which is the correct VRM 1.0 convention. VRMMetalKit normalises VRM 0.x models into the same VRM 1.0 / glTF coordinate space at load, so the camera placement is consistent regardless of spec version.

The VRM 0.x spec (`docs/upstream-specs/vrm-specification/specification/0.0/README.md`) states: "Model faces towards −Z direction" (in Unity's left-handed coordinate system). After VRMMetalKit's normalisation, the model faces +Z in glTF right-handed space — which is the same physical forward direction, just expressed in the target coordinate frame. This is the correct transformation.

## When to revisit

If the conformance suite ever:

1. Positions cameras using VRM 0.x spec conventions directly (facing −Z toward the model), cross-renderer pixel comparisons will show VMK rendering the front of the model while a renderer that does NOT normalise shows the back — this would be a genuine cross-renderer divergence and worth filing upstream.
2. Adds a `raw_vrm0_pose_dump` op that reads node TRS without the normalisation, expecting Unity-frame values — the normalisation would silently invalidate the dump.

Neither scenario applies to the current slice 1 scope.

## Repro (for reference)

Load `assets/humanoid/avatarA_0_0.vrm` through a renderer that does NOT normalise VRM 0.x coordinates, set camera at −Z facing +Z (the Unity convention for VRM 0.x). The model should face the camera. If the camera is instead set at +Z facing −Z (the VRM 1.0 convention), the model will face away — that is the source of "observed back of head" reports, not a VMK rendering bug.

## Spec citation

`docs/upstream-specs/vrm-specification/specification/0.0/README.md` — facing direction section: "Model faces towards −Z direction" (Unity left-handed). VRM 1.0 spec models face +Z (glTF right-handed).

## Crossref

- `docs/findings.md` — "2026-05-26 — VMK 180° flip on VRM 0.x: location and structurality (slice 1 days 1–3 empirical check)" for the full investigation record.
- `adapters/vrm-metal-kit/Package.swift` — 0.14.0 release-notes comment documents the load-time migration.
