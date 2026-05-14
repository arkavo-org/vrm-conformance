#!/usr/bin/env bash
#
# Render the humanoid test plans (test-plans/manual/humanoid/) through
# every available adapter and report a 3-way (or N-way) consensus diff.
#
# Unlike bootstrap-goldens.sh, this script does not push to a manifest —
# the humanoid corpus is locally rendered, locally diffed. Use it to
# catch regressions on humanoid features that the procedural sphere
# corpus can't surface (face/eye shading, bust spring-bones, etc.).
#
# Prerequisite:
#   scripts/install-humanoid-fixtures.sh   (materializes assets/humanoid/)
#
# Env:
#   SKIP_THREE_VRM=1     Skip three-vrm
#   SKIP_VRM_METAL_KIT=1 Skip vrm-metal-kit
#   SKIP_GODOT_VRM=1     Skip godot-vrm

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
cd "$ROOT"

PLANS_DIR="$ROOT/test-plans/manual/humanoid"
ASSETS_DIR="$ROOT/assets/humanoid"
OUT_ROOT="$ROOT/goldens-cache/humanoid"

if [ ! -L "$ASSETS_DIR/avatarA_1_0.vrm" ] && [ ! -f "$ASSETS_DIR/avatarA_1_0.vrm" ]; then
    echo "render-humanoid: fixture avatarA_1_0.vrm not installed." >&2
    echo "                 Run scripts/install-humanoid-fixtures.sh first." >&2
    exit 2
fi

echo "==> Building binaries (release)"
cargo build --release -q -p vrm-runner --bin vrm-runner

RUNNER="$ROOT/target/release/vrm-runner"
VMK="$ROOT/adapters/vrm-metal-kit/.build/release/vrm-metal-kit-adapter"
THREE="$ROOT/adapters/three-vrm/dist/main.js"
GODOT_SHIM="$ROOT/target/release/vrm-godot-shim"

mkdir -p "$OUT_ROOT"

# Build adapter list dynamically.
ADAPTERS=()
if [ "${SKIP_VRM_METAL_KIT:-0}" != "1" ] && [ -x "$VMK" ]; then
    ADAPTERS+=("vrm-metal-kit|$VMK|")
fi
if [ "${SKIP_THREE_VRM:-0}" != "1" ] && [ -f "$THREE" ] && command -v node >/dev/null; then
    ADAPTERS+=("three-vrm|$(command -v node)|$THREE")
fi
if [ "${SKIP_GODOT_VRM:-0}" != "1" ] && [ -x "$GODOT_SHIM" ] && command -v godot >/dev/null; then
    ADAPTERS+=("godot-vrm|$GODOT_SHIM|")
fi

if [ "${#ADAPTERS[@]}" -lt 2 ]; then
    echo "render-humanoid: need ≥2 adapters for consensus; got ${#ADAPTERS[@]}." >&2
    exit 3
fi

PLANS=()
while IFS= read -r p; do PLANS+=("$p"); done < <(find "$PLANS_DIR" -maxdepth 1 -name '*.test.yaml' | sort)

for plan in "${PLANS[@]}"; do
    tid=$(basename "$plan" .test.yaml)
    echo
    echo "==> $tid"
    render_args=()
    for entry in "${ADAPTERS[@]}"; do
        IFS='|' read -r name bin extra <<< "$entry"
        out_dir="$OUT_ROOT/$name"
        mkdir -p "$out_dir"
        args=(execute-test-plan --plan "$plan" --adapter-bin "$bin"
              --asset-dir "$ASSETS_DIR" --output-dir "$out_dir"
              --renderer-name "$name" --json)
        if [ -n "$extra" ]; then
            args+=(--adapter-args "$extra")
        fi
        if "$RUNNER" "${args[@]}" >/dev/null 2>&1; then
            png="$out_dir/${tid}.png"
            [ -f "$out_dir/${tid}_${name}.png" ] && mv -f "$out_dir/${tid}_${name}.png" "$png"
            render_args+=("--render" "${name}=${png}")
            echo "    $name: rendered"
        else
            echo "    $name: FAILED" >&2
        fi
    done

    if [ "${#render_args[@]}" -ge 4 ]; then
        # consensus-diff exits 1 on consensus failure (valid data point, not
        # a runner error). Pipe through python and only abort on hard errors.
        { "$RUNNER" consensus-diff --plan "$plan" --json "${render_args[@]}" 2>/dev/null || true; } \
            | python3 -c "
import json, sys
d = json.load(sys.stdin)
rs = d['renderers']
m = d['ssim_matrix']
print(f'    consensus_passed={d[\"consensus_passed\"]} threshold={d[\"threshold\"]}')
print(f'    outliers: {d.get(\"outliers\", [])}')
for i in range(len(rs)):
    for j in range(i+1, len(rs)):
        print(f'      {rs[i]:14s} vs {rs[j]:14s}: {m[i][j]:.4f}')
"
    fi
done

echo
echo "==> Renders in $OUT_ROOT/"
