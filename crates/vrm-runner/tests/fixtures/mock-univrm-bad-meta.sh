#!/usr/bin/env bash
# Mock UniVRM adapter — emits a malformed _meta envelope (missing
# total_tests). Parser must reject the batch with a clear error.

set -euo pipefail

results="$2"

cat > "$results" <<'EOF'
{"_meta":true,"manifest_version":1,"renderer_name":"univrm","renderer_version":"mock-v0.131.2","unity_version":"mock-2022.3.50f1","render_pipeline":"Built-in RP"}
EOF
