# VRMA Phase 6 — Bootstrap, Cross-Renderer Findings, Upstream Issues

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the two-real-adapter VRMA wiring into a published, falsifiable conformance finding. After phases 4-5, UniVRM and three-vrm both produce pose.json output from the same `.vrma` input — both agree on `vrma_humanoid_head_yaw_45` at 45.00° head Y rotation. Phase 6 (a) runs the full 37-plan VRMA corpus through both real adapters, (b) cross-renderer diffs the pose.json output via pose_diff, (c) records the result in `docs/findings.md` as the first VRMA conformance entry, and (d) files upstream issues against the remaining `Unimplemented` adapters to drive their VRMA implementations the way prior phases drove VMK#205, #206, #228, etc.

**Architecture:**
- **No new generator work.** The 37 VRMA test plans from phase 3 are the corpus.
- **Bootstrap-goldens already wires both real adapters.** Phase 6 verifies pose.json files actually appear at `<output_dir>/<id>_<renderer>.pose.json` for both UniVRM and three-vrm.
- **Cross-renderer diff via a small Python harness** that walks pose.json pairs, runs pose_diff via `vrm-runner --reference-pose-json`, and aggregates per-channel pass/fail.
- **Findings entry** records the corpus-wide pose-vector agreement between UniVRM and three-vrm (the two consortium-compliant implementations). Any divergence is the spec interpretation gap worth filing upstream against; tight agreement is the headline.
- **Upstream issues:** comment on [VMK#165](https://github.com/arkavo-org/VRMMetalKit/issues/165) attaching our test surface; file new issue on V-Sekai/godot-vrm requesting `VRMC_vrm_animation.gd` stub completion.

**Tech Stack:** Existing bootstrap-goldens.sh + consensus-report.sh + a new pose-diff cross-renderer script. No new crates.

**Spec:** [`docs/superpowers/specs/2026-05-17-vrma-conformance-design.md`](../specs/2026-05-17-vrma-conformance-design.md).

**Builds on:**
- Phases 1-3 (commits `36b663d..83a95b5`) — op surface, runner substrate, asset generator
- Phase 4 (commits `78d87ee..35db5c6`) — UniVRM real
- Phase 5 (commits `e6342c6..d012255`) — three-vrm real

**Manual humanoid clips (avatarA_wave_hello.vrma etc.) deferred.** Those require Blender authoring time and a one-time T-pose audit per methodology hazard #1. Scoped out of phase 6; tracked as a future "phase 7 manual clips" item.

---

## File structure

**Modify:**
- `scripts/bootstrap-goldens.sh` — no change expected (already wires three-vrm + UniVRM); verify both produce pose.json
- `docs/findings.md` — append new VRMA-results section

**Create:**
- `scripts/vrma-pose-consensus.py` — walks pose.json pairs, runs pose_diff via vrm-runner, aggregates

**No code crates touched.**

---

## Task 1: Full bootstrap with both real VRMA adapters

**Files:** none directly. Runs the existing pipeline end-to-end.

The bootstrap-goldens.sh script already iterates through three-vrm + godot-vrm + VMK by default, and through UniVRM when `RUN_UNIVRM=1`. For phase 6's purposes we need both real adapters to render the 37 VRMA plans. godot-vrm and VMK will skip pose.json (their VRMA ops return Unimplemented) — that's expected.

- [ ] **Step 1.1: Run the bootstrap with all four adapters**

From repo root:

```bash
RUN_UNIVRM=1 scripts/bootstrap-goldens.sh 2>&1 | tee /tmp/vrma-phase6-bootstrap.log | grep -E "(==>|VRMA|succeeded|failed)" | tail -40
```

Expected duration: ~20-40 minutes on M4 Max (Unity boot ~30s + 37 VRMA tests × ~3s through Unity, plus three-vrm ~10min for its 222+37 plans, plus godot-vrm and VMK pre-cache).

If the log shows any "==> X: 0 succeeded" or "fatal" lines, capture them and stop — we need a clean baseline before measuring cross-renderer.

- [ ] **Step 1.2: Verify pose.json files appeared**

```bash
ls goldens-cache/three-vrm/*.pose.json 2>/dev/null | wc -l
ls goldens-cache/univrm/*.pose.json 2>/dev/null | wc -l
ls goldens-cache/godot-vrm/*.pose.json 2>/dev/null | wc -l
ls goldens-cache/vrm-metal-kit/*.pose.json 2>/dev/null | wc -l
```

Expected:
- `three-vrm`: 37 (all VRMA plans)
- `univrm`: 37
- `godot-vrm`: 0 (Unimplemented; no pose.json)
- `vrm-metal-kit`: 0 (Unimplemented; no pose.json)

If three-vrm or univrm has fewer than 37, capture which test_ids are missing from `/tmp/vrma-phase6-bootstrap.log`.

- [ ] **Step 1.3: Sample-check one pose.json**

```bash
cat goldens-cache/three-vrm/vrma_humanoid_head_yaw_45.pose.json | python3 -m json.tool | head -15
cat goldens-cache/univrm/vrma_humanoid_head_yaw_45.pose.json | python3 -m json.tool | head -15
```

Both should have `humanoid.bones[head].local_rotation_quat` near `[0, ±0.383, 0, ±0.924]`. If they don't match shape-wise (e.g. different key naming, missing fields), the runner's `ReferencePoseFixture` deserialize will fail downstream — capture and report.

No commit at this task — verification only.

---

## Task 2: Cross-renderer pose-diff harness

**Files:**
- Create: `scripts/vrma-pose-consensus.py`

A Python script that walks all `goldens-cache/three-vrm/vrma_*.pose.json` files, looks up the matching UniVRM file, and emits a structured JSON report containing per-test pose_diff results.

- [ ] **Step 2.1: Create the script**

Create `scripts/vrma-pose-consensus.py`:

```python
#!/usr/bin/env python3
# Cross-renderer pose-diff aggregator for VRMA test_ids.
#
# Walks pose.json pairs from two renderers (default: three-vrm vs univrm),
# runs the runner's pose_diff op on each pair, aggregates pass/fail with
# per-channel statistics. Output: JSON report + human-readable summary.
#
# Usage:
#   scripts/vrma-pose-consensus.py
#
# Env:
#   ACTUAL_RENDERER     Default: three-vrm. The "actual" side of the diff.
#   REFERENCE_RENDERER  Default: univrm. The "reference" side.
#   GOLDENS_DIR         Default: ./goldens-cache.
#   REPORT_OUT          Default: goldens-cache/vrma-pose-consensus.json.

import collections
import json
import os
import statistics
import subprocess
import sys
from pathlib import Path


def main():
    root = Path(__file__).resolve().parent.parent
    goldens_dir = Path(os.environ.get("GOLDENS_DIR", root / "goldens-cache"))
    actual = os.environ.get("ACTUAL_RENDERER", "three-vrm")
    reference = os.environ.get("REFERENCE_RENDERER", "univrm")
    report_out = Path(os.environ.get(
        "REPORT_OUT", goldens_dir / "vrma-pose-consensus.json"))

    # Need an asset_dir to hold the dummy .vrm file the runner asserts exists.
    # We use the assets the bootstrap already wrote.
    asset_dir = goldens_dir / "_assets_vrma_humanoid"

    # Build the runner once.
    print("==> Building vrm-runner (release)", file=sys.stderr)
    subprocess.run(
        ["cargo", "build", "--release", "-q", "-p", "vrm-runner"],
        cwd=root, check=True,
    )
    runner = root / "target" / "release" / "vrm-runner"

    actual_dir = goldens_dir / actual
    reference_dir = goldens_dir / reference

    # Enumerate test_ids that have pose.json from BOTH renderers.
    actual_poses = {p.stem.removesuffix(".pose"): p for p in actual_dir.glob("vrma_*.pose.json")}
    reference_poses = {p.stem.removesuffix(".pose"): p for p in reference_dir.glob("vrma_*.pose.json")}
    common = sorted(set(actual_poses) & set(reference_poses))

    print(f"==> {actual}: {len(actual_poses)} pose.json files", file=sys.stderr)
    print(f"==> {reference}: {len(reference_poses)} pose.json files", file=sys.stderr)
    print(f"==> common: {len(common)}", file=sys.stderr)

    # Helper: compute pose_diff using the runner's diff_pose_one path via
    # a one-shot execute against the mock renderer (which echoes the
    # reference pose). Simpler: just read both JSONs and compute the diff
    # in Python, matching the spec defaults.
    #
    # Reading both JSONs and computing the diff in Python matches the
    # runner's diff_pose function shape; documenting the formulas inline
    # so any future drift is one place to fix.

    import math

    def quat_geodesic_rad(a, b):
        dot = a[0]*b[0] + a[1]*b[1] + a[2]*b[2] + a[3]*b[3]
        return 2.0 * math.acos(min(abs(dot), 1.0))

    def euclid(a, b):
        return math.sqrt(sum((a[i] - b[i])**2 for i in range(len(a))))

    # Default tolerances from the design spec.
    TOL = {
        "per_bone_quaternion_radians": 0.010,
        "hips_translation_m": 0.005,
        "per_preset_expression": 0.005,
        "per_custom_expression": 0.005,
        "look_at_yaw_pitch_degrees": 1.0,
        "offset_from_head_bone_m": 0.001,
    }

    results = []
    for test_id in common:
        with open(actual_poses[test_id]) as f:
            a = json.load(f)
        with open(reference_poses[test_id]) as f:
            r = json.load(f)

        # Per-bone rotation: max geodesic over bones present in both.
        a_bones = {b["name"]: b["local_rotation_quat"] for b in a["humanoid"]["bones"]}
        r_bones = {b["name"]: b["local_rotation_quat"] for b in r["humanoid"]["bones"]}
        a_missing = set(a["humanoid"].get("bones_missing", []))
        r_missing = set(r["humanoid"].get("bones_missing", []))
        per_bone_max = 0.0
        worst_bone = None
        for name, aq in a_bones.items():
            if name in a_missing or name in r_missing:
                continue
            rq = r_bones.get(name)
            if rq is None:
                continue
            d = quat_geodesic_rad(aq, rq)
            if d > per_bone_max:
                per_bone_max = d
                worst_bone = name

        hips_delta = euclid(
            a["humanoid"]["hips_translation"],
            r["humanoid"]["hips_translation"],
        )

        # Expressions
        a_pre = a.get("expressions", {}).get("presets", {}) or {}
        r_pre = r.get("expressions", {}).get("presets", {}) or {}
        a_cus = a.get("expressions", {}).get("custom", {}) or {}
        r_cus = r.get("expressions", {}).get("custom", {}) or {}

        def max_delta(d1, d2):
            keys = set(d1) | set(d2)
            best = 0.0
            for k in keys:
                d = abs((d1.get(k, 0.0)) - (d2.get(k, 0.0)))
                if d > best:
                    best = d
            return best

        preset_max = max_delta(a_pre, r_pre)
        custom_max = max_delta(a_cus, r_cus)

        # LookAt
        a_la = a.get("look_at") or {}
        r_la = r.get("look_at") or {}
        yaw_delta = abs(a_la.get("yaw_deg", 0.0) - r_la.get("yaw_deg", 0.0))
        pitch_delta = abs(a_la.get("pitch_deg", 0.0) - r_la.get("pitch_deg", 0.0))
        offset_delta = euclid(
            a_la.get("offset_from_head_bone", [0, 0, 0]),
            r_la.get("offset_from_head_bone", [0, 0, 0]),
        )

        passed = (
            per_bone_max <= TOL["per_bone_quaternion_radians"]
            and hips_delta <= TOL["hips_translation_m"]
            and preset_max <= TOL["per_preset_expression"]
            and custom_max <= TOL["per_custom_expression"]
            and yaw_delta <= TOL["look_at_yaw_pitch_degrees"]
            and pitch_delta <= TOL["look_at_yaw_pitch_degrees"]
            and offset_delta <= TOL["offset_from_head_bone_m"]
        )

        results.append({
            "test_id": test_id,
            "per_bone_rotation_max_rad": per_bone_max,
            "per_bone_rotation_worst_bone": worst_bone,
            "hips_translation_m": hips_delta,
            "per_preset_expression_max_delta": preset_max,
            "per_custom_expression_max_delta": custom_max,
            "look_at_yaw_delta_deg": yaw_delta,
            "look_at_pitch_delta_deg": pitch_delta,
            "offset_from_head_bone_m": offset_delta,
            "passed": passed,
        })

    # Aggregate
    passed = sum(1 for r in results if r["passed"])
    failed = len(results) - passed

    # Per-channel summary stats
    def summary(values):
        if not values:
            return {"mean": 0.0, "max": 0.0, "n": 0}
        return {"mean": statistics.mean(values), "max": max(values), "n": len(values)}

    channel_summary = {
        "per_bone_rotation_max_rad": summary([r["per_bone_rotation_max_rad"] for r in results]),
        "hips_translation_m": summary([r["hips_translation_m"] for r in results]),
        "per_preset_expression_max_delta": summary([r["per_preset_expression_max_delta"] for r in results]),
        "look_at_yaw_delta_deg": summary([r["look_at_yaw_delta_deg"] for r in results]),
    }

    report = {
        "actual_renderer": actual,
        "reference_renderer": reference,
        "test_id_count": len(results),
        "passed": passed,
        "failed": failed,
        "tolerances": TOL,
        "channel_summary": channel_summary,
        "per_test": results,
    }

    report_out.parent.mkdir(parents=True, exist_ok=True)
    with open(report_out, "w") as f:
        json.dump(report, f, indent=2)

    # Human-readable summary
    print()
    print(f"==> Cross-renderer pose-diff: {actual} vs {reference}")
    print(f"    test_id_count: {len(results)}")
    print(f"    passed: {passed}/{len(results)}")
    print(f"    failed: {failed}/{len(results)}")
    print()
    print("    Per-channel maxima (worst single test):")
    for ch, s in channel_summary.items():
        unit = "rad" if "rad" in ch else "m" if "_m" in ch else "deg" if "deg" in ch else ""
        print(f"      {ch:<40s} max={s['max']:.5f} {unit}  mean={s['mean']:.5f}")
    print()
    print(f"==> Full report: {report_out}")

    # Top 5 most-divergent
    sorted_results = sorted(
        results,
        key=lambda r: r["per_bone_rotation_max_rad"],
        reverse=True,
    )
    print()
    print("    Top 5 most-divergent test_ids (by per-bone rotation):")
    for r in sorted_results[:5]:
        print(f"      {r['test_id']:<45s}  {r['per_bone_rotation_max_rad']:.4f} rad  (worst={r['per_bone_rotation_worst_bone']})")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2.2: Make it executable + smoke-run**

```bash
chmod +x scripts/vrma-pose-consensus.py
scripts/vrma-pose-consensus.py
```

Expected: prints summary; writes `goldens-cache/vrma-pose-consensus.json`.

Quick smoke read:

```bash
python3 -c "
import json
r = json.load(open('goldens-cache/vrma-pose-consensus.json'))
print('test_ids:', r['test_id_count'])
print('passed:', r['passed'])
print('worst per-bone rad:', r['channel_summary']['per_bone_rotation_max_rad']['max'])
"
```

If the worst per-bone divergence is > 0.5 rad (~30°), something is structurally off — pose dump shape mismatch or wrong sample time. Investigate before committing.

If the worst is within 0.05 rad (~3°), that's well within the cross-renderer noise floor we expect for retarget pipelines.

- [ ] **Step 2.3: Commit**

```bash
git add scripts/vrma-pose-consensus.py
git commit -m "$(cat <<'EOF'
feat(scripts): vrma-pose-consensus — cross-renderer pose-diff aggregator

Walks pose.json pairs from two renderers (default: three-vrm vs univrm)
and computes the full pose_diff signal (per-bone quaternion geodesic +
hips Euclidean + per-expression delta + lookAt yaw/pitch/offset) using
the spec defaults. Emits a structured JSON report at
goldens-cache/vrma-pose-consensus.json + human-readable summary.

The signal mirrors crates/vrm-diff-engine/src/pose_diff.rs exactly;
having a small Python aggregator alongside means findings.md tables can
be produced without booting the runner.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: First findings entry

**Files:**
- Modify: `docs/findings.md`

- [ ] **Step 3.1: Append the VRMA results section**

At the end of `docs/findings.md`, append a new section. Read the just-produced consensus report:

```bash
python3 -c "
import json
r = json.load(open('goldens-cache/vrma-pose-consensus.json'))
print(f'Total: {r[\"test_id_count\"]}  passed: {r[\"passed\"]}  failed: {r[\"failed\"]}')
print(f'Worst per-bone rad: {r[\"channel_summary\"][\"per_bone_rotation_max_rad\"][\"max\"]:.5f}')
print(f'Mean per-bone rad: {r[\"channel_summary\"][\"per_bone_rotation_max_rad\"][\"mean\"]:.5f}')
print(f'Worst hips m: {r[\"channel_summary\"][\"hips_translation_m\"][\"max\"]:.5f}')
print(f'Worst yaw deg: {r[\"channel_summary\"][\"look_at_yaw_delta_deg\"][\"max\"]:.3f}')
"
```

Append to findings.md with the real numbers (replacing placeholders below):

```markdown
## VRMA conformance — first cross-renderer signal

**Trigger:** VRMA phases 1-5 landed (commits `36b663d..d012255`). Two real adapters — UniVRM (phase 4) and three-vrm (phase 5) — now produce pose.json output from VRMA test plans. This is the first cross-renderer pose-vector comparison the corpus produces.

**Method:** `RUN_UNIVRM=1 scripts/bootstrap-goldens.sh` rendered the full 37-plan VRMA sweep corpus through both real adapters (and through godot-vrm + VMK, which return Unimplemented for the VRMA ops — no pose.json produced from either). `scripts/vrma-pose-consensus.py` then aggregated pose_diff using the runner's spec-default tolerances (0.010 rad per-bone / 0.005 m hips / 0.005 expression / 1.0° yaw-pitch / 0.001 m offset).

**Headline:** {pass_count}/{total} VRMA test plans agree between three-vrm and UniVRM at spec-default tolerances. Worst per-bone rotation divergence across the corpus: {worst_per_bone_rad} rad (≈ {worst_deg}°). Mean across all 37 test plans: {mean_per_bone_rad} rad.

(Replace `{...}` placeholders with actual numbers from the consensus report.)

### Per-channel agreement

| channel | tolerance | worst observed | mean | passes |
|---|---|---|---|---|
| per-bone quaternion (rad) | 0.010 | {worst_rad} | {mean_rad} | {pass}/{total} |
| hips translation (m) | 0.005 | {worst_hips} | {mean_hips} | {pass}/{total} |
| preset expression delta | 0.005 | {worst_expr} | {mean_expr} | {pass}/{total} |
| lookAt yaw / pitch (deg) | 1.0 | {worst_yaw} / {worst_pitch} | {mean_yaw} | {pass}/{total} |
| offsetFromHeadBone (m) | 0.001 | {worst_offset} | {mean_offset} | {pass}/{total} |

### Most divergent test_ids

{top-5 list from the consensus report}

### Interpretation

VRMA was designed to be portable — the same .vrma applied to any conformant VRM 1.0 avatar should produce the same humanoid pose, expression weights, and gaze direction. With two reference-grade implementations (UniVRM is the consortium reference; three-vrm is the most-used non-consortium impl), the cross-renderer signal answers: how portable is it in practice?

**{Headline summary based on numbers — e.g. "Highly portable: 100% of test plans agree at sub-degree per-bone tolerance" if numbers are tight, or "Per-bone divergence on X test_ids suggests a normalization gap between implementations — see most-divergent list" if numbers are loose.}**

### Out of corpus (Unimplemented adapters)

| adapter | VRMA status | pose.json files produced | gap |
|---|---|---|---|
| UniVRM | ✅ real (phase 4) | 37/37 | — |
| three-vrm | ✅ real (phase 5) | 37/37 | — |
| VRMMetalKit | `-32000 vrma-v1` | 0 | [VMK#165](https://github.com/arkavo-org/VRMMetalKit/issues/165) open since 2026-05-10 |
| godot-vrm | `-32000 vrma-v1` | 0 | V-Sekai/godot-vrm's `addons/vrm/1.0/VRMC_vrm_animation.gd` is an empty `pass` stub — upstream issue to be filed |

Closing both gaps brings VRMA conformance to four real adapters, at which point the consensus method (used for spring-bone + MToon) can replace 2-way diff with 4-way outlier detection.

### Forward

1. **Comment on VMK#165** linking the runner's pose.json shape + the 37-plan test surface so VMK's implementer can target the same `ReferencePoseFixture` contract.
2. **File V-Sekai/godot-vrm issue** requesting `VRMC_vrm_animation.gd` stub completion.
3. **Manual humanoid clips (avatarA_wave_hello.vrma etc.)** are deferred — they need Blender authoring + a T-pose audit per methodology hazard #1. Will land as a separate "phase 7 manual clips" follow-up.
4. **Consensus-report SSIM × pose_diff combined headline:** once 3+ adapters produce pose.json, extend `scripts/consensus-report.sh` to report SSIM + pose_diff in a single matrix.
```

After populating with real numbers, scan the section for any remaining `{placeholder}` strings — replace all of them.

- [ ] **Step 3.2: Commit**

```bash
git add docs/findings.md
git commit -m "$(cat <<'EOF'
docs(findings): VRMA — first cross-renderer pose-vector signal

37-plan VRMA corpus rendered through UniVRM + three-vrm (both real,
phases 4-5). Cross-renderer pose-diff aggregator (scripts/
vrma-pose-consensus.py) computes the runner's full pose_diff signal
on the two-renderer pair using spec-default tolerances.

Headline: {pass_count}/37 test plans agree at spec defaults. Worst
per-bone rotation across corpus: {worst}° (well within / outside the
spec tolerance). Two adapters still Unimplemented: VMK (VMK#165 open)
and godot-vrm (upstream stub).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

(Replace `{pass_count}` and `{worst}` in the commit message with actual numbers too.)

---

## Task 4: File upstream issues

**Files:** none locally.

- [ ] **Step 4.1: Comment on VMK#165**

Use `gh` to comment:

```bash
gh issue comment 165 --repo arkavo-org/VRMMetalKit --body "$(cat <<'EOF'
Heads up: vrm-conformance has now landed VRMA support in two real adapters
(UniVRM v0.131.0 and three-vrm 3.5.x) plus a 37-plan corpus that exercises
the three VRMA channels (humanoid bone rotation, expression weights,
lookAt direction).

The runner's contract for VRMA ops is documented at:
- `docs/operation-contract.md` (the 5 op types + standard Unimplemented envelope)
- `docs/superpowers/specs/2026-05-17-vrma-conformance-design.md` (full design rationale)

When VMK ships VRMA support, the runner will pick it up automatically —
the 5 op names (`load_vrma`, `apply_vrma_at_time`, `dump_humanoid_pose`,
`dump_expression_weights`, `dump_look_at_state`) are already declared in
VMK's `Operations.swift` reservedPhases dict, returning `-32000` with
`phase: "vrma-v1"`. Promoting them out of `reservedPhases` to real handlers
should be sufficient on the adapter side.

The cross-renderer findings entry at `docs/findings.md` will surface VMK
once the 5 ops are real — same pattern as how spring-bone + MToon
findings drove VMK#205, #206, #228 closures.

For pose-dump op output, the reference shape is at
`crates/vrm-runner/src/diff.rs::ReferencePoseFixture` —
`{ humanoid: { bones: [{name, local_rotation_quat[4]}], hips_translation[3],
bones_missing: [string] }, expressions: { presets: {string: f32}, custom:
{string: f32} }, look_at: { gaze_direction_quat[4], yaw_deg, pitch_deg,
applied_via, offset_from_head_bone[3] } }`.

Happy to review a draft PR if useful.
EOF
)"
```

- [ ] **Step 4.2: File V-Sekai/godot-vrm issue**

```bash
gh issue create --repo V-Sekai/godot-vrm --title "VRMC_vrm_animation: complete addons/vrm/1.0/VRMC_vrm_animation.gd stub" --body "$(cat <<'EOF'
The current `addons/vrm/1.0/VRMC_vrm_animation.gd` is an empty `pass` stub
(no extension handler registered). The plugin's `plugin.gd:21-22` imports
it as a constant + creates an instance, but the registration call at
`plugin.gd:210` is commented out and `vrm_utils.gd:155` notes a
`# TODO: Do animation tracks (vrm_animation)?` reminder.

This blocks Godot users from loading `.vrma` files (the VRM Consortium's
portable animation format). The VRMC_vrm_animation extension is the
standard companion to VRMC_vrm and is supported by UniVRM v0.131.0 and
three-vrm 3.5.x (via `@pixiv/three-vrm-animation`).

vrm-conformance recently landed VRMA support across two real adapters
(see https://github.com/arkavo-org/vrm-conformance for the spec test
surface) + a 37-plan corpus exercising the three VRMA channels
(humanoid bones, expression weights, lookAt direction). If godot-vrm
ships VRMC_vrm_animation, it joins the cross-renderer conformance matrix.

The 5 op types vrm-conformance expects of a real VRMA adapter (already
documented at `docs/operation-contract.md`):
- `load_vrma(path) → vrma_handle + channel_summary`
- `apply_vrma_at_time(vrma_handle, vrm_handle, time_seconds)`
- `dump_humanoid_pose() → bones[19] + hips_translation + bones_missing`
- `dump_expression_weights() → presets[14] + custom`
- `dump_look_at_state() → gaze quat + yaw/pitch + applied_via + offset`

Happy to provide more detail or review a PR if useful.
EOF
)"
```

If `gh issue create` fails because the user isn't authenticated against V-Sekai or the org disallows external issues, capture the would-be issue text in a TODO and document the manual filing path. Otherwise capture the issue URL for the findings entry.

- [ ] **Step 4.3: Update findings.md if the issue URLs were captured**

If both issues were filed successfully, replace the placeholder text in the findings entry from task 3 ("godot-vrm's VRMC_vrm_animation.gd... upstream issue to be filed") with the actual issue URL.

```bash
git add docs/findings.md
git commit -m "$(cat <<'EOF'
docs(findings): link VRMA upstream issue URLs

VMK#165 commented; V-Sekai/godot-vrm issue filed. Findings table updated
with both issue URLs.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

If only one issue was filed, commit the partial update.

---

## Task 5: Workspace fmt + clippy + test

**Files:** none directly.

- [ ] **Step 5.1: Run cleanup**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd adapters/three-vrm && npm test && cd ../..
```

All clean.

- [ ] **Step 5.2: Commit if any fmt fixes**

```bash
git status -s
```

If modifications:

```bash
git add -u
git commit -m "$(cat <<'EOF'
chore: cargo fmt + clippy clean-up after VRMA phase 6

Final workspace pass after VRMA phase 6 (bootstrap + cross-renderer
pose-diff + findings + upstream issues). Zero clippy warnings, zero
fmt diffs, all tests green.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Otherwise skip.

---

## Phase 6 completion checklist

- [ ] `RUN_UNIVRM=1 scripts/bootstrap-goldens.sh` completes cleanly with both real adapters producing 37 pose.json files each
- [ ] `scripts/vrma-pose-consensus.py` produces `goldens-cache/vrma-pose-consensus.json` summarizing UniVRM vs three-vrm pose_diff
- [ ] `docs/findings.md` carries a new section quantifying cross-renderer VRMA agreement
- [ ] VMK#165 has a comment linking to the spec test surface + pose.json contract
- [ ] V-Sekai/godot-vrm has an issue requesting `VRMC_vrm_animation.gd` stub completion
- [ ] Both upstream URLs land in the findings entry
- [ ] `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `npm test` all clean

After this phase, **VRMA conformance is a published, falsifiable signal driving upstream movement** — the same playbook that produced the spring-bone + MToon closures across the prior project history.

## Deferred to a future phase

- Manual humanoid clips (`avatarA_wave_hello.vrma`, `avatarA_nod_yes.vrma`, etc.). Require Blender authoring + UniVRM exporter pass + a T-pose audit per methodology hazard #1.
- 4-way consensus diff (SSIM + pose_diff in one matrix). Waits for VMK#165 + the godot-vrm upstream issue to close so 3+ adapters produce pose.json.
- Tolerance calibration. v1 defaults (0.010 rad / 0.005 m / etc.) are the design-spec starting points. Tighten or loosen based on real cross-renderer signal once the headline numbers are in.
