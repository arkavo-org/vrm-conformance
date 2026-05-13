#!/usr/bin/env bash
# Mock UniVRM adapter — partial output. Writes _meta declaring N tests
# but only emits the first ceil(N/2) entries, then exits non-zero
# (simulates Unity crash mid-batch). The runner must detect the
# mismatch via meta.total_tests and report the missing entries as
# errors.

set -euo pipefail

manifest="$1"
results="$2"

python3 - "$manifest" "$results" <<'PY'
import json, os, sys, math

manifest_path, results_path = sys.argv[1], sys.argv[2]
with open(manifest_path) as f:
    m = json.load(f)

total = len(m["tests"])
emit_count = math.ceil(total / 2)
output_dir = m["output_dir"]
os.makedirs(output_dir, exist_ok=True)

with open(results_path, "w") as out:
    out.write(json.dumps({
        "_meta": True,
        "manifest_version": 1,
        "renderer_name": m["renderer_name"],
        "renderer_version": "mock-v0.131.2",
        "unity_version": "mock-2022.3.50f1",
        "render_pipeline": "Built-in RP",
        "total_tests": total,
    }) + "\n")
    for t in m["tests"][:emit_count]:
        png_path = os.path.join(output_dir, f"{t['test_id']}.png")
        with open(png_path, "wb") as p:
            p.write(b"")  # empty file ok for the mock
        out.write(json.dumps({
            "test_id": t["test_id"],
            "status": "ok",
            "output_path": png_path,
            "actual_color_space": "Srgb",
            "render_seconds": 0.01,
        }) + "\n")
PY

# Exit non-zero to simulate Unity crash. Runner should still parse what
# was written.
exit 1
