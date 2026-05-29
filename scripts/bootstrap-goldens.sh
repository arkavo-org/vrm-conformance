#!/usr/bin/env bash
#
# Bootstrap-goldens: generate the full test corpus locally, render each
# asset through every available real renderer adapter, and populate
# `goldens/manifest.json` with one entry per (test_id, renderer) pair.
#
# Usage:
#   scripts/bootstrap-goldens.sh
#
# Env:
#   VRM_GOLDENS_BUCKET    If set + AWS creds are present, each rendered
#                         PNG is uploaded to that bucket and the manifest
#                         records s3:// URLs. Otherwise, manifest entries
#                         use file:// URLs pointing at the local cache.
#   SKIP_THREE_VRM=1      Skip the three-vrm adapter (e.g., if Playwright
#                         install is broken in this environment).
#   SKIP_VRM_METAL_KIT=1  Skip the vrm-metal-kit adapter (e.g., on Linux).
#   SKIP_GODOT_VRM=1      Skip the godot-vrm adapter (e.g., if godot is not
#                         on PATH).
#   GOLDENS_DIR=path      Override the on-disk cache location.
#                         Default: ./goldens-cache/
#   QUICK=1               Render only emit-default + emit-springbone (2
#                         assets total) instead of the full sweep.
#                         Useful for smoke-validating the pipeline.
#   SPEC_VERSION=0.x      Emit the VRM 0.x corpus instead of VRM 1.0.
#                         Output lands in a parallel <GOLDENS_DIR>_v0 tree
#                         so the 1.0 corpus is never clobbered. Sweeps that
#                         have no 0.x form are skipped with a clear note.
#                         Default: 1.0 (current behaviour, byte-identical).
#
# Output layout:
#   goldens-cache/<renderer-name>/<test_id>.png
#   goldens/manifest.json   (one ManifestEntry per (test_id, renderer))
#
# Re-runnable: re-rendering an existing entry upserts the manifest in
# place. PNGs are overwritten; BLAKE3 hashes get recomputed.

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
cd "$ROOT"

# ---------------------------------------------------------------------------
# SPEC_VERSION knob: default 1.0 (current behaviour, byte-identical).
# Set SPEC_VERSION=0.x to emit the 0.x corpus into a parallel _v0 staging
# tree so the VRM 1.0 corpus is never clobbered.
# ---------------------------------------------------------------------------
SPEC_VERSION="${SPEC_VERSION:-1.0}"
SPEC_FLAG=()
if [ "$SPEC_VERSION" = "0.x" ]; then
    SPEC_FLAG=(--spec-version 0.x)
    # All output dirs derive from GOLDENS_DIR, so suffixing it here isolates
    # the entire 0.x run without touching any downstream variable derivations.
    GOLDENS_DIR="${GOLDENS_DIR:-$ROOT/goldens-cache}_v0"
    echo "==> SPEC_VERSION=0.x: writing corpus into $GOLDENS_DIR"
else
    GOLDENS_DIR="${GOLDENS_DIR:-$ROOT/goldens-cache}"
fi
ASSETS_DIR="$GOLDENS_DIR/_assets"

mkdir -p "$GOLDENS_DIR" "$ROOT/goldens"

GIT_HASH=$(git rev-parse HEAD)
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
OS_VERSION=$(uname -r)
GPU_VENDOR="${VRM_GPU_VENDOR:-Apple}"
GPU_MODEL="${VRM_GPU_MODEL:-$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo unknown)}"

# Manifest path depends on mode. file:// URLs point at this machine's
# filesystem, so we never want a local-mode manifest in git; route those
# to the already-gitignored goldens-cache/ tree. S3-mode manifests are
# host-independent and live at goldens/manifest.json so reviewers can
# commit them.
if [ -n "${VRM_GOLDENS_BUCKET:-}" ]; then
    PUSH_MODE="s3"
    MANIFEST="$ROOT/goldens/manifest.json"
    echo "==> Bootstrap mode: S3 push (bucket: $VRM_GOLDENS_BUCKET)"
else
    PUSH_MODE="local"
    MANIFEST="$GOLDENS_DIR/local-manifest.json"
    echo "==> Bootstrap mode: local (file:// URLs; no S3 push)"
fi
echo "==> Manifest output: $MANIFEST"

echo "==> Building Rust binaries (release)"
cargo build --release \
    -p vrm-asset-generator --bin vrm-asset-generator \
    -p vrm-runner --bin vrm-runner \
    -p vrm-s3 --bin push-goldens >/dev/null

rm -rf "$ASSETS_DIR"
mkdir -p "$ASSETS_DIR"

if [ "${QUICK:-0}" = "1" ]; then
    echo "==> QUICK mode: emitting assets only"
    cargo run --release -q -p vrm-asset-generator -- emit-default \
        --id mtoon_default --output-dir "$ASSETS_DIR" ${SPEC_FLAG[@]+"${SPEC_FLAG[@]}"} >/dev/null
    if [ "$SPEC_VERSION" = "0.x" ]; then
        # emit-springbone has no --spec-version flag; use a second emit-default
        # for the spring-bone smoke asset at 0.x instead.
        echo "    SKIP emit-springbone: no --spec-version flag; emitting emit-default springbone_default_0x as smoke substitute"
        cargo run --release -q -p vrm-asset-generator -- emit-default \
            --id springbone_default_0x --output-dir "$ASSETS_DIR" ${SPEC_FLAG[@]+"${SPEC_FLAG[@]}"} >/dev/null
    else
        cargo run --release -q -p vrm-asset-generator -- emit-springbone \
            --id springbone_default --output-dir "$ASSETS_DIR" >/dev/null
    fi
else
    echo "==> Emitting MToon sweep"
    cargo run --release -q -p vrm-asset-generator -- emit-sweep \
        --output-dir "$ASSETS_DIR" --json ${SPEC_FLAG[@]+"${SPEC_FLAG[@]}"} >/dev/null

    echo "==> Emitting MToon emissive sweep (14 plans, VRMC_materials_hdr_emissiveMultiplier)"
    EMISSIVE_DIR="$GOLDENS_DIR/_assets_emissive"
    rm -rf "$EMISSIVE_DIR"; mkdir -p "$EMISSIVE_DIR"
    cargo run --release -q -p vrm-asset-generator -- emit-emissive-sweep \
        --output-dir "$EMISSIVE_DIR" --json ${SPEC_FLAG[@]+"${SPEC_FLAG[@]}"} >/dev/null

    echo "==> Emitting VRMC_vrm.firstPerson sweep (4 plans, third-person camera)"
    FIRSTPERSON_DIR="$GOLDENS_DIR/_assets_firstperson"
    rm -rf "$FIRSTPERSON_DIR"; mkdir -p "$FIRSTPERSON_DIR"
    cargo run --release -q -p vrm-asset-generator -- emit-first-person-sweep \
        --output-dir "$FIRSTPERSON_DIR" --json ${SPEC_FLAG[@]+"${SPEC_FLAG[@]}"} >/dev/null

    if [ "$SPEC_VERSION" != "0.x" ]; then
        echo "==> Emitting KHR_texture_transform sweep (8 plans, textured MToon)"
        UVXFORM_DIR="$GOLDENS_DIR/_assets_uvxform"
        rm -rf "$UVXFORM_DIR"; mkdir -p "$UVXFORM_DIR"
        cargo run --release -q -p vrm-asset-generator -- emit-texture-transform-sweep \
            --output-dir "$UVXFORM_DIR" --json >/dev/null
    else
        echo "    SKIP emit-texture-transform-sweep: NotApplicable at VRM 0.x (rejects --spec-version 0.x with a structured NotApplicableReason)"
        UVXFORM_DIR=""
    fi

    echo "==> Emitting MToon shadeMultiplyTexture sweep (6 plans, shaded-color path)"
    SHADETEX_DIR="$GOLDENS_DIR/_assets_shadetex"
    rm -rf "$SHADETEX_DIR"; mkdir -p "$SHADETEX_DIR"
    cargo run --release -q -p vrm-asset-generator -- emit-shade-multiply-texture-sweep \
        --output-dir "$SHADETEX_DIR" --json ${SPEC_FLAG[@]+"${SPEC_FLAG[@]}"} >/dev/null

    echo "==> Emitting MToon matcapTexture sweep (5 plans, view-space matcap)"
    MATCAP_DIR="$GOLDENS_DIR/_assets_matcap"
    rm -rf "$MATCAP_DIR"; mkdir -p "$MATCAP_DIR"
    cargo run --release -q -p vrm-asset-generator -- emit-matcap-texture-sweep \
        --output-dir "$MATCAP_DIR" --json ${SPEC_FLAG[@]+"${SPEC_FLAG[@]}"} >/dev/null

    if [ "$SPEC_VERSION" != "0.x" ]; then
        echo "==> Emitting MToon shadingShiftTexture sweep (5 plans, per-pixel boundary)"
        SHADINGSHIFTTEX_DIR="$GOLDENS_DIR/_assets_shadingshifttex"
        rm -rf "$SHADINGSHIFTTEX_DIR"; mkdir -p "$SHADINGSHIFTTEX_DIR"
        cargo run --release -q -p vrm-asset-generator -- emit-shading-shift-texture-sweep \
            --output-dir "$SHADINGSHIFTTEX_DIR" --json >/dev/null
    else
        echo "    SKIP emit-shading-shift-texture-sweep: NotApplicable at VRM 0.x (rejects --spec-version 0.x with a structured NotApplicableReason)"
        SHADINGSHIFTTEX_DIR=""
    fi

    if [ "$SPEC_VERSION" != "0.x" ]; then
        echo "==> Emitting MToon rimMultiplyTexture sweep (4 plans, parametric rim)"
        RIMTEX_DIR="$GOLDENS_DIR/_assets_rimtex"
        rm -rf "$RIMTEX_DIR"; mkdir -p "$RIMTEX_DIR"
        cargo run --release -q -p vrm-asset-generator -- emit-rim-multiply-texture-sweep \
            --output-dir "$RIMTEX_DIR" --json >/dev/null
    else
        echo "    SKIP emit-rim-multiply-texture-sweep: NotApplicable at VRM 0.x (rejects --spec-version 0.x with a structured NotApplicableReason)"
        RIMTEX_DIR=""
    fi

    echo "==> Emitting MToon outlineWidthMultiplyTexture sweep (5 plans, per-vertex width)"
    OUTLINETEX_DIR="$GOLDENS_DIR/_assets_outlinetex"
    rm -rf "$OUTLINETEX_DIR"; mkdir -p "$OUTLINETEX_DIR"
    cargo run --release -q -p vrm-asset-generator -- emit-outline-width-multiply-texture-sweep \
        --output-dir "$OUTLINETEX_DIR" --json ${SPEC_FLAG[@]+"${SPEC_FLAG[@]}"} >/dev/null

    echo "==> Emitting glTF-core PBR textures sweep (6 plans, normalTexture + occlusionTexture)"
    PBRTEX_DIR="$GOLDENS_DIR/_assets_pbrtex"
    rm -rf "$PBRTEX_DIR"; mkdir -p "$PBRTEX_DIR"
    cargo run --release -q -p vrm-asset-generator -- emit-pbr-textures-sweep \
        --output-dir "$PBRTEX_DIR" --json ${SPEC_FLAG[@]+"${SPEC_FLAG[@]}"} >/dev/null

    echo "==> Emitting spring-bone settle sweep"
    cargo run --release -q -p vrm-asset-generator -- emit-springbone-sweep \
        --output-dir "$ASSETS_DIR" --json ${SPEC_FLAG[@]+"${SPEC_FLAG[@]}"} >/dev/null

    echo "==> Emitting spring-bone swing sweep (separate dir; same .vrm bodies)"
    SWING_DIR="$GOLDENS_DIR/_assets_swing"
    rm -rf "$SWING_DIR"; mkdir -p "$SWING_DIR"
    cargo run --release -q -p vrm-asset-generator -- emit-springbone-swing-sweep \
        --output-dir "$SWING_DIR" --json ${SPEC_FLAG[@]+"${SPEC_FLAG[@]}"} >/dev/null

    # Phase 2-6 sweeps (VRMC_springBone gap closure design):
    #   - colliders: 24 cartesian × settle/swing = 48 plans
    #   - extended_colliders: 18 cartesian × settle/swing = 36 plans
    #   - gravity_dir: 4 × settle/swing = 8 plans
    #   - per-joint taper: 7 × settle/swing = 14 plans
    #   - multi-chain: 18 × settle/swing = 36 plans
    # Total new: 142 plans. Each sweep gets its own subdirectory so the runner's
    # `find ... -name '*.test.yaml'` picks them all up.
    echo "==> Emitting collider sweep (phase 2: 48 plans)"
    COLLIDER_DIR="$GOLDENS_DIR/_assets_collider"
    rm -rf "$COLLIDER_DIR"; mkdir -p "$COLLIDER_DIR"
    cargo run --release -q -p vrm-asset-generator -- emit-springbone-collider-sweep \
        --output-dir "$COLLIDER_DIR" --json ${SPEC_FLAG[@]+"${SPEC_FLAG[@]}"} >/dev/null

    if [ "$SPEC_VERSION" != "0.x" ]; then
        echo "==> Emitting extended-collider sweep (phase 3: 36 plans)"
        EXTENDED_DIR="$GOLDENS_DIR/_assets_extended"
        rm -rf "$EXTENDED_DIR"; mkdir -p "$EXTENDED_DIR"
        cargo run --release -q -p vrm-asset-generator -- emit-springbone-extended-sweep \
            --output-dir "$EXTENDED_DIR" --json >/dev/null
    else
        echo "    SKIP emit-springbone-extended-sweep: NotApplicable at VRM 0.x (rejects --spec-version 0.x with a structured NotApplicableReason)"
        EXTENDED_DIR=""
    fi

    echo "==> Emitting gravity-dir sweep (phase 4: 8 plans)"
    GRAVITY_DIR="$GOLDENS_DIR/_assets_gravity"
    rm -rf "$GRAVITY_DIR"; mkdir -p "$GRAVITY_DIR"
    cargo run --release -q -p vrm-asset-generator -- emit-springbone-gravity-dir-sweep \
        --output-dir "$GRAVITY_DIR" --json ${SPEC_FLAG[@]+"${SPEC_FLAG[@]}"} >/dev/null

    if [ "$SPEC_VERSION" != "0.x" ]; then
        echo "==> Emitting per-joint taper sweep (phase 5: 14 plans)"
        TAPER_DIR="$GOLDENS_DIR/_assets_taper"
        rm -rf "$TAPER_DIR"; mkdir -p "$TAPER_DIR"
        cargo run --release -q -p vrm-asset-generator -- emit-springbone-taper-sweep \
            --output-dir "$TAPER_DIR" --json >/dev/null
    else
        echo "    SKIP emit-springbone-taper-sweep: NotApplicable at VRM 0.x (rejects --spec-version 0.x with a structured NotApplicableReason)"
        TAPER_DIR=""
    fi

    echo "==> Emitting multi-chain sweep (phase 6: 36 plans)"
    MULTICHAIN_DIR="$GOLDENS_DIR/_assets_multichain"
    rm -rf "$MULTICHAIN_DIR"; mkdir -p "$MULTICHAIN_DIR"
    cargo run --release -q -p vrm-asset-generator -- emit-springbone-multichain-sweep \
        --output-dir "$MULTICHAIN_DIR" --json ${SPEC_FLAG[@]+"${SPEC_FLAG[@]}"} >/dev/null

    if [ "$SPEC_VERSION" != "0.x" ]; then
        echo "==> Emitting VRMA humanoid sweep (VRMA phase 3: 15 plans)"
        VRMA_HUMANOID_DIR="$GOLDENS_DIR/_assets_vrma_humanoid"
        rm -rf "$VRMA_HUMANOID_DIR"; mkdir -p "$VRMA_HUMANOID_DIR"
        cargo run --release -q -p vrm-asset-generator -- emit-vrma-humanoid-sweep \
            --output-dir "$VRMA_HUMANOID_DIR" --json >/dev/null
    else
        echo "    SKIP emit-vrma-humanoid-sweep: VRM 1.0-era VRMA; no 0.x form"
        VRMA_HUMANOID_DIR=""
    fi

    if [ "$SPEC_VERSION" != "0.x" ]; then
        echo "==> Emitting VRMA expression sweep (VRMA phase 3: 12 plans)"
        VRMA_EXPRESSION_DIR="$GOLDENS_DIR/_assets_vrma_expression"
        rm -rf "$VRMA_EXPRESSION_DIR"; mkdir -p "$VRMA_EXPRESSION_DIR"
        cargo run --release -q -p vrm-asset-generator -- emit-vrma-expression-sweep \
            --output-dir "$VRMA_EXPRESSION_DIR" --json >/dev/null
    else
        echo "    SKIP emit-vrma-expression-sweep: VRM 1.0-era VRMA; no 0.x form"
        VRMA_EXPRESSION_DIR=""
    fi

    if [ "$SPEC_VERSION" != "0.x" ]; then
        echo "==> Emitting VRMA lookAt sweep (VRMA phase 3: 10 plans, dual avatar configs)"
        VRMA_LOOKAT_DIR="$GOLDENS_DIR/_assets_vrma_lookat"
        rm -rf "$VRMA_LOOKAT_DIR"; mkdir -p "$VRMA_LOOKAT_DIR"
        cargo run --release -q -p vrm-asset-generator -- emit-vrma-lookat-sweep \
            --output-dir "$VRMA_LOOKAT_DIR" --json >/dev/null
    else
        echo "    SKIP emit-vrma-lookat-sweep: VRM 1.0-era VRMA; no 0.x form"
        VRMA_LOOKAT_DIR=""
    fi

    echo "==> Emitting VMK#162 coupling sweep (4 variants × settle + swing = 8 plans)"
    VMK162_DIR="$GOLDENS_DIR/_assets_vmk162"
    rm -rf "$VMK162_DIR"; mkdir -p "$VMK162_DIR"
    cargo run --release -q -p vrm-asset-generator -- emit-springbone-coupling-sweep \
        --output-dir "$VMK162_DIR" --json ${SPEC_FLAG[@]+"${SPEC_FLAG[@]}"} >/dev/null
fi

# mapfile -t TEST_PLANS isn't available on macOS's bundled bash 3.2.
# Use a portable read loop instead.
TEST_PLANS=()
while IFS= read -r line; do
    TEST_PLANS+=("$line")
done < <(find "$GOLDENS_DIR" -maxdepth 3 -name '*.test.yaml' | sort)
echo "==> ${#TEST_PLANS[@]} test plans found"

render_with_adapter() {
    local renderer_name="$1"
    local renderer_version="$2"
    local adapter_bin="$3"
    shift 3
    local adapter_args=("$@")

    local out_dir="$GOLDENS_DIR/$renderer_name"
    mkdir -p "$out_dir"

    local count=0
    local failed=0
    for plan in "${TEST_PLANS[@]}"; do
        local plan_dir
        plan_dir=$(dirname "$plan")
        local test_id
        test_id=$(basename "$plan" .test.yaml)
        local png="$out_dir/${test_id}.png"

        count=$((count + 1))
        if [ $((count % 10)) -eq 1 ] || [ "${QUICK:-0}" = "1" ]; then
            echo "    [$count/${#TEST_PLANS[@]}] $renderer_name -> $test_id"
        fi

        # The runner writes <output-dir>/<test_id>_<renderer-name>.png by
        # convention. Move to our canonical <out_dir>/<test_id>.png.
        local runner_out="$out_dir/${test_id}_${renderer_name}.png"
        local run_args=(
            --plan "$plan"
            --adapter-bin "$adapter_bin"
            --asset-dir "$plan_dir"
            --output-dir "$out_dir"
            --renderer-name "$renderer_name"
            --json
        )
        if [ "${#adapter_args[@]}" -gt 0 ]; then
            for a in "${adapter_args[@]}"; do
                run_args+=(--adapter-args "$a")
            done
        fi
        if cargo run --release -q -p vrm-runner -- execute-test-plan "${run_args[@]}" >/dev/null 2>&1; then
            if [ -f "$runner_out" ]; then
                mv -f "$runner_out" "$png"
            fi
        else
            failed=$((failed + 1))
            echo "    [$count/${#TEST_PLANS[@]}] FAIL $renderer_name $test_id" >&2
            continue
        fi

        local push_args=(
            --file "$png"
            --test-id "$test_id"
            --renderer-name "$renderer_name"
            --renderer-version "$renderer_version"
            --git-hash "$GIT_HASH"
            --os "$OS"
            --os-version "$OS_VERSION"
            --gpu-vendor "$GPU_VENDOR"
            --gpu-model "$GPU_MODEL"
            --manifest "$MANIFEST"
        )
        if [ "$PUSH_MODE" = "local" ]; then
            push_args+=(--local)
        fi
        cargo run --release -q -p vrm-s3 --bin push-goldens -- "${push_args[@]}" >/dev/null
    done

    echo "==> $renderer_name: $((count - failed)) succeeded, $failed failed"
}

if [ "${SKIP_VRM_METAL_KIT:-0}" != "1" ] && [ "$OS" = "darwin" ]; then
    echo "==> Building vrm-metal-kit adapter"
    (cd adapters/vrm-metal-kit \
        && swift build --configuration release 2>&1 | tail -2)
    VMK_BIN="$ROOT/adapters/vrm-metal-kit/.build/release/vrm-metal-kit-adapter"
    if [ -x "$VMK_BIN" ]; then
        render_with_adapter "vrm-metal-kit" "0.1.0" "$VMK_BIN"
    else
        echo "    (skipping vrm-metal-kit: binary not built)" >&2
    fi
else
    echo "==> Skipping vrm-metal-kit (not macOS or SKIP_VRM_METAL_KIT=1)"
fi

if [ "${SKIP_THREE_VRM:-0}" != "1" ]; then
    echo "==> Building three-vrm adapter"
    (cd adapters/three-vrm \
        && npm install --silent \
        && npm run build --silent \
        && npx --yes playwright install chromium >/dev/null 2>&1) \
        || { echo "    (three-vrm build failed; skipping)" >&2; }
    TVM_BIN="$ROOT/adapters/three-vrm/dist/main.js"
    if [ -f "$TVM_BIN" ]; then
        render_with_adapter "three-vrm" "3.5.0" "$(command -v node)" "$TVM_BIN"
    else
        echo "    (skipping three-vrm: dist/main.js not built)" >&2
    fi
else
    echo "==> Skipping three-vrm (SKIP_THREE_VRM=1)"
fi

if [ "${SKIP_GODOT_VRM:-0}" != "1" ]; then
    echo "==> Building vrm-godot-shim"
    cargo build --release -q -p vrm-godot-shim >/dev/null
    GVRM_BIN="$ROOT/target/release/vrm-godot-shim"
    if [ -x "$GVRM_BIN" ] && command -v godot >/dev/null 2>&1; then
        render_with_adapter "godot-vrm" "0.1.0" "$GVRM_BIN"
    else
        if [ ! -x "$GVRM_BIN" ]; then
            echo "    (skipping godot-vrm: shim binary not built)" >&2
        else
            echo "    (skipping godot-vrm: godot not on PATH)" >&2
        fi
    fi
else
    echo "==> Skipping godot-vrm (SKIP_GODOT_VRM=1)"
fi

# UniVRM uses the batched adapter contract (one Unity invocation for the
# whole batch) — execute-test-batch instead of execute-test-plan. See
# rfcs/0003-engine-idiom-divergence.md.
if [ "${RUN_UNIVRM:-0}" = "1" ] && [ "$OS" = "darwin" ]; then
    UNIVRM_BIN_DEFAULT="/Applications/Unity/Hub/Editor/6000.4.6f1/Unity.app/Contents/MacOS/Unity"
    UNITY_BIN_PATH="${UNITY_BIN:-$UNIVRM_BIN_DEFAULT}"
    if [ -x "$UNITY_BIN_PATH" ]; then
        echo "==> UniVRM (batched, Unity 6 + UniVRM v0.131.0)"
        UNIVRM_OUT="$GOLDENS_DIR/univrm"
        mkdir -p "$UNIVRM_OUT"

        # Stage every _assets* sweep dir into one plans dir so a single
        # batch covers the full corpus and writes a single
        # local-manifest.json. Includes .vrma files for VRMA test plans
        # (the runner absolutizes animation.vrma.path against the .vrm
        # asset's parent dir, so symlinks here resolve correctly).
        UNIVRM_STAGE="$GOLDENS_DIR/_univrm_stage"
        rm -rf "$UNIVRM_STAGE" && mkdir -p "$UNIVRM_STAGE"
        for src in \
            "$ASSETS_DIR" \
            "${SWING_DIR:-}" \
            "${COLLIDER_DIR:-}" \
            "${EXTENDED_DIR:-}" \
            "${GRAVITY_DIR:-}" \
            "${TAPER_DIR:-}" \
            "${MULTICHAIN_DIR:-}" \
            "${VRMA_HUMANOID_DIR:-}" \
            "${VRMA_EXPRESSION_DIR:-}" \
            "${VRMA_LOOKAT_DIR:-}" \
            "${EMISSIVE_DIR:-}" \
            "${FIRSTPERSON_DIR:-}" \
            "${UVXFORM_DIR:-}" \
            "${SHADETEX_DIR:-}" \
            "${MATCAP_DIR:-}" \
            "${SHADINGSHIFTTEX_DIR:-}" \
            "${RIMTEX_DIR:-}" \
            "${OUTLINETEX_DIR:-}" \
            "${PBRTEX_DIR:-}" \
            "${VMK162_DIR:-}"; do
            [ -z "$src" ] && continue
            [ ! -d "$src" ] && continue
            for f in "$src"/*.test.yaml "$src"/*.vrm "$src"/*.vrma; do
                [ ! -f "$f" ] && continue
                ln -sf "$f" "$UNIVRM_STAGE/$(basename "$f")"
            done
        done
        cargo run --release -q -p vrm-runner -- execute-test-batch \
            --plans "$UNIVRM_STAGE" \
            --adapter-bin "$ROOT/adapters/univrm/launcher.sh" \
            --output-dir "$UNIVRM_OUT" \
            --renderer-name univrm \
            --json 2>&1 | tail -3

        # Push each ok PNG to the goldens manifest.
        if [ -f "$UNIVRM_OUT/local-manifest.json" ]; then
            # Read directly from the file — never embed JSON in a python
            # triple-quoted string. Entry error_message fields contain
            # multi-line stack traces with control characters that break
            # python heredoc parsing.
            ok_test_ids=$(python3 -c "
import json, sys
with open(sys.argv[1]) as f:
    m = json.load(f)
for e in m.get('entries', []):
    if e.get('status') == 'ok':
        print(e['test_id'])
" "$UNIVRM_OUT/local-manifest.json")
            echo "    pushing $(echo "$ok_test_ids" | wc -w | tr -d ' ') univrm PNGs to manifest"
            for test_id in $ok_test_ids; do
                png="$UNIVRM_OUT/${test_id}.png"
                [ ! -f "$png" ] && continue
                push_args=(
                    --file "$png"
                    --test-id "$test_id"
                    --renderer-name univrm
                    --renderer-version v0.131.0
                    --git-hash "$GIT_HASH"
                    --os "$OS"
                    --os-version "$OS_VERSION"
                    --gpu-vendor "$GPU_VENDOR"
                    --gpu-model "$GPU_MODEL"
                    --manifest "$MANIFEST"
                )
                if [ "$PUSH_MODE" = "local" ]; then
                    push_args+=(--local)
                fi
                cargo run --release -q -p vrm-s3 --bin push-goldens -- "${push_args[@]}" >/dev/null
            done
        fi
    else
        echo "==> Skipping UniVRM (Unity not at $UNITY_BIN_PATH; set UNITY_BIN env)"
    fi
elif [ "${RUN_UNIVRM:-0}" = "1" ]; then
    echo "==> Skipping UniVRM (not macOS)"
else
    echo "==> Skipping UniVRM (set RUN_UNIVRM=1 to enable)"
fi

echo
if [ -f "$MANIFEST" ]; then
    COUNT=$(python3 -c "import json; print(len(json.load(open('$MANIFEST'))['entries']))")
    echo "==> $MANIFEST: $COUNT entries"
    python3 -c "
import json, collections
m = json.load(open('$MANIFEST'))
c = collections.Counter(e['renderer_name'] for e in m['entries'])
for name, n in sorted(c.items()):
    print(f'    {name}: {n}')
"
else
    echo "==> No entries written (all adapters skipped or all renders failed)"
fi

# VMK#162 coupling-matrix detector. Runs per-adapter against the
# coupling sweep emitted above. Failing this matrix means a per-joint
# drift exceeded the calibrated threshold — investigate before merging.
# Locked per-renderer signatures live in
# `docs/upstream/VMK-162-suite-side-detector.md`.
VMK162_MATRIX="$ROOT/test-plans/manual/coupling/vmk162_swing_coupling.matrix.yaml"
VMK162_ASSETS="${VMK162_DIR:-$GOLDENS_DIR/_assets_vmk162}"
if [ -f "$VMK162_MATRIX" ] && [ -d "$VMK162_ASSETS" ]; then
    echo
    echo "==> VMK#162 coupling matrix (per-adapter)"
    matrix_for_adapter() {
        local label="$1" bin="$2" args="$3"
        local outdir="$GOLDENS_DIR/_vmk162_matrix/$label"
        mkdir -p "$outdir"
        local run_args=(execute-test-plan-matrix --matrix "$VMK162_MATRIX"
            --adapter-bin "$bin" --asset-dir "$VMK162_ASSETS"
            --output-dir "$outdir" --renderer-name "$label" --json)
        [ -n "$args" ] && run_args+=(--adapter-args "$args")
        local result
        result=$(cargo run --release -q -p vrm-runner -- "${run_args[@]}" 2>/dev/null | tail -1)
        echo "$result" | python3 -c "
import json, sys
try: d = json.loads(sys.stdin.read())
except: print('    $label: parse failed'); sys.exit(0)
status = 'PASS' if d.get('overall_passed') else 'FAIL'
print(f'    {status} $label (threshold={d.get(\"coupling_threshold_m\", 0):.3f} m):')
for o in d.get('outcomes', []):
    print(f'      {o[\"name\"]:45s} max_drift={o.get(\"max_drift_m\", 0):.4f} m')
"
    }
    if [ "${SKIP_VRM_METAL_KIT:-0}" != "1" ] && [ -x "${VMK_BIN:-}" ]; then
        matrix_for_adapter "vrm-metal-kit" "$VMK_BIN" ""
    fi
    if [ "${SKIP_THREE_VRM:-0}" != "1" ] && [ -f "${TVM_BIN:-}" ]; then
        matrix_for_adapter "three-vrm" "$(command -v node)" "$TVM_BIN"
    fi
    if [ "${SKIP_GODOT_VRM:-0}" != "1" ] && [ -x "${GVRM_BIN:-}" ] && command -v godot >/dev/null 2>&1; then
        matrix_for_adapter "godot-vrm" "$GVRM_BIN" ""
    fi
fi

if [ "$PUSH_MODE" = "local" ]; then
    echo
    echo "==> Bootstrap complete (local mode). To publish to S3:"
    echo "    1. Set VRM_GOLDENS_BUCKET=<bucket> in your environment"
    echo "    2. Configure AWS credentials (e.g., aws configure)"
    echo "    3. Re-run this script — it will upload + rewrite manifest URLs"
    echo "       (writing to goldens/manifest.json instead of the local path above)"
fi
