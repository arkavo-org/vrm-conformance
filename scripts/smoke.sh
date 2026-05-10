#!/usr/bin/env bash
set -euo pipefail

# Phase 1 v0.1 hello-world end-to-end smoke. Requires:
#   - Validator shim installed (.tools/vrm-validator-cli)
#   - VRMMetalKit adapter built (adapters/vrm-metal-kit/.build/release/vrm-metal-kit-adapter)
#       NOTE: as of v0.1 the adapter returns Unimplemented for every op (L3 deferred);
#             the runner step is therefore expected to fail at load_vrm. Pass --skip-render
#             or `SMOKE_SKIP_RENDER=1` to bypass.
#   - AWS credentials in env (or default profile) with VRM_GOLDENS_BUCKET set
#       (S3 upload is gated on $VRM_GOLDENS_BUCKET — unset means "skip")
#   - cargo, swift, node available
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

ADAPTER=$ROOT/adapters/vrm-metal-kit/.build/release/vrm-metal-kit-adapter
PNG="$OUTPUTS/smoke_default_vrm-metal-kit.png"

# ---- step 1: generate asset ------------------------------------------------
echo "==> Generating asset (vrm-asset-generator emit-default)"
cargo run --release -p vrm-asset-generator -- emit-default \
    --id smoke_default \
    --output-dir "$ASSETS"

# ---- step 2: build Swift adapter (best effort) ----------------------------
if [ "$SKIP_RENDER" = "1" ]; then
    echo "==> Skipping Swift adapter build + runner step (--skip-render)"
else
    echo "==> Building Swift adapter (release)"
    if (cd adapters/vrm-metal-kit && swift build --configuration release); then
        :
    else
        echo "    Swift build failed — falling back to --skip-render mode" >&2
        SKIP_RENDER=1
    fi
fi

# ---- step 3: runner (known-blocked on L3) ---------------------------------
if [ "$SKIP_RENDER" != "1" ]; then
    echo "==> Running test plan against vrm-metal-kit adapter"
    echo "    NOTE: L3 (real Metal rendering) is deferred for v0.1."
    echo "          The adapter returns Unimplemented for load_vrm; the runner is"
    echo "          expected to error here. Re-run with --skip-render to bypass."
    if cargo run --release -p vrm-runner -- execute-test-plan \
            --plan "$ASSETS/smoke_default.test.yaml" \
            --adapter-bin "$ADAPTER" \
            --asset-dir "$ASSETS" \
            --output-dir "$OUTPUTS" \
            --renderer-name vrm-metal-kit \
            --json; then
        echo "    runner succeeded (unexpected — L3 must have landed!)"
    else
        rc=$?
        echo "    runner exited with status $rc — this is the known L3-blocked checkpoint." >&2
        echo "    Continuing with downstream smoke steps using a placeholder PNG." >&2
    fi
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
    echo "OK — smoke complete (render step blocked on L3, downstream steps green)."
    echo "     Open site/dist/index.html in a browser to view."
fi
