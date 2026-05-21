# VMK#283 — non-determinism on animated spring-bone in 0.16.0-rc.1

**Status**: filed 2026-05-21 as [VMK#283](https://github.com/arkavo-org/VRMMetalKit/issues/283).

---

**Title:** SpringBone animated integration is non-deterministic in 0.16.0-rc.1 (was deterministic in 0.15.2)

**Labels:** bug, springbone, regression-from-0.15.x

**Body:**

In 0.16.0-rc.1, the same animated swing test plan rendered twice on identical hardware with the same binary produces different PNG output. 0.15.2 produces byte-identical output across repeated runs of the same input. The non-determinism is localised to the animated spring-bone path; the static settle path and MToon path are byte-identical between 0.15.2 and 0.16.0-rc.1.

## Reproducer

Tested via the [vrm-conformance](https://github.com/arkavo-org/vrm-conformance) adapter binary, which loads a VRM through VRMMetalKit, runs `reset_physics(settle_steps=30)` from rest, drives `animate_root_transform` for 60 frames at 60 Hz with translation_end=(0, 0, -0.15), and `render`s the final frame.

Test plan: a 16-joint spring-bone chain with default drag/stiffness/gravity (`swing_springbone_joints_16` in the conformance corpus).

**0.15.2 (de87578), 3 runs:**

```
run 1  size=46068 bytes  blake3=14b61fb5...
run 2  size=46068 bytes  blake3=14b61fb5...
run 3  size=46068 bytes  blake3=14b61fb5...
```

Byte-identical across runs.

**0.16.0-rc.1 (6a7084d), 5 runs:**

```
run 1  size=46068 bytes  blake3=14b61fb5...   ← matches 0.15.2 baseline
run 2  size=46068 bytes  blake3=14b61fb5...   ← matches 0.15.2 baseline
run 3  size=48480 bytes  blake3=d5e06701...
run 4  size=48734 bytes  blake3=1144c101...
run 5  size=48480 bytes  blake3=d5e06701...
```

**Three distinct outputs across 5 runs**, and pairwise SSIM of the divergent outputs against run 1 is 0.9897 / 0.9885 / 0.9897.

Both binaries are built with `swift build --configuration release` against Xcode 26.3 / Swift 6.3 on macOS 26 / Apple M4 Max. Runs were contiguous on the same machine, no other GPU workload.

## What's affected vs not affected

| Surface | Result |
|---|---|
| MToon (49 sweep variants) | byte-identical, 0.15.2 ≡ RC |
| Static spring-bone settle (82 variants — settle from rest then render) | byte-identical, 0.15.2 ≡ RC |
| Animated swing spring-bone with multi-joint chain | **non-deterministic on RC**, deterministic on 0.15.2 |
| `swing_springbone_default` (1-joint chain) | byte-identical, 0.15.2 ≡ RC |
| `swing_springbone_joints_8` (8-joint chain) | byte-identical, 0.15.2 ≡ RC |
| `swing_springbone_joints_16` (16-joint chain) | **non-deterministic** as above |

Subset of swing tests where the RC was observed to drift between same-binary runs in this sweep (others observed deterministic, but the noise floor "0.15.2 always reproduces, RC sometimes reproduces" suggests broader coverage with more samples would surface more):

- `swing_springbone_joints_16`
- `swing_springbone_drag_{0, 0p2, 0p8, 1}`
- `swing_springbone_stiffness_{0p2, 0p8, 1}`
- `swing_springbone_segment_{0p1, 0p2}`

The static settle sweep (`springbone_*`, 82 variants) is fully deterministic across runs and matches 0.15.2 byte-for-byte. This bug only manifests when the adapter drives `animate_root_transform` per-frame across many physics substeps.

## Likely root cause

Two PRs in 0.16.0-rc.1 touch this code path:

- **#278** (closes VMK#268) — CPU/GPU race on shared-buffer multi-system. Re-architects `animatedRootPositionsBuffer` to pre-allocate per-substep segments with 256-byte alignment and bind the kinematic kernel with per-substep `byteOffset`. The release notes say "Single-system / self-committed-buffer callers are unaffected." That claim appears to need re-verification — the conformance adapter is single-system and is now non-deterministic on input that was deterministic at 0.15.2. The non-determinism's signature (two stable alternative outputs plus the original) is consistent with a per-substep race that resolves to N stable states depending on timing.
- **#274** (closes VMK#237) — five SpringBone fixes including "completion handler optimization" which changes when the CPU completion handler registers across substeps. If a downstream read of simulation state has an implicit dependency on per-substep completion ordering that is no longer guaranteed, that is a race.

I would suspect #278 first based on the surface — the `animatedRootPositionsBuffer` write path is exactly what changed and exactly what fires during `animate_root_transform`.

## Additional context

This was caught while validating the RC against the [vrm-conformance](https://github.com/arkavo-org/vrm-conformance) suite. Other surfaces in the RC validate cleanly:

- 190/191 conformance pass-rate against UniVRM consortium reference (same as 0.15.1's 80/81; identical near-threshold miss on `mtoon_rimLightingMix_1`).
- MToon pairwise SSIM mean vs UniVRM: 0.954 (improved from 0.947 at 0.15.1 on a 2.3× larger sample).
- KHR_texture_transform / KHR_materials_emissive_strength / KHR_materials_ior round-trip parse (PR #277) verified via the GLTFMetalKit test suite.
- `render_sequence` produces all 60 frames cleanly through the existing static-frame test pipeline (Phase 5 RFC-0004 compliance).

No other RC behaviour change surfaces as a regression in the conformance corpus. The only blocker for pin-bumping the suite is this reproducibility regression.

## Suggested triage

1. Reproduce by rendering `swing_springbone_joints_16` (or any of the listed variants) twice with the RC and confirming output diverges.
2. Revert PR #278 locally, re-render — if deterministic, the regression is in #278's pre-allocation / byteOffset path even for single-system callers.
3. If still non-deterministic after #278 revert, revert PR #274's completion handler change next.

If the bisect isolates a specific commit, we have the full vrm-conformance swing corpus available to validate the fix before re-tagging.
