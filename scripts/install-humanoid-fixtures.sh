#!/usr/bin/env bash
#
# Materialize the humanoid fixture (AvatarSample_A) into assets/humanoid/
# so the test plans in test-plans/manual/humanoid/ can run.
#
# The asset is NOT redistributed by this repo — it ships under the VRM
# Platform License 1.0 (see VMK's LICENSE-MODELS.md). This script locates
# an existing copy and symlinks it.
#
# Resolution order:
#   1. $VRM_HUMANOID_FIXTURE points at the .vrm.glb directly
#   2. $VMK_REPO/AvatarSample_A_1.0.vrm.glb (env var to a VRMMetalKit clone)
#   3. ~/Projects/VRMMetalKit/AvatarSample_A_1.0.vrm.glb (default checkout)
#
# Usage:
#   scripts/install-humanoid-fixtures.sh

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
DEST="$ROOT/assets/humanoid"
mkdir -p "$DEST"

find_fixture() {
    local basename="$1"
    if [ -n "${VRM_HUMANOID_FIXTURE:-}" ] && [ "$(basename "$VRM_HUMANOID_FIXTURE")" = "$basename" ]; then
        echo "$VRM_HUMANOID_FIXTURE"; return
    fi
    if [ -n "${VMK_REPO:-}" ] && [ -f "$VMK_REPO/$basename" ]; then
        echo "$VMK_REPO/$basename"; return
    fi
    if [ -f "$HOME/Projects/VRMMetalKit/$basename" ]; then
        echo "$HOME/Projects/VRMMetalKit/$basename"; return
    fi
    return 1
}

install_one() {
    local basename="$1"
    local destname="$2"
    local src
    if ! src=$(find_fixture "$basename"); then
        echo "  $basename: NOT FOUND (set VRM_HUMANOID_FIXTURE or VMK_REPO, or clone VRMMetalKit into ~/Projects/)" >&2
        return 1
    fi
    ln -sf "$src" "$DEST/$destname"
    echo "  $destname -> $src"
}

echo "==> Installing humanoid fixtures into $DEST"
install_one "AvatarSample_A_1.0.vrm.glb" "avatarA_1_0.vrm" || true
install_one "AvatarSample_A_0.0.vrm.glb" "avatarA_0_0.vrm" || true
install_one "AvatarSample_U_0.0.vrm.glb" "avatarU_0_0.vrm" || true
# Tier 2 canonical-content fixtures (RFC 0005): VRoid Studio exports with
# permissive license metadata set at export time. The .meta.json sidecar
# next to the symlink is committed to git (gitignore negation).
install_one "vroid_default_F_1_0.vrm" "vroid_default_F_1_0.vrm" || true

# Real-rig humanoid retarget triplet (vrm-conformance#17 / VMK#338): the
# avatarO/avatarA/avatarH x VRMA_01 plans discriminate the retarget defect
# (O: right-lower-leg deformation, A: whole-clip no-op, H: control). The
# clip is the official VRM sample animation VRMA_01 (full-body idle,
# 11.8 s) — third-party binary, symlinked like the .vrm fixtures.
install_one "AvatarSample_O_1.0.vrm" "avatarO_1_0.vrm" || true
install_one "AvatarSample_H_1.0.vrm" "avatarH_1_0.vrm" || true
install_one "VRMA_01.vrma" "VRMA_01_idle.vrma" || true

# Gaze VRMA clips for the VMK #332 eye look-at corpus (vroid_default_F_gaze_*).
# Generated next to the avatar so the runner's --asset-dir finds both.
echo "Emitting gaze VRMA clips (VMK #332 coverage)..."
cargo run -q -p vrm-asset-generator -- emit-gaze-sweep --output-dir "$DEST"

# Expression VRMA clips for the VMK #333 facial-expression corpus (vroid_default_F_expr_*).
echo "Emitting expression VRMA clips (VMK #333 coverage)..."
cargo run -q -p vrm-asset-generator -- emit-expression-clips --output-dir "$DEST"

echo
echo "==> Done. Run humanoid plans with:"
echo "    scripts/render-humanoid.sh           # all available adapters"
