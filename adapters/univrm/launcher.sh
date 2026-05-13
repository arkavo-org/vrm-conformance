#!/usr/bin/env bash
# UniVRM adapter launcher — invokes Unity Editor in batchmode to run
# Conformance.RunBatch against a manifest. Filesystem-as-protocol;
# arguments: $1 = manifest.json, $2 = results.ndjson.
#
# UNITY_BIN env override: explicit path to the Unity binary.
# UNITY_VERSION env: which Hub-installed version to default to
#   (only used if UNITY_BIN is unset).
#
# Defaults to 6000.4.6f1 (Unity 6 LTS — the version pinned in
# UniVRMConformance/ProjectSettings/ProjectVersion.txt). If Unity isn't
# installed at the default path, the launcher prints a clear error and
# exits 127 — the runner reports this as a batch-level failure.

set -euo pipefail

manifest="${1:?manifest path required}"
results="${2:?results path required}"

UNITY_VERSION="${UNITY_VERSION:-6000.4.6f1}"
UNITY_BIN_DEFAULT="/Applications/Unity/Hub/Editor/${UNITY_VERSION}/Unity.app/Contents/MacOS/Unity"
UNITY_BIN="${UNITY_BIN:-$UNITY_BIN_DEFAULT}"

if [ ! -x "$UNITY_BIN" ]; then
  echo "error: Unity binary not executable at $UNITY_BIN" >&2
  echo "       set UNITY_BIN env to the Unity binary path, or install" >&2
  echo "       Unity $UNITY_VERSION via Unity Hub." >&2
  exit 127
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_path="$script_dir/UniVRMConformance"
log_path="$script_dir/last-run.log"

# `-batchmode` (without `-nographics`) keeps Metal initialized so the
# RenderTexture readback path works. `-quit` is implicit when the
# entry point calls EditorApplication.Exit().
#
# `-logFile -` writes Unity's stdout/stderr to this process's stdout;
# we tee it to last-run.log for postmortems. The runner does NOT read
# stdout — it only consumes results.ndjson.
"$UNITY_BIN" \
  -batchmode \
  -projectPath "$project_path" \
  -executeMethod Conformance.Conformance.RunBatch \
  -logFile - \
  -- \
  "$manifest" \
  "$results" \
  2>&1 | tee "$log_path"

unity_exit=${PIPESTATUS[0]}
exit "$unity_exit"
