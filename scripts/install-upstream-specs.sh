#!/usr/bin/env bash
#
# Install upstream spec mirrors under docs/upstream-specs/ for offline
# reading + grep. Both trees are gitignored; this script just shallow-
# clones them. Re-run any time you want the latest spec text.
#
# Usage:
#   scripts/install-upstream-specs.sh        # clone if missing
#   scripts/install-upstream-specs.sh -f     # delete + re-clone
#
# See docs/upstream-specs/README.md for what's there and why.

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
SPEC_DIR="$ROOT/docs/upstream-specs"
mkdir -p "$SPEC_DIR"

FORCE=0
if [ "${1:-}" = "-f" ] || [ "${1:-}" = "--force" ]; then
    FORCE=1
fi

clone_or_skip() {
    local repo_url="$1"
    local target_dir="$2"
    local name="$3"

    if [ -d "$target_dir/.git" ]; then
        if [ "$FORCE" = "1" ]; then
            echo "==> removing $name (force)"
            rm -rf "$target_dir"
        else
            echo "==> $name already present at $target_dir (use -f to re-clone)"
            return 0
        fi
    fi

    echo "==> cloning $name → $target_dir"
    git clone --depth 1 "$repo_url" "$target_dir"
}

clone_or_skip \
    https://github.com/vrm-c/vrm-specification.git \
    "$SPEC_DIR/vrm-specification" \
    "vrm-c/vrm-specification"

clone_or_skip \
    https://github.com/KhronosGroup/glTF.git \
    "$SPEC_DIR/glTF" \
    "KhronosGroup/glTF"

echo
echo "==> done. See $SPEC_DIR/README.md for what's there."
du -sh "$SPEC_DIR"/* 2>/dev/null || true
