#!/usr/bin/env bash
# Determine whether VRoid Studio 2.12.0 still ships the "Export -> VRM 0.x" path.
#
# Methodology:
# 1. Open VRoid Studio (manual -- Studio is a GUI app, no CLI).
# 2. Open any character (or create a "Default Female" preset).
# 3. File -> Export -> check the format dropdown for a "VRM 0.x" or "Legacy VRM" entry.
# 4. Record AVAILABLE / REMOVED in docs/findings.md.
#
# This script just emits the procedure -- the human runs the check.
set -euo pipefail
cat <<'EOF'
VRoid Studio 0.x export availability check -- manual procedure:

1. Launch VRoid Studio (version 2.12.0 confirmed; check Settings -> About).
2. Open any saved character (or create a new "Default Female" preset).
3. Navigate to File -> Export.
4. Check the export-format dropdown:
   - "VRM" -> 1.0 path (default).
   - "VRM 0.x" or "Legacy VRM" -> 0.x path still present. RECORD: AVAILABLE.
   - No 0.x option visible -> RECORD: REMOVED.

Append the finding to docs/findings.md under a new dated entry:
"VRoid Studio 0.x export availability: <AVAILABLE | REMOVED>".

Fallbacks if REMOVED:
- Source 0.x VRoid Studio 1.x installer (older release).
- Use VRoid Hub-sourced 0.x content (license-vetted, attribution-required).
- Drop the vroid_default_F_0_0 fixture from slice 1; rely on avatarA_0_0 alone.
EOF
