# capture_positions in the godot-vrm adapter — design

**Date:** 2026-06-06
**Status:** Approved (brainstorm)
**Context:** Closes the gap recorded in `docs/methodology.md` ("CCD / `penetration-diff` is mock-backed only") and `docs/findings.md` (2026-06-06 #313 Track 2): the CCD penetration metric has never run against a real spring-bone solver, because `capture_positions` → `SequenceFrame.spring_positions` is implemented only in `vrm-mock-renderer` (a static synthetic chain). This makes godot-vrm the first **real** adapter to report per-frame spring-bone positions, enabling a measured end-to-end CCD result.

## Goal

1. Implement `capture_positions` in the godot-vrm adapter so `render_sequence` returns per-frame `spring_positions` from godot's actual simulated chain.
2. Run the 12-asset CCD sweep (`emit-springbone-ccd-sweep`) through godot end-to-end, measure penetration via `vrm-runner penetration-diff`, and record the first real-engine CCD result in `docs/findings.md`.

Non-goals: implementing capture in any other adapter (UniVRM remains the highest-value follow-up); a permanent CI gate for the godot CCD run; adding CCD/swept collision to godot (godot's discrete collision is exactly what we want to measure).

## Why godot-vrm first

- Already implements `dump_bone_positions` (`session.gd:263-298`) — single-frame joint world-position extraction. `capture_positions` is "call that per frame inside the render_sequence loop."
- Real L4 spring-bone physics; full 80-test corpus renders end-to-end; godot 4.6.3 on PATH (fast iteration).
- Its spring-bone solver is a faithful port of UniVRM's algorithm, so it is a reasonable proxy for the golden's behavior.
- It uses **discrete** (non-CCD) collision, so the threshold-straddling sweep should surface real tunneling on the fast/thin-radius cells — the asymmetry that proves the metric measures tunnel-prevention, and a genuine conformance finding in its own right.

## Change surface — godot adapter only

The runner requires **no changes**. Verified end-to-end with the mock: the runner already threads `capture_positions` from the plan's `render_sequence` block into `RenderSequenceParams`, persists `spring_positions` to `<output_dir>/<plan_id>_<renderer>_positions.json` (`execute.rs`), and centralizes BLAKE3 rehashing from on-disk PNG bytes — all adapter-agnostic. `operations.gd` already forwards the full params dict to `session.render_sequence`, so `capture_positions` flows through without a dispatch change.

### `adapters/godot-vrm/src/session.gd`

1. **Extract a shared helper.** Refactor the per-spring extraction loop currently in `dump_bone_positions` (lines ~277-296) into:

   ```gdscript
   func _collect_spring_positions() -> Array:
       # Returns [{ "name": String, "joint_positions": [[x,y,z], ...] }, ...]
       # for every spring chain, in spring order. Empty array when there is
       # no spring-bone manager / skeleton.
   ```

   `dump_bone_positions` calls it (preserving its existing `spring_index` filter — the filter stays in `dump_bone_positions`, the helper returns all springs). The helper is the single definition of the `SpringPositions` shape (`name` + `joint_positions`), matching `vrm_ops::SpringPositions`.

2. **Capture in the render_sequence loop.** Read `var capture_positions: bool = params.get("capture_positions", false)`. Inside the frame loop, **immediately after `vrm_secondary.do_process(physics_dt)` (line 401)** and before the render `await`, when `capture_positions` is set, call `_collect_spring_positions()` and attach the result to the frame dict:

   ```gdscript
   var frame := {
       "index": i,
       "timestamp_seconds": float(i) / frame_hz,
       "path": frame_path,
       "blake3": zero_hash,
   }
   if capture_positions:
       frame["spring_positions"] = positions_for_this_frame
   frames.append(frame)
   ```

   (`positions_for_this_frame` captured right after `do_process`, stored in a local before the render await so the rendered pose and the recorded positions are the same simulation state.)

### Timing correctness

`do_process` advances the spring-bone simulation (including godot's discrete collision resolution); the frame then renders that post-step pose. Capturing positions right after `do_process` — not after the render await — guarantees `spring_positions[frame]` is the exact post-step, post-collision pose the PNG shows and that `penetration-diff` evaluates. When `capture_positions` is false the loop is byte-unchanged (existing sequence tests unaffected).

## Data flow (unchanged downstream)

```
plan.render_sequence.capture_positions: true
  → runner RenderSequenceParams.capture_positions = true
  → godot render_sequence → frames[i].spring_positions  (NEW: real godot chain)
  → runner persists <id>_godot_positions.json
  → vrm-runner penetration-diff --positions <json> --plan <plan>
       → vrm_diff_engine::penetration::worst_penetration vs plan.ccd_colliders
       → { max_penetration_depth_m, worst_frame_index, passed, ... }
```

## Verification / measurement

1. Build the godot shim (`target/release/vrm-godot-shim`); emit the 12-asset CCD sweep (`emit-springbone-ccd-sweep`).
2. For each of the 12 plans: `execute-test-plan --adapter-bin target/release/vrm-godot-shim` → positions JSON; then `penetration-diff` → `{max_penetration_depth_m, passed, worst_frame_index}`.
3. **Expected:** fast/thin-radius cells (e.g. `ccd_sphere_r0p005_fast`) show non-zero penetration (godot tunnels thin colliders at speed — no CCD); slow/large-radius cells pass. Tabulate all 12 in `docs/findings.md` as the first real-engine CCD measurement.
4. **Trajectory sanity check (risk guard).** Before trusting any "pass", confirm godot's chain actually swings into the collider's path at `x=0.10` (inspect a captured positions JSON: do the joint x-coordinates approach 0.10?). If the chain never reaches the collider, every cell trivially passes — the same hollow-pass failure mode as the mock — and that must be reported as a corpus-geometry caveat, not a clean pass.

## Tests

Add a godot `render_sequence` positions assertion mirroring the existing `crates/vrm-runner/tests/render_sequence_e2e_godot_vrm.rs` pattern (godot-on-PATH gated): run a short sequence with `capture_positions: true`, assert each frame carries `spring_positions` with non-empty `joint_positions`, and assert positions are **not** all-identical across frames under `animate_root_transform` (the property that distinguishes a real solver from the mock's static chain). GDScript-side: extend `adapters/godot-vrm/tests/test_operations.gd` if there is a cheap in-engine assertion, else rely on the runner-level test.

## Risks

- **Hollow pass** if the sweep geometry doesn't bring godot's chain near the collider — mitigated by the trajectory sanity check above.
- **godot-on-PATH** required for the measurement and the E2E test (consistent with existing godot E2E coverage); the implementation itself is verifiable by code review + the runner test.
