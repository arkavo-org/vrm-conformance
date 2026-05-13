#!/usr/bin/env bash
# Mock UniVRM adapter — happy path. Reads the manifest (arg 1), emits a
# valid results.ndjson with one "ok" entry per test_id (arg 2), and
# writes a placeholder PNG to each entry's output_path.
#
# Used by crates/vrm-runner/tests/execute_test_batch.rs to verify the
# runner-side contract without a real Unity install.

set -euo pipefail

manifest="$1"
results="$2"

# Tiny PNG: a 1x1 magenta pixel (matches the cross-adapter sentinel).
# Decoded bytes of a minimal PNG file:
PNG_BYTES_HEX="89504e470d0a1a0a0000000d49484452000000010000000108020000009077531d0000000c4944415478daedc1010100000080900052bdc1000000000049454e44ae426082"

# Resolve manifest fields via /usr/bin/python3 (universally available on
# macOS + most Linux CI runners). Avoids a jq dep.
python3 - "$manifest" "$results" "$PNG_BYTES_HEX" <<'PY'
import json, os, sys, binascii

manifest_path, results_path, png_hex = sys.argv[1], sys.argv[2], sys.argv[3]
with open(manifest_path) as f:
    m = json.load(f)

png_bytes = binascii.unhexlify(png_hex)
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
        "total_tests": len(m["tests"]),
    }) + "\n")
    for t in m["tests"]:
        png_path = os.path.join(output_dir, f"{t['test_id']}.png")
        with open(png_path, "wb") as p:
            p.write(png_bytes)
        out.write(json.dumps({
            "test_id": t["test_id"],
            "status": "ok",
            "output_path": png_path,
            "actual_color_space": t["output"]["color_space"].capitalize(),
            "render_seconds": 0.01,
        }) + "\n")
PY
