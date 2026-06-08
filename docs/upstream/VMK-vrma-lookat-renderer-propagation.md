# VMK — VRMA `lookAt` parsed correctly (VMK#286 closure) but parsed yaw/pitch doesn't reach the rendered avatar

**Status**: filed 2026-05-23 as [VMK#294](https://github.com/arkavo-org/VRMMetalKit/issues/294); **closed 2026-05-23 in 0.16.0-rc.4** (commit `81ebce6`, PR #296) via `VRMLookAtController.applyImmediately()`. Adapter wiring updated to call `applyImmediately()` after setting the target (`adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`). **Not yet observable on this suite's conformance corpus** because the synthetic humanoid avatars lack `leftEye`/`rightEye` bones (where `applyToBones` writes) and lack `LookLeft`/`LookRight`/`LookUp`/`LookDown` custom expressions (where `applyToExpressions` writes). The VMK fix is plausibly correct; suite-side asset coverage needs extending to verify end-to-end. Follow-up to [VMK#286](https://github.com/arkavo-org/VRMMetalKit/issues/286).

> **SUITE-SIDE FOLLOW-UP CLOSED (2026-06-07).** The "asset coverage needs extending" gap is closed: the `vroid_default_F_gaze_*` corpus (8 manual plans + the `emit-gaze-sweep` VRMA clips) drives bone-driven gaze on the real VRoid avatar (`J_Adj_*_FaceEye` eye bones), and `leftEye`/`rightEye` were added to the pose-dump bone list. End-to-end gaze propagation is now observable and was verified during the **VMK 0.17.1 (#332)** bump — see the 2026-06-07 "VMK 0.17.1 eye look-at #332" entry in `docs/findings.md` for the 0.17.0→0.17.1 before/after (wall-eyed → parallel; head-offset → head-relative). Note 0.17.1 fixed two *further* eye-look-at bugs (head-local resolution + eye-rest composition) that the original `applyImmediately()` propagation alone did not address.

---

**Title:** VRMA lookAt: rotation channel now parsed correctly (VMK#286 closed) but parsed `yaw_deg`/`pitch_deg` don't reach humanoid bones or expression presets — all 10 gaze plans render byte-identical PNGs

**Labels:** bug, vrma, animation, lookat, follow-up

**Body:**

[VMK#286](https://github.com/arkavo-org/VRMMetalKit/issues/286) (rotation-channel gaze in `VRMAnimationLoader` silently dropped) is **closed on 0.16.0-rc.3** by PR #291 — the pose dump's `look_at.yaw_deg` / `look_at.pitch_deg` are now correctly populated from the rotation channel. The filing's specific assertion is satisfied.

However, the parsed gaze doesn't propagate to the rendered avatar geometry. All 10 plans in the conformance suite's `vrma_lookat_*` corpus still render to a byte-identical PNG, with the avatar in its T-pose facing forward, indistinguishable from a neutral-gaze plan.

## What's working (rc.3)

```
test_id                          dump look_at.yaw_deg  pitch_deg  applied_via
vrma_lookat_yaw_neg60_bone               +60.00          0.00      bone        ← correct
vrma_lookat_yaw_pos60_bone               -60.00          0.00      bone        ← correct
vrma_lookat_pitch_neg30_bone               0.00        -30.00      bone        ← correct
vrma_lookat_pitch_pos30_bone               0.00        +30.00      bone        ← correct
vrma_lookat_neutral_bone                   0.00          0.00      bone        ← correct
vrma_lookat_yaw_pos60_expr               -60.00          0.00      expression  ← correct
vrma_lookat_pitch_pos30_expr               0.00        +30.00      expression  ← correct
... (5 more matching this pattern)
```

(Sign convention is "avatar's gaze rotation vs world frame", which is the negation of the named-target direction — consistent with spec.)

## What's broken

Despite the gaze state being correctly computed, the rendered output is unchanged across the entire sweep:

```
test_id                              rendered PNG sha256[:16]
vrma_lookat_yaw_neg60_bone          5d8cf1789282275f
vrma_lookat_yaw_pos60_bone          5d8cf1789282275f
vrma_lookat_pitch_neg30_bone        5d8cf1789282275f
vrma_lookat_pitch_pos30_bone        5d8cf1789282275f
vrma_lookat_neutral_bone            5d8cf1789282275f
vrma_lookat_yaw_neg60_expr          5d8cf1789282275f
vrma_lookat_yaw_pos60_expr          5d8cf1789282275f
vrma_lookat_pitch_neg30_expr        5d8cf1789282275f
vrma_lookat_pitch_pos30_expr        5d8cf1789282275f
vrma_lookat_neutral_expr            5d8cf1789282275f
```

**All 10 plans, byte-identical.** And critically, the pose dump shows the gaze isn't being pushed into the bone graph or expression weights:

- **Bone-driven plans** (`*_bone` variants, with `lookAt.type = "bone"` in the VRM): every entry in `humanoid.bones[*].local_rotation_quat` is identity `[0, 0, 0, 1]`. The neck / head bones — which on a bone-driven lookAt should rotate to follow the gaze direction — stay at rest.
- **Expression-driven plans** (`*_expr` variants, with `lookAt.type = "expression"` in the VRM): every preset weight in `expressions.presets` is `0.0`, including `lookLeft` / `lookRight` / `lookUp` / `lookDown` — which on an expression-driven lookAt should activate to drive the iris/eye-shading shift.

So `apply_vrma_at_time` is decoding the rotation channel into `(yaw_deg, pitch_deg)` and storing it on the controller, but the controller's "push state into bones or expression weights" pass doesn't fire before the render — or fires in a way that doesn't survive the render-frame snapshot.

## Suspected root causes

Two non-exclusive possibilities:

1. **`apply_vrma_at_time` updates the lookAt state in the controller without dispatching the bone/expression resolution.** The yaw/pitch end up retrievable via `dump_look_at_state` (which reads the controller's stored gaze directly) but the avatar's bone graph / expression weight buffer are never written to. The reverse case for `dump_humanoid_pose` reading bone rotations and `dump_expression_weights` reading the weight buffer.
2. **`VRMLookAtController.update` IS called but only on a render-time tick that's gated on `Application.isPlaying` or equivalent.** The offline render path bypasses the tick, leaving the controller in "queued gaze, not yet applied" state.

Either way, the user-visible effect is the same: 10 plans, 1 rendered image.

## Reproducer

Conformance corpus at [`crates/vrm-asset-generator/src/sweep.rs::vrma_lookat_sweep`](https://github.com/arkavo-org/vrm-conformance) — emits 10 paired VRM + VRMA + test-plan triplets covering yaw ±60°, pitch ±30°, and neutral, across both `lookAt.type` axes (bone vs expression).

```bash
target/release/vrm-asset-generator emit-vrma-lookat-sweep \
    --output-dir /tmp/rc3-verify/vrma_lookat

for plan in /tmp/rc3-verify/vrma_lookat/*.test.yaml; do
    pid=$(basename "$plan" .test.yaml)
    target/release/vrm-runner execute-test-plan \
        --plan "$plan" --adapter-bin /tmp/vmk-adapter.rc3 \
        --asset-dir /tmp/rc3-verify/vrma_lookat \
        --output-dir "/tmp/rc3-verify/render/$pid" \
        --renderer-name vrm-metal-kit --json >/dev/null

    PNG="/tmp/rc3-verify/render/$pid/${pid}_vrm-metal-kit.png"
    POSE="/tmp/rc3-verify/render/$pid/${pid}_vrm-metal-kit.pose.json"
    echo "$pid  sha=$(shasum -a 256 $PNG | cut -c1-16)"
    python3 -c "
import json; d=json.load(open('$POSE'))
la=d['look_at']
non_id=[b['name'] for b in d['humanoid']['bones'] if any(abs(c)>1e-5 for c in b['local_rotation_quat'][:3])]
exprs={k:v for k,v in d['expressions']['presets'].items() if v>0.001}
print(f'   yaw={la[\"yaw_deg\"]:6.2f} pitch={la[\"pitch_deg\"]:6.2f} non_id_bones={non_id or \"none\"} active_exprs={exprs or \"none\"}')
"
done
```

## Suggested fix

Push the lookAt state into the bone graph and expression weight buffer at the end of `apply_vrma_at_time`, mirroring the pattern that `apply_vrma_at_time` already uses for humanoid bone targets. Something like:

```swift
func applyVrmaAtTime(_ time: TimeInterval) {
    // ... existing humanoid bone + expression preset application ...

    // After all per-bone/expression channels are applied, resolve the lookAt
    // state into the corresponding bone or preset.
    if let lookAt = clip.lookAtTargetSampler?.sampleAtTime(time) {
        controller.setGaze(lookAt)
        controller.resolveToRenderTargets()  // ← currently appears missing on offline path
    }
}
```

`resolveToRenderTargets` (or equivalent) being whatever method walks the avatar's `lookAt.type` (bone or expression) and writes either the neck/head local rotation or the `lookLeft/Right/Up/Down` preset weights, respectively.

## Crossref

- [VMK#286](https://github.com/arkavo-org/VRMMetalKit/issues/286) — VRMA lookAt rotation channel parsing, closed in 0.16.0-rc.3 PR #291. This issue is the follow-up gap.
- [VMK#269](https://github.com/arkavo-org/VRMMetalKit/issues/269) — VRMA retargeting "zombie pose", closed in 0.15.1. Same code area (VRMA application path) but different bug class.
