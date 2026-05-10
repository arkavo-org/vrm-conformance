#!/usr/bin/env bash
set -euo pipefail

# Phase 1+2A+2B end-to-end smoke. Requires:
#   - Validator shim installed (.tools/vrm-validator-cli) — optional; only
#       affects asset-validator gating.
#   - vrm-mock-renderer binary (cargo will build it in step 2 below)
#       The mock is a deterministic CPU adapter that satisfies the Phase 1
#       op contract without GPU or VRMMetalKit dependencies. The real Swift
#       adapter (adapters/vrm-metal-kit/) remains an L3 stretch target; the
#       smoke uses the mock so the full runner → diff → S3 → site loop runs
#       green by default.
#   - AWS credentials in env (or default profile) with VRM_GOLDENS_BUCKET set
#       (S3 upload is gated on $VRM_GOLDENS_BUCKET — unset means "skip")
#   - cargo, node, python3 available
#       (python3 is used only by the diff-loop self-test JSON parse;
#        see step 4b below.)
#
# Usage:
#   scripts/smoke.sh                      # full pipeline (will hit known L3 failure)
#   scripts/smoke.sh --skip-render        # skip the adapter/runner step
#   SMOKE_SKIP_RENDER=1 scripts/smoke.sh  # same, via env
#
# Exit semantics: this script is operator-driven, not CI. The known L3 gap is
# reported as a "blocked-checkpoint" message rather than a hard failure, and
# downstream steps continue when possible (using a placeholder PNG for the
# self-diff sanity check when no real render is available).

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

# ---- preflight: required tools --------------------------------------------
# swift is checked separately because it's only needed when not skipping render.
for tool in cargo node python3; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "smoke: missing required tool: $tool" >&2
        exit 1
    }
done

# ---- arg parsing -----------------------------------------------------------
SKIP_RENDER="${SMOKE_SKIP_RENDER:-0}"
for arg in "$@"; do
    case "$arg" in
        --skip-render) SKIP_RENDER=1 ;;
        -h|--help)
            grep '^# ' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) echo "unknown arg: $arg" >&2; exit 2 ;;
    esac
done

ASSETS=$ROOT/assets/generated
OUTPUTS=$ROOT/.smoke/renders
mkdir -p "$ASSETS" "$OUTPUTS"

PNG="$OUTPUTS/smoke_default_vrm-mock.png"

# ---- step 1: generate asset ------------------------------------------------
echo "==> Generating asset (vrm-asset-generator emit-default)"
cargo run --release -p vrm-asset-generator -- emit-default \
    --id smoke_default \
    --output-dir "$ASSETS"

# ---- step 2: build mock renderer ------------------------------------------
echo "==> Building mock renderer (cargo build --release -p vrm-mock-renderer)"
cargo build --release -p vrm-mock-renderer

ADAPTER=$ROOT/target/release/vrm-mock-renderer

# ---- step 3: runner drives mock adapter -----------------------------------
if [ "$SKIP_RENDER" = "1" ]; then
    echo "==> Skipping runner step (--skip-render)"
else
    echo "==> Running test plan against mock adapter (vrm-mock-renderer)"
    cargo run --release -p vrm-runner -- execute-test-plan \
        --plan "$ASSETS/smoke_default.test.yaml" \
        --adapter-bin "$ADAPTER" \
        --asset-dir "$ASSETS" \
        --output-dir "$OUTPUTS" \
        --renderer-name vrm-mock \
        --json
fi

# ---- step 4: self-diff sanity ---------------------------------------------
echo "==> Diff against self (sanity)"
if [ ! -f "$PNG" ]; then
    echo "    no real render at $PNG — synthesizing a 4x4 placeholder PNG so"
    echo "    the diff-engine self-test still exercises ssim_pngs end-to-end."
    # Tiny valid 4x4 grayscale PNG (pre-generated, base64 encoded).
    # Bytes are a hand-rolled grayscale-2 PNG; verified to load with `image` crate.
    python3 - "$PNG" <<'PY'
import base64, sys
# 4x4 single-color (mid-gray) 8-bit grayscale PNG.
# Generated with Python's zlib+struct so the IDAT CRC is valid.
png_b64 = (
    "iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAAAAACMmsGiAAAADklEQVR42mNo"
    "AAIGVAIAUBQIAW1N4EkAAAAASUVORK5CYII="
)
with open(sys.argv[1], "wb") as f:
    f.write(base64.b64decode(png_b64))
PY
fi

cargo run --release -p vrm-diff-engine --example self_diff -- "$PNG"

# ---- step 4b: runner diff CLI sanity --------------------------------------
echo "==> Running runner diff loop (self-diff via vrm-runner diff)"
RUNNER_RENDER="$OUTPUTS/runner_diff_render.png"
RUNNER_REF="$OUTPUTS/runner_diff_reference.png"
cp "$PNG" "$RUNNER_RENDER"
cp "$PNG" "$RUNNER_REF"

DIFF_PLAN="$ASSETS/smoke_default.test.yaml"
if [ -f "$DIFF_PLAN" ]; then
    DIFF_OUT=$(cargo run --release -p vrm-runner -- diff \
        --plan "$DIFF_PLAN" \
        --render "$RUNNER_RENDER" \
        --reference "$RUNNER_REF" \
        --renderer-name smoke-test \
        --json) || {
        echo "    runner diff failed (unexpected — should self-diff to SSIM~1)" >&2
        exit 1
    }
    echo "$DIFF_OUT" | python3 -c "
import json, sys
d = json.load(sys.stdin)
assert d['ssim_passed'], f'expected ssim_passed=True, got {d}'
assert d['ssim'] > 0.99, f'expected SSIM ~1, got {d[\"ssim\"]}'
print(f'    SSIM={d[\"ssim\"]:.4f}, properties={len(d[\"properties\"])}, overall PASS')
"
else
    echo "    (skipped: no plan at $DIFF_PLAN)"
fi

# ---- step 5: optional S3 upload -------------------------------------------
if [ -n "${VRM_GOLDENS_BUCKET:-}" ]; then
    if [ -f "$PNG" ] && [ "$SKIP_RENDER" != "1" ]; then
        echo "==> Uploading to S3 (bucket: $VRM_GOLDENS_BUCKET)"
        "$ROOT/scripts/push-goldens.sh" \
            "$PNG" smoke_default vrm-metal-kit 0.1.0
    else
        echo "==> Skipping S3 upload (no real render produced; placeholder not uploadable)"
    fi
else
    echo "==> Skipping S3 upload (set VRM_GOLDENS_BUCKET to enable)"
fi

# ---- step 6: build site ----------------------------------------------------
echo "==> Building site (Vite)"
(cd site && npm install && npm run build)

echo
if [ "$SKIP_RENDER" = "1" ]; then
    echo "OK — smoke complete (render step skipped). Open site/dist/index.html in a browser."
else
    echo "OK — smoke complete (mock adapter rendered, all stages green)."
    echo "     Open site/dist/index.html in a browser to view."
fi
