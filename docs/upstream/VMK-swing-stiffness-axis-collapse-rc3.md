# VMK — Spring-bone stiffness sweep collapses to default-plan output under rc.3's fixed-rate `synchronousSpringBone` timestep

**Status**: filed 2026-05-23 as [VMK#292](https://github.com/arkavo-org/VRMMetalKit/issues/292); **closed 2026-05-23 in 0.16.0-rc.4** (commit `81ebce6`, PR #296). Verified on rc.4: 9 swing axis variants produce 9 distinct hashes; stiffness sweep `{0, 0.2, 0.8, 1}` now differentiates (`773ddc9f...` / `bf3106ea...` / `87302812...` / `855a0b71...`). The fix drains settling frames inside `warmupPhysics` per [the upstream PR description](https://github.com/arkavo-org/VRMMetalKit/pull/296).

---

**Title:** Spring-bone: entire stiffness sweep collapses to the default-plan output on 0.16.0-rc.3 — the fixed-rate `synchronousSpringBone` step that closes VMK#283 makes stiffness have no observable effect on rendered swing

**Labels:** bug, spring-bone, animation, regression

**Body:**

PR #291's fixed-rate (60 Hz) `synchronousSpringBone` timestep — which closes [VMK#283](https://github.com/arkavo-org/VRMMetalKit/issues/283) and is the headline win of 0.16.0-rc.3 — has a side effect on a different surface: the entire stiffness sweep on the conformance suite's animated swing corpus now renders byte-identical to the default plan. On rc.2 (and 0.15.2 under-sample), the stiffness axis differentiated despite per-run jitter; on rc.3 it's flat.

Same bug class as the previously-closed [VMK#240](https://github.com/arkavo-org/VRMMetalKit/issues/240) (stiffness collapse under animation, closed in 0.15.0 by consuming `settlingFrames` inside `warmupPhysics`). Likely a settle/warmup interaction with the new fixed-rate step path that didn't exist on the old wall-clock path.

## Reproducer

Conformance corpus at [`crates/vrm-asset-generator/src/sweep.rs::springbone_swing_sweep`](https://github.com/arkavo-org/vrm-conformance) — the `swing_springbone_stiffness_*` and `swing_springbone_drag_*` test plans, which use `animate_root_transform` to excite the chain (the only way to test drag/stiffness/inertia per the suite's methodology pin, since static `step_physics` only exercises gravity). Same machine: Apple M4 Max, macOS 26.5 (build 25F71), Xcode 26.5 / Swift 6.3.2.

```bash
# Three runs per stiffness value on rc.2 (binary at /tmp/vmk-adapter.rc2)
# vs deterministic single run on rc.3 (binary at /tmp/vmk-adapter.rc3)
for plan_id in swing_springbone_default \
               swing_springbone_stiffness_0 swing_springbone_stiffness_0p2 \
               swing_springbone_stiffness_0p8 swing_springbone_stiffness_1; do
    vrm-runner execute-test-plan \
        --plan "goldens-cache/_assets_swing/${plan_id}.test.yaml" \
        --adapter-bin /tmp/vmk-adapter.rc3 \
        --asset-dir "goldens-cache/_assets_swing" \
        --output-dir "/tmp/rc3/${plan_id}" \
        --renderer-name vrm-metal-kit --json >/dev/null
    shasum -a 256 "/tmp/rc3/${plan_id}/${plan_id}_vrm-metal-kit.png" | cut -c1-16
done
```

Output table:

| plan_id | rc.2 sha256[:12] (3 runs) | rc.3 sha256[:12] (deterministic) | axis effect on rc.3 |
|---|---|---|---|
| `swing_springbone_default` | `8bd3bca3...` / `68b391e7...` ×2 | `68b391e7764a2a9e` | baseline |
| `swing_springbone_stiffness_0` | `68b391e7...` / `009b0cbd...` ×2 | `68b391e7764a2a9e` | **identical to default** |
| `swing_springbone_stiffness_0p2` | `68b391e7...` ×3 | `68b391e7764a2a9e` | **identical to default** |
| `swing_springbone_stiffness_0p8` | `e790e30c...` ×2 / `3074ad2f...` | `68b391e7764a2a9e` | **identical to default** |
| `swing_springbone_stiffness_1` | `be7e94a8...` ×2 / `98483065...` | `68b391e7764a2a9e` | **identical to default** |

The same collapse pattern affects high-drag (`swing_springbone_drag_0p8` and `swing_springbone_drag_1` both render as `68b391e7764a2a9e`, identical to default), while low-drag (`drag_0`, `drag_0p2`) still differentiates. The high-end-of-axis collapse is plausibly explained by "high drag damps the chain back to rest before the capture frame, so the rendered output reflects only the rest pose regardless of stiffness." But the `stiffness_0` (zero stiffness — chain should flop loosely under the swing motion) producing the same hash as `stiffness_1` (rigid chain) suggests stiffness genuinely isn't being applied through the new step path, not just that capture-time choice masks it.

## What rc.2 looked like

On rc.2 (and 0.15.2 with deep sampling per the [rc.2 verification](https://github.com/arkavo-org/vrm-conformance/blob/main/docs/findings.md#vmk-0160-rc2-verification)), the stiffness axis showed **three distinct value-clusters** across the sweep despite per-run non-determinism: `{0, 0.2}` clustered around `68b391e7...`, `{0.8}` around `e790e30c.../3074ad2f...`, `{1}` around `be7e94a8.../98483065...`. So even with the rc.1/rc.2 swing-jitter bug, the stiffness axis differentiated output. On rc.3 the jitter is gone (good — closes VMK#283) but the axis differentiation is also gone.

## Suspected root cause

The fixed-rate path in PR #291 — `simulationDeltaTime` defaulting to 1/60 when unset, and the conformance adapter not setting it — likely interacts with `settlingFrames` / `warmupPhysics` differently than the old wall-clock path did. In particular, the new constant-rate ticks may cause the warmup band (where the shader's `1 - smoothstep(0, 60, settlingFrames)` curve zeroes the stiffness contribution; see VMK#240's closure) to be entered and consumed in a way that leaves stiffness=0 at the conformance capture window for every plan in the sweep.

A complementary hypothesis: the conformance test plans' capture-time may now fall reproducibly inside a post-settle window where the chain has finished moving and stiffness no longer affects the visible rest position. If so, the right fix on the suite side is to retune `animate_root_transform`'s duration so the capture frame lands mid-swing — but this would have shown up as `swing_springbone_drag_0` (zero drag — chain should never settle) collapsing too. It doesn't: `drag_0` is distinct from default. So stiffness specifically appears unreached, not just unobservable.

## Asks

1. Confirm whether `synchronousSpringBone`'s new fixed-rate path runs the per-joint stiffness contribution through the same shader path as the old wall-clock path, or whether the integrator was simplified during the #283 fix.
2. If the fix is renderer-side, optional asset-side workaround: should the conformance adapter set `simulationDeltaTime` explicitly (and to what value) to recover stiffness differentiation?

## Crossref

Related: [VMK#240](https://github.com/arkavo-org/VRMMetalKit/issues/240) (stiffness collapse under animation, closed in 0.15.0 — same bug class). The closure in 0.15.0 fixed the warmup counter; this looks like the same end-user symptom returning via a different code path.
