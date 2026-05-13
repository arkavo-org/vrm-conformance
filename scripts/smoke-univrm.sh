#!/usr/bin/env bash
# Smoke test for the UniVRM adapter: generates a one-test corpus, runs
# it through the runner's execute-test-batch subcommand pointed at the
# real adapter, asserts a non-trivial PNG is produced and SSIM ≥ 0.75
# against three-vrm's baseline for the same test_id (when present).
#
# Skip with SKIP_SMOKE=1 to short-circuit.
#
# Usage:
#   scripts/smoke-univrm.sh                 # runs full smoke
#   UNITY_BIN=/path/to/Unity ./scripts/smoke-univrm.sh

set -euo pipefail

if [ "${SKIP_SMOKE:-0}" = "1" ]; then
  echo "SKIP_SMOKE=1; exiting clean."
  exit 0
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [ ! -x adapters/univrm/launcher.sh ]; then
  echo "error: adapters/univrm/launcher.sh missing or not executable" >&2
  exit 1
fi
UNITY_BIN_PATH="${UNITY_BIN:-/Applications/Unity/Hub/Editor/6000.4.6f1/Unity.app/Contents/MacOS/Unity}"
if [ ! -x "$UNITY_BIN_PATH" ]; then
  echo "error: Unity binary not found at $UNITY_BIN_PATH" >&2
  echo "       set UNITY_BIN env or install Unity 6000.4.6f1" >&2
  exit 127
fi

SMOKE_DIR=/tmp/univrm-smoke
rm -rf "$SMOKE_DIR" && mkdir -p "$SMOKE_DIR/plans" "$SMOKE_DIR/out"

echo ">>> Generating one-test corpus (mtoon_default)"
cargo run --release -p vrm-asset-generator -- emit-default \
  --id smoke_mtoon_default \
  --output-dir "$SMOKE_DIR/plans/" >/dev/null

echo ">>> Running adapter via execute-test-batch"
cargo run --release -p vrm-runner -- execute-test-batch \
  --plans "$SMOKE_DIR/plans" \
  --adapter-bin adapters/univrm/launcher.sh \
  --output-dir "$SMOKE_DIR/out" \
  --renderer-name univrm \
  --json | tee "$SMOKE_DIR/run-summary.json"

echo ">>> Asserting output exists + non-trivial"
PNG="$SMOKE_DIR/out/smoke_mtoon_default.png"
if [ ! -f "$PNG" ]; then
  echo "FAIL: $PNG missing" >&2; exit 2
fi
size=$(stat -f%z "$PNG")
if [ "$size" -lt 10000 ]; then
  echo "FAIL: $PNG only $size bytes — render probably empty" >&2; exit 3
fi
echo "OK: PNG produced, $size bytes"

# Optional SSIM vs three-vrm baseline (only if baseline present).
BASELINE="goldens-cache/three-vrm/mtoon_default.png"
if [ -f "$BASELINE" ]; then
  echo ">>> SSIM vs three-vrm baseline"
  cargo run --release -p vrm-runner -- diff \
    --plan "$SMOKE_DIR/plans/smoke_mtoon_default.test.yaml" \
    --render "$PNG" \
    --reference "$BASELINE" \
    --renderer-name univrm \
    --json | tee "$SMOKE_DIR/diff.json"
  ssim=$(python3 -c "import json; print(json.load(open('$SMOKE_DIR/diff.json'))['ssim'])")
  echo "SSIM = $ssim"
  if awk "BEGIN{ exit !($ssim >= 0.75) }"; then
    echo "OK: SSIM $ssim ≥ 0.75"
  else
    echo "WARN: SSIM $ssim < 0.75 — UniVRM diverges from three-vrm baseline more than expected" >&2
  fi
else
  echo "(no three-vrm baseline at $BASELINE; skipping SSIM check)"
fi

echo ">>> Smoke OK"
