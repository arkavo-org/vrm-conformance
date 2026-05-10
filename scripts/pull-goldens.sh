#!/usr/bin/env bash
#
# Pull every entry in goldens/manifest.json from S3 to a local mirror,
# verifying BLAKE3 content addressing for each file. Wraps the
# cargo-built pull-goldens binary.
#
# Usage:
#   scripts/pull-goldens.sh <output-dir>
#   scripts/pull-goldens.sh <output-dir> <manifest-path>
#   scripts/pull-goldens.sh /tmp/goldens-mirror
#   scripts/pull-goldens.sh /tmp/goldens-mirror goldens/manifest.json
#
# Requires:
#   - AWS credentials in env (or default profile) with s3:GetObject access
#     to the bucket(s) referenced by manifest entries.
#   - cargo + a built vrm-s3 crate (the script runs `cargo run --release`).

set -euo pipefail

OUTPUT_DIR="${1:?usage: pull-goldens.sh <output-dir> [manifest-path]}"
MANIFEST="${2:-goldens/manifest.json}"

if [ ! -f "${MANIFEST}" ]; then
    echo "pull-goldens: manifest not found: ${MANIFEST}" >&2
    echo "             pass it as the second argument, or create" >&2
    echo "             goldens/manifest.json (see RFC-0002)." >&2
    exit 2
fi

exec cargo run --release --bin pull-goldens -p vrm-s3 -- \
    --manifest "${MANIFEST}" \
    --output-dir "${OUTPUT_DIR}"
