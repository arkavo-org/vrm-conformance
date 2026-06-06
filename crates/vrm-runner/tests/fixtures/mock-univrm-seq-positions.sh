#!/usr/bin/env bash
# Mock UniVRM adapter — render_sequence with capture_positions. Emits a
# results.ndjson where each test's entry carries a `frames` array, and each
# frame carries `spring_positions` (the canonical [[x,y,z],...] shape). Used
# by execute_test_batch.rs to verify the runner persists the per-plan
# positions JSON for batch-mode adapters — without a real Unity install.

set -euo pipefail

manifest="$1"
results="$2"

PNG_BYTES_HEX="89504e470d0a1a0a0000000d49484452000000010000000108020000009077531d0000000c4944415478daedc1010100000080900052bdc1000000000049454e44ae426082"

python3 - "$manifest" "$results" "$PNG_BYTES_HEX" <<'PY'
import json, os, sys, binascii

manifest_path, results_path, png_hex = sys.argv[1], sys.argv[2], sys.argv[3]
with open(manifest_path) as f:
    m = json.load(f)

png_bytes = binascii.unhexlify(png_hex)
output_dir = m["output_dir"]
os.makedirs(output_dir, exist_ok=True)

# Two frames, positions MOVE across frames (a real solver under animation).
# FLAT joint positions [x0,y0,z0,x1,y1,z1] — the adapter's JsonUtility vec3
# convention; the runner reshapes to canonical [[x,y,z],...] on disk.
frame_positions = [
    [0.0, 1.50, 0.0, 0.0, 1.45, 0.0],
    [0.02, 1.50, 0.0, 0.04, 1.45, 0.0],
]

with open(results_path, "w") as out:
    out.write(json.dumps({
        "_meta": True,
        "manifest_version": 1,
        "renderer_name": m["renderer_name"],
        "renderer_version": "mock-v0.131.2",
        "unity_version": "mock-2022.3.50f1",
        "render_pipeline": "Built-in RP",
        "total_tests": len(m["tests"]),
    }) + "\n")
    for t in m["tests"]:
        frames_dir = os.path.join(output_dir, f"{t['test_id']}_frames")
        os.makedirs(frames_dir, exist_ok=True)
        frames = []
        for i, jp in enumerate(frame_positions):
            fp = os.path.join(frames_dir, f"{i:04d}.png")
            with open(fp, "wb") as p:
                p.write(png_bytes)
            frames.append({
                "index": i,
                "timestamp_seconds": float(i) / 30.0,
                "path": fp,
                "blake3": "blake3:" + ("0" * 64),
                "spring_positions": [{"name": "hair", "joint_positions": jp}],
            })
        out.write(json.dumps({
            "test_id": t["test_id"],
            "status": "ok",
            "output_path": frames[0]["path"],
            "actual_color_space": t["output"]["color_space"].capitalize(),
            "frames": frames,
            "duration_seconds": len(frames) / 30.0,
            "frame_hz_achieved": 30.0,
        }) + "\n")
PY
