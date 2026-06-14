#!/usr/bin/env bash
#
# Aggregate per-test PerfReport JSON files (written by
# `vrm-runner benchmark-execute`) into a single corpus-wide report at
# goldens-cache/perf-report.json, plus a VMK-vs-golden-ref structural delta
# summary on stdout. Observational — no pass/fail.
#
# Usage:
#   scripts/perf-report.sh <perf-json-dir> [reference-renderer]
#
#   <perf-json-dir>      Directory containing <id>_<renderer>.perf.json files.
#   [reference-renderer] Renderer name used as the "familiar" baseline for the
#                        structural delta. Default: univrm (golden reference).
#
# Env:
#   REPORT_OUT  Override output path. Default: goldens-cache/perf-report.json
#   SUBJECT     Renderer to compare against the reference. Default: vrm-metal-kit

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
cd "$ROOT"

PERF_DIR="${1:-}"
REFERENCE="${2:-univrm}"
SUBJECT="${SUBJECT:-vrm-metal-kit}"
REPORT_OUT="${REPORT_OUT:-$ROOT/goldens-cache/perf-report.json}"

if [ -z "$PERF_DIR" ] || [ ! -d "$PERF_DIR" ]; then
    echo "perf-report: pass a directory of *.perf.json files" >&2
    echo "             usage: scripts/perf-report.sh <perf-json-dir> [reference-renderer]" >&2
    exit 2
fi

command -v jq >/dev/null 2>&1 || { echo "perf-report: jq is required" >&2; exit 2; }

mkdir -p "$(dirname "$REPORT_OUT")"

# Collect every report into a JSON array under `reports`.
# Portable file-collection (bash 3.2 lacks mapfile): use a while-read loop.
FILES=()
while IFS= read -r f; do
    FILES+=("$f")
done < <(find "$PERF_DIR" -name '*.perf.json' | sort)

if [ "${#FILES[@]}" -eq 0 ]; then
    echo "perf-report: no *.perf.json files under $PERF_DIR" >&2
    exit 1
fi

jq -s '{ reports: . }' "${FILES[@]}" > "$REPORT_OUT.tmp"

# Compute the structural delta: for each test_id present for BOTH subject and
# reference, percentage difference of draw_calls (subject vs reference).
jq --arg subject "$SUBJECT" --arg reference "$REFERENCE" '
  .reports as $r
  | [ $r[] | select(.renderer_name == $subject) ] as $subj
  | [ $r[] | select(.renderer_name == $reference) ] as $ref
  | .structural_delta = [
      $subj[] as $s
      | ($ref[] | select(.test_id == $s.test_id)) as $b
      | select($s.structural != null and $b.structural != null
               and $b.structural.draw_calls != 0)
      | {
          test_id: $s.test_id,
          subject: $subject,
          reference: $reference,
          subject_draw_calls: $s.structural.draw_calls,
          reference_draw_calls: $b.structural.draw_calls,
          draw_calls_pct: ((($s.structural.draw_calls - $b.structural.draw_calls)
                            / $b.structural.draw_calls) * 100)
        }
    ]
' "$REPORT_OUT.tmp" > "$REPORT_OUT"
rm -f "$REPORT_OUT.tmp"

echo "==> perf-report written: $REPORT_OUT"
echo "    reports: ${#FILES[@]}"
echo "    structural delta ($SUBJECT vs $REFERENCE, draw_calls %):"
jq -r '.structural_delta[] | "      \(.test_id): \(.draw_calls_pct | . * 100 | round / 100)%"' "$REPORT_OUT" \
    || echo "      (no overlapping test_ids between $SUBJECT and $REFERENCE)"
