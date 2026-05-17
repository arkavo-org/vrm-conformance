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
import math
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

    actual_dir = goldens_dir / actual
    reference_dir = goldens_dir / reference

    # Enumerate test_ids that have pose.json from BOTH renderers.
    # Filenames are `<test_id>_<renderer>.pose.json` — strip both the
    # `.pose` extension stem AND the trailing `_<renderer>` suffix to
    # recover the bare test_id used by the runner / asset generator.
    def stem_test_id(p, renderer):
        s = p.stem.removesuffix(".pose")
        return s.removesuffix("_" + renderer)

    actual_poses = {stem_test_id(p, actual): p for p in actual_dir.glob("vrma_*.pose.json")}
    reference_poses = {stem_test_id(p, reference): p for p in reference_dir.glob("vrma_*.pose.json")}
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
        # UniVRM (Unity JsonUtility.ToJson + File.WriteAllText) writes
        # UTF-8 BOM on some hosts; three-vrm/Node writes plain UTF-8.
        # utf-8-sig handles both.
        with open(actual_poses[test_id], encoding="utf-8-sig") as f:
            a = json.load(f)
        with open(reference_poses[test_id], encoding="utf-8-sig") as f:
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
