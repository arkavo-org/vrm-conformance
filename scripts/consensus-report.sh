#!/usr/bin/env bash
#
# Corpus-wide consensus report: walk the bootstrap-goldens manifest and run
# `vrm-runner consensus-diff` for every test_id, aggregating per-pair SSIM
# stats and ranking the most-divergent test_ids. The output is a single
# JSON report at `goldens-cache/consensus-report.json` plus a human-
# readable summary on stdout.
#
# This is the corpus-wide flavor of the per-test-id consensus diff that
# already exists as a CLI subcommand. It produces an at-a-glance view of
# where renderers diverge most, so each entry becomes a candidate for an
# upstream issue (or a methodology refinement, if the divergence turns
# out to be a legitimate spec interpretation gap).
#
# Usage:
#   scripts/consensus-report.sh [manifest-path]
#
# If no manifest path is given, defaults to goldens-cache/local-manifest.json
# (the bootstrap-goldens.sh local-mode output). For the S3-published
# manifest, pass goldens/manifest.json explicitly.
#
# Env:
#   REPORT_OUT  Override the JSON output path. Default:
#               goldens-cache/consensus-report.json

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
cd "$ROOT"

MANIFEST="${1:-$ROOT/goldens-cache/local-manifest.json}"
REPORT_OUT="${REPORT_OUT:-$ROOT/goldens-cache/consensus-report.json}"

if [ ! -f "$MANIFEST" ]; then
    echo "consensus-report: manifest not found at $MANIFEST" >&2
    echo "                  Run scripts/bootstrap-goldens.sh first." >&2
    exit 2
fi

echo "==> Building vrm-runner (release)"
cargo build --release -q -p vrm-runner --bin vrm-runner

BIN="$ROOT/target/release/vrm-runner"
if [ ! -x "$BIN" ]; then
    echo "consensus-report: $BIN missing after build" >&2
    exit 3
fi

echo "==> Running consensus-diff across the corpus"
echo "    manifest: $MANIFEST"
echo "    output:   $REPORT_OUT"
echo

python3 - "$MANIFEST" "$ROOT" "$REPORT_OUT" "$BIN" << 'PYEOF'
import collections, json, os, statistics, subprocess, sys

manifest_path, root, report_out, runner_bin = sys.argv[1:5]
m = json.load(open(manifest_path))

# Group entries by test_id.
by_id = collections.defaultdict(list)
for entry in m["entries"]:
    by_id[entry["test_id"]].append(entry)

# Locate each test_id's test.yaml plan. swing variants live in
# goldens-cache/_assets_swing/; everything else in _assets/.
def find_plan(test_id):
    for sub in ["_assets", "_assets_swing"]:
        p = os.path.join(root, "goldens-cache", sub, f"{test_id}.test.yaml")
        if os.path.exists(p):
            return p
    return None

results = []
skipped = []

total = len(by_id)
for i, (test_id, entries) in enumerate(sorted(by_id.items()), 1):
    if len(entries) < 2:
        skipped.append({"test_id": test_id, "reason": f"only {len(entries)} renderer(s)"})
        continue

    plan_path = find_plan(test_id)
    if not plan_path:
        skipped.append({"test_id": test_id, "reason": "no .test.yaml found"})
        continue

    # Build --render <name>=<abs-path> args from manifest entries. Only
    # file:// URLs are supported here — the report runs locally against
    # the bootstrap cache. For S3-backed manifests, run pull-goldens
    # first to mirror the corpus, then point this script at the mirror's
    # manifest.
    render_args = []
    bad = None
    for e in entries:
        url = e["image_url"]
        if url.startswith("file://"):
            local_path = url[7:]
        else:
            bad = f"non-file:// URL: {url}"
            break
        if not os.path.exists(local_path):
            bad = f"missing render: {local_path}"
            break
        render_args.extend(["--render", f'{e["renderer_name"]}={local_path}'])

    if bad:
        skipped.append({"test_id": test_id, "reason": bad})
        continue

    cmd = [runner_bin, "consensus-diff", "--plan", plan_path, "--json"] + render_args
    out = subprocess.run(cmd, capture_output=True, text=True, cwd=root)

    # consensus-diff exits 0 on pass, 1 on consensus-failure (which is a
    # valid data point, not an error). Other codes are real failures.
    if out.returncode not in (0, 1):
        skipped.append({
            "test_id": test_id,
            "reason": f"runner rc={out.returncode}: {out.stderr.strip()[:160]}"
        })
        continue

    json_out = out.stdout.strip()
    if not json_out:
        skipped.append({"test_id": test_id, "reason": "empty consensus-diff stdout"})
        continue

    try:
        result = json.loads(json_out)
    except json.JSONDecodeError as e:
        skipped.append({
            "test_id": test_id,
            "reason": f"JSON parse: {e}; raw: {json_out[:160]}"
        })
        continue

    results.append(result)
    if i % 10 == 0 or i == total:
        print(f"  [{i:3}/{total}] processed")

print()
print(f"==> Processed {len(results)} test_ids; skipped {len(skipped)}")

# Aggregate
passed = sum(1 for r in results if r["consensus_passed"])
failed = len(results) - passed
print(f"    consensus_passed: {passed}/{len(results)}")
print(f"    consensus_failed: {failed}/{len(results)}")

# Per-pair stats
pair_scores = collections.defaultdict(list)
for r in results:
    rs = r["renderers"]
    mat = r["ssim_matrix"]
    for i in range(len(rs)):
        for j in range(i + 1, len(rs)):
            pair = tuple(sorted([rs[i], rs[j]]))
            pair_scores[pair].append(mat[i][j])

print()
print("    Pairwise SSIM stats across the corpus:")
print(f"      {'pair':<36s}  mean    min     max     n")
for pair, scores in sorted(pair_scores.items()):
    name = f"{pair[0]} vs {pair[1]}"
    print(
        f"      {name:<36s}  {statistics.mean(scores):.4f}  "
        f"{min(scores):.4f}  {max(scores):.4f}  {len(scores)}"
    )

# Top-N most divergent
def min_pair_ssim(r):
    n = len(r["renderers"])
    return min(r["ssim_matrix"][i][j] for i in range(n) for j in range(i + 1, n))

sorted_by_div = sorted(results, key=min_pair_ssim)
print()
print("    Top 15 most-divergent test_ids (lowest min pairwise SSIM):")
for r in sorted_by_div[:15]:
    s = min_pair_ssim(r)
    print(f"      {r['test_id']:<40s}  {s:.4f}  outliers={r['outliers']}")

# Write the full report
report = {
    "manifest": manifest_path,
    "test_id_count": len(results),
    "consensus_passed": passed,
    "consensus_failed": failed,
    "skipped": skipped,
    "pair_stats": {
        f"{p[0]} vs {p[1]}": {
            "mean": statistics.mean(s),
            "min": min(s),
            "max": max(s),
            "n": len(s),
        }
        for p, s in pair_scores.items()
    },
    "most_divergent": [
        {
            "test_id": r["test_id"],
            "min_pair_ssim": min_pair_ssim(r),
            "renderers": r["renderers"],
            "ssim_matrix": r["ssim_matrix"],
            "outliers": r["outliers"],
        }
        for r in sorted_by_div[:20]
    ],
    "per_test_id": results,
}
os.makedirs(os.path.dirname(report_out), exist_ok=True)
with open(report_out, "w") as f:
    json.dump(report, f, indent=2)
print()
print(f"==> Full report: {report_out}")
PYEOF
