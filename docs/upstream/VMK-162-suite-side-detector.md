# VMK#162 — suite-side detector for the parser-substitution + inertia-compensation equilibrium

**Status**: suite-side detector; tracks upstream [VMK#162](https://github.com/arkavo-org/VRMMetalKit/issues/162) (state: OPEN).

This file is not an issue we filed — VMK#162 was filed by upstream. It documents the **suite-side detector** we built to guard against silent regressions of the load-bearing equilibrium described in the upstream issue body.

---

## What we guard against

VMK#162's body identifies two load-bearing pieces of code whose interaction is what makes AvatarSample_A's hair render correctly:

1. **Parser substitution** at `VRMExtensionParser.swift:1061` (was line 666 in the original issue; line number drifted with refactors):

   ```swift
   joint.gravityPower = rawGravityPower > 0 ? rawGravityPower : 1.0
   ```

   Author-specified `gravityPower=0` is silently rewritten to `1.0`. Author intent is overridden.

2. **Inertia compensation** at `SpringBonePredict.metal:107-142`. The shader comment at line 116 explicitly names the parser substitution as its load-bearing partner — the compensation only behaves correctly under the assumption that gravityPower is never literally 0.

The model author (AvatarSample_A_1.0) tuned `stiffness=0.85 / gravityPower=0 / dragForce=0.4` against this combination. Two prior attempts (per VMK#159's review history) tried to fix one piece in isolation:

- "Stop overriding author-specified `gravityPower=0`" — preserve author intent in the parser.
- "Gate inertia compensation on `gravityPower > 0`" — don't apply world-space inertia preservation to chains the author wanted held only by stiffness.

Both individual fixes were "theoretically clean." Together they broke AvatarSample_A's hair — persistent cat-ear tufts at rest, sharp spiking during head rotation. Reverted in the same branch. **The equilibrium is load-bearing.**

A future PR that touches either piece in isolation will silently regress hair behavior on tuned models. There's no upstream automated test for this. This is what the suite-side detector exists to catch.

## The detector

`vrm-runner execute-test-plan-matrix` (Phase 7 — runner subcommand, shipped 2026-05-18) computes per-joint position deltas between a baseline asset and N perturbation assets that vary one parameter each. The matrix YAML at [`test-plans/manual/coupling/vmk162_swing_coupling.matrix.yaml`](../../test-plans/manual/coupling/vmk162_swing_coupling.matrix.yaml) targets the swing variant of the coupling sweep (animated excitation via `animate_root_transform`, because static settle on a stiff 4-joint chain doesn't move enough under gravity to differentiate).

The coupling sweep ([`crates/vrm-asset-generator/src/spring_bone.rs::spring_bone_coupling_sweep`](../../crates/vrm-asset-generator/src/spring_bone.rs)) emits 4 variants × {settle, swing} = 8 assets:

| variant | gravity_power (author) | expected behavior under substitution |
|---|---|---|
| `vmk162_baseline` | 0.5 | baseline (literal 0.5 → inertia comp active) |
| `vmk162_gravity_0` | **0.0** | parser rewrites to 1.0 internally; inertia comp **may** check raw author value → different chain behavior than gravity_1 |
| `vmk162_gravity_1` | 1.0 | literal 1.0; inertia comp active → near-baseline chain behavior |
| `vmk162_gravity_2` | 2.0 | literal 2.0; inertia comp active → near-baseline chain behavior |

## Calibration data (2026-05-23, Apple M4 Max / macOS 26.5)

Reference signatures captured against three real renderers, in order of fidelity:

```
                                    gravity_0    gravity_1    gravity_2
                                    max_drift_m  max_drift_m  max_drift_m
vrm-metal-kit (VMK 0.16.0 stable)     0.1444       0.0014       0.0014    ← substitution + inertia comp signature
three-vrm 3.5.0                       0.0711       0.0235       0.0393    ← spec-correct (no substitution, no comp)
godot-vrm                             0.0000       0.0000       0.0000    ← invalid (import-time bug; see docs/findings.md)
```

**Per-joint drift vectors** (4 joints; root is joint 0):

```
VMK 0.16.0:
  gravity_0  per_joint=[0.00000, 0.02703, 0.08320, 0.14436]   ← monotonic chain droop
  gravity_1  per_joint=[0.00000, 0.00144, 0.00101, 0.00119]   ← negligible
  gravity_2  per_joint=[0.00000, 0.00144, 0.00101, 0.00119]   ← negligible (identical to gravity_1)

three-vrm 3.5.0:
  gravity_0  per_joint=[0.00000, 0.01237, 0.03799, 0.07106]
  gravity_1  per_joint=[0.00000, 0.00411, 0.01263, 0.02347]
  gravity_2  per_joint=[0.00000, 0.00694, 0.02141, 0.03934]
```

### Interpretation

The VMK signature has a **distinctive shape**: gravity_0 drift is ~100× gravity_1 drift, and gravity_1 ≡ gravity_2. The three-vrm signature is **monotonic**: gravity_2 > gravity_1 > 0, and all three are non-trivially non-zero.

The VMK shape is the fingerprint of the parser substitution + inertia compensation interplay:
- gravity_0 (author=0.0): parser rewrites to 1.0. Inertia compensation appears NOT to fire (or fires with a different gating). Chain droops dramatically.
- gravity_1 (author=1.0): parser keeps 1.0. Inertia compensation fires fully. Chain stays near baseline.
- gravity_2 (author=2.0): parser keeps 2.0. Inertia compensation fires fully and snaps chain back to near-baseline regardless of magnitude.

The three-vrm shape is the fingerprint of a **spec-correct** renderer: gravity_power is applied as authored, chain hangs proportionally, no clamping or compensation.

If a future VMK commit **removes the parser substitution in isolation**: gravity_0 would behave like literal 0 (no gravity, chain held only by stiffness). The VMK signature would shift toward three-vrm's — gravity_0 drift would *decrease* (chain stops falling), gravity_1 would still be near-zero (compensation still active), and the shape would lose its 100× asymmetry.

If a future VMK commit **removes the inertia compensation in isolation**: gravity_1 and gravity_2 would no longer snap back to baseline. Their drifts would grow toward three-vrm's range. The baseline itself might shift (chain no longer snapped to its current position by compensation).

Either change is detectable by comparing a fresh matrix run against this locked signature.

## Threshold-based pass/fail (coarse)

The matrix YAML's `coupling_threshold_m: 0.20` is set conservatively above VMK 0.16.0's gravity_0 drift (0.1444). Under this threshold:

- VMK 0.16.0: **passes** (max drift 0.1444 < 0.20).
- three-vrm: **passes** (max drift 0.0711 < 0.20).
- godot-vrm: **passes** (all zeros — but the result is meaningless because the godot adapter has the import-time bug; see `docs/findings.md`).

A future change that pushes any perturbation's max_drift above 0.20 m trips the threshold. This is a **catastrophic-only** signal — it catches "spring-bone integration exploded" but not subtle equilibrium shifts.

## Fine-grained pass/fail (manual diff vs locked signatures)

For the more sensitive "is the equilibrium intact" check, compare a fresh matrix output to the per-renderer reference signatures above. A perturbation's `max_drift_m` shifting by more than ~30% from its locked value indicates equilibrium change worth investigating.

Example diagnostic protocol after a VMK pin bump:

```bash
# Bump pin, build, then:
target/release/vrm-runner execute-test-plan-matrix \
    --matrix test-plans/manual/coupling/vmk162_swing_coupling.matrix.yaml \
    --adapter-bin /path/to/vmk-adapter.new \
    --asset-dir goldens-cache/_assets_vmk162 \
    --output-dir /tmp/vmk162-check \
    --renderer-name vrm-metal-kit --json

# Compare each outcome's max_drift_m against:
#   gravity_0  expected ≈ 0.1444  (±30% → [0.10, 0.19])
#   gravity_1  expected ≈ 0.0014  (±30% → [0.001, 0.002])
#   gravity_2  expected ≈ 0.0014  (±30% → [0.001, 0.002])
#
# Out-of-range on gravity_1 OR gravity_2 → inertia compensation
#   likely disturbed.
# Out-of-range on gravity_0 (specifically: dropping toward three-vrm's
#   0.07) → parser substitution likely disturbed.
```

## CI / bootstrap wiring

`scripts/bootstrap-goldens.sh` (2026-05-23) emits the coupling sweep and runs the matrix per-adapter after the main render pass. The bootstrap output includes a "VMK#162 coupling matrix (per-adapter)" section showing PASS/FAIL + per-perturbation drift for each adapter. Failure of the matrix threshold doesn't currently fail the bootstrap exit code (Phase 7 ships the runner as a manual diagnostic; CI gating is a follow-up).

## Follow-ups

1. **Make the matrix CI-gating.** Wire `bootstrap-goldens.sh`'s exit code to the matrix failure, gated behind an opt-in env var (e.g. `STRICT_COUPLING=1`) so existing workflows aren't broken.
2. **Per-renderer reference-signature mode for the matrix runner.** Right now the matrix YAML carries a single global threshold. A future enhancement would let the YAML carry per-renderer expected signatures (or a separate `*.calibration.json` file) and surface deviation from expected as the pass/fail criterion.
3. **Extend the coupling sweep** with stiffness × gravity_power and drag × gravity_power 2D perturbation sets (matrix runner already supports N perturbations; just needs the asset emission). Would surface the wider equilibrium beyond gravity_power alone.

## Related

- [VMK#162](https://github.com/arkavo-org/VRMMetalKit/issues/162) (upstream, OPEN) — original investigation.
- [VMK#143](https://github.com/arkavo-org/VRMMetalKit/pull/143) — re-enabled the inertia compensation that's load-bearing here.
- [VMK#159](https://github.com/arkavo-org/VRMMetalKit/pull/159) — PR where the failed-fix experiment lived and was reverted; history flattened.
- `docs/findings.md` — Phase 6 + Phase 7 entries for the matrix-runner infrastructure background.
