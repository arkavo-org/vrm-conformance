# VMK — VRMA `lookAt` block silently dropped when gaze is encoded as a rotation channel

**Status**: filed 2026-05-22 as [VMK#286](https://github.com/arkavo-org/VRMMetalKit/issues/286).

---

**Title:** `VRMAnimationLoader` `lookAt` block: only translation tracks parsed — rotation-channel gaze (as used by `@pixiv/three-vrm-animation` + Pixiv VRMA samples) silently dropped

**Labels:** bug, animation, vrma, spec-interpretation

**Body:**

`VRMAnimationLoader.loadVRMA(from:model:)` parses the `VRMC_vrm_animation.lookAt` block but only populates `clip.lookAtTargetSampler` when the referenced node has a `translation` track. When the gaze is encoded as a **rotation** channel on the lookAt node — which is what `@pixiv/three-vrm-animation`, the Pixiv VRMA sample files, and the [vrm-conformance](https://github.com/arkavo-org/vrm-conformance) asset generator emit — the sampler stays `nil` and `apply_vrma` is silently a no-op for gaze.

## Reproducer

A minimal VRMA file whose `animations[0].channels[]` targets the lookAt node's `rotation` path:

```json
{
  "extensions": {
    "VRMC_vrm_animation": {
      "specVersion": "1.0",
      "lookAt": { "node": 0, "offsetFromHeadBone": [0, 0.06, 0] }
    }
  },
  "animations": [
    { "channels": [ { "sampler": 0, "target": { "node": 0, "path": "rotation" } } ],
      "samplers": [ { "input": 0, "output": 1, "interpolation": "LINEAR" } ] }
  ]
}
```

Then through VMK:

```swift
let clip = try VRMAnimationLoader.loadVRMA(from: url, model: model)
print(clip.lookAtTargetSampler != nil)  // false
```

The conformance suite's `vrma_lookat_*` corpus (10 plans, mix of yaw/pitch/bone/expression variants) all hit this path and render the avatar with no applied gaze. Pose-dump `yaw_deg` / `pitch_deg` come out as `0` on VMK; three-vrm + UniVRM produce non-zero values from the same assets.

## Where the gap is

`Sources/VRMMetalKit/Animation/VRMAnimationLoader.swift:390-402`:

```swift
// B1: Parse lookAt block from VRMC_vrm_animation extension.
// Spec: the referenced node's translation track drives the head-local look-at target.
if let extensionDict = document.extensions?["VRMC_vrm_animation"] as? [String: Any],
   let lookAtBlock = extensionDict["lookAt"] as? [String: Any],
   let lookAtNodeAny = lookAtBlock["node"],
   let lookAtNodeIndex = intValue(from: lookAtNodeAny),
   let lookAtTracks = nodeTracks[lookAtNodeIndex],
   let translationTrack = lookAtTracks["translation"] {     // ← translation only
    clip.lookAtTargetSampler = { t in sampleVector3(translationTrack, at: t) }
}
```

`nodeTracks[lookAtNodeIndex]` for a rotation-encoded gaze contains `["rotation": KeyTrack]` but no `"translation"` key, so the entire block is skipped without warning.

## Spec is unambiguous — rotation is mandatory

The VRMC_vrm_animation-1.0 spec README explicitly mandates the rotation-channel form (https://github.com/vrm-c/vrm-specification/blob/master/specification/VRMC_vrm_animation-1.0/README.md, "LookAt → Eye gaze direction" section):

> `VRMC_vrm_animation/lookAt/node` specifies the glTF node that has the rotation as the eye gaze direction.
> The rotation in the local space of the specified node is treated as the animation data for the eye gaze direction.
> In glTF, the rotation is defined as a quaternion.
> However, when applying it to the LookAt component of the `VRMC_vrm`, it is converted to the yaw-pitch Euler angle.
> The rotation order of the Euler angle must be interpreted as Extrinsic ZXY, and the rotation around the Y axis is yaw and the rotation around the X axis is pitch.

The translation-only path in the current loader is non-conformant. Corroborating evidence:

- `@pixiv/three-vrm-animation`'s `VRMAnimationLoaderPlugin` consumes the rotation channel and applies it to a forward vector.
- Pixiv's own published VRMA samples (e.g., the avatar-sample VRMAs distributed with `three-vrm`) use rotation channels.
- The vrm-conformance asset generator's `vrma_lookat_*` corpus uses rotation channels per the spec wording above.

## Suggested fix

When the lookAt node has only a rotation track, derive a head-local target point from the rotation applied to a head-local forward vector. Approximately:

```swift
} else if let rotationTrack = lookAtTracks["rotation"] {
    // Pixiv VRMA samples + three-vrm-animation: gaze is the rotation
    // of the lookAt node applied to head-local forward (-Z).
    clip.lookAtTargetSampler = { t in
        let q = sampleQuaternion(rotationTrack, at: t)
        let forward = SIMD3<Float>(0, 0, -1)
        return q.act(forward)
    }
}
```

(Distance for the target point can be 1.0 by convention since `VRMLookAtController` normalises the direction internally; the magnitude only matters for sign in atan2/asin.)

A more thorough fix would also handle the translation+rotation case by composing both, though no asset in the wild currently does that.

## Why this doesn't show up in image-level conformance

Pixel impact of gaze direction at 1024² with the default eye-pupil contrast is small enough that the SSIM threshold (0.85) is not breached even when the gaze isn't applied. The conformance suite's image consensus across `vrma_lookat_*` reports 10/10 pass with `mean SSIM ≈ 0.9665` vs three-vrm. The pose-level dump diff (yaw/pitch from `dump_look_at_state`) is what surfaces the gap. Once the conformance suite adds a pose-level diff layer to `consensus-report.sh`, these tests will start failing on VMK — which is the prompt to land this fix.

## Filer context

This was surfaced by [vrm-conformance](https://github.com/arkavo-org/vrm-conformance) commit `c221767` which landed VRMA wiring for the `vrm-metal-kit` adapter on top of VMK 0.16.0-rc.2. Details + commit logs:

- Findings entry: `docs/findings.md` "VMK lookAt rotation-channel gap"
- Adapter wiring: `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift` (`handleApplyVrmaAtTime`, `handleDumpLookAtState`)
- Conformance numbers: 575/575 VMK rendering success, 10/10 `vrma_lookat_*` pass image consensus despite the silent gaze drop
