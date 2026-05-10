#!/usr/bin/env bash
set -euo pipefail

# Wraps the cargo-built push-goldens binary with sensible defaults pulled from
# `git rev-parse` and `uname`. Called from CI and from operator machines after
# a local render run.

cargo run --release --bin push-goldens -p vrm-s3 -- \
    --file "${1:?usage: push-goldens.sh <png> <test_id> <renderer_name> <renderer_version>}" \
    --test-id "${2:?missing test_id}" \
    --renderer-name "${3:?missing renderer_name}" \
    --renderer-version "${4:?missing renderer_version}" \
    --git-hash "$(git rev-parse HEAD)" \
    --os "$(uname -s | tr '[:upper:]' '[:lower:]')" \
    --os-version "$(uname -r)" \
    --gpu-vendor "${VRM_GPU_VENDOR:-unknown}" \
    --gpu-model "${VRM_GPU_MODEL:-unknown}" \
    --driver-version "${VRM_GPU_DRIVER:-}" \
    --build-flags "release"
