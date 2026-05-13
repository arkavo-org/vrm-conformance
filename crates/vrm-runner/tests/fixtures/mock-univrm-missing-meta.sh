#!/usr/bin/env bash
# Mock UniVRM adapter — emits a results file whose first line is NOT a
# _meta envelope. Parser must reject.

set -euo pipefail

results="$2"

cat > "$results" <<'EOF'
{"test_id":"a","status":"ok","output_path":"/tmp/a.png"}
EOF
