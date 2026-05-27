#!/usr/bin/env bash
# One-shot migration: backfill spec_version: "1.0" on every existing
# manifest entry that lacks it. Idempotent — re-running is a no-op.
#
# Usage: scripts/migrate-manifest-spec-version.sh [manifest.json]
set -euo pipefail
MANIFEST="${1:-goldens/manifest.json}"
TMP="$(mktemp)"

if ! command -v jq >/dev/null; then
    echo "error: jq is required" >&2
    exit 1
fi

# For every entry that lacks spec_version, add it set to "1.0".
jq '(.entries[] | select(.spec_version == null)) |= (. + {spec_version: "1.0"})' \
    "$MANIFEST" > "$TMP"

mv "$TMP" "$MANIFEST"
echo "Backfilled spec_version on $(jq '.entries | length' "$MANIFEST") entries in $MANIFEST"
