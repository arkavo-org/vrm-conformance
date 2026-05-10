# Phase 2E — pull-goldens Real Implementation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the existing `scripts/pull-goldens.sh` stub (which exits 1 with a TODO message) with a real implementation that reads `goldens/manifest.json`, downloads each entry's PNG from S3 to a local mirror, and verifies BLAKE3 content addressing. After this plan, offline reviewers can fetch the latest golden corpus in one command and feed it into `vrm-runner diff` for local comparison without S3 credentials beyond read access.

**Architecture:** New Rust binary `pull-goldens` in `crates/vrm-s3/`, mirroring the existing `push-goldens` binary. Uses the already-shipped `vrm_s3::push_pull::pull_png` async function. Walks manifest entries; for each, computes the local destination path under `--output-dir`, downloads via S3 GetObject, hashes the downloaded bytes with BLAKE3, and verifies against `entry.image_blake3`. Mismatch → exit non-zero with a clear error pointing at the bad entry. The shell wrapper `scripts/pull-goldens.sh` becomes a thin convenience over the binary.

**Tech Stack:** Existing workspace (`vrm-s3` crate already has `aws-sdk-s3`, `blake3`, `clap`, `tokio`, `serde_json`). No new dependencies.

**Why scope-bound:**
- The expensive parts (S3 client construction, byte streaming, BLAKE3 hashing) are already implemented in `push_pull.rs`. This plan is mostly orchestration.
- We trust that `pull_png` works correctly — its sibling `push_png` is exercised by the Phase 1 J3 binary and the smoke script. No new integration testing of the AWS SDK plumbing.
- Local-mirror layout matches the runner's `--asset-dir` / `--reference` shapes: `<output-dir>/<test_id>/<renderer>.png`. Reviewers point `vrm-runner diff --reference` at one of these paths.

**YAGNI scope guards:**
- ✅ No parallel/concurrent downloads in v0.1. Serial pulls per entry. If pulling 50 golden PNGs ever takes too long for an interactive workflow, revisit; today each PNG is 100-500KB, serial is fine.
- ✅ No incremental sync. Every run downloads every manifest entry. Caching by BLAKE3 (skip download if local file's hash matches manifest) is a defensible Phase 2F task.
- ✅ No filtering options (e.g., `--renderer three-vrm` to pull only one column). Add when there are multiple renderers and the corpus grows.
- ✅ No `--dry-run` flag. Defer until someone asks.
- ✅ No retry-on-transient-network-error logic. The AWS SDK has its own retry policy.

---

## File Layout

| File | Status | Responsibility |
|---|---|---|
| `crates/vrm-s3/src/bin/pull-goldens.rs` | Create | tokio-async clap binary that reads manifest + pulls each entry + verifies BLAKE3. |
| `crates/vrm-s3/src/push_pull.rs` | Modify | Add `verify_blake3(bytes, expected: &str) -> Result<()>` helper; refactor existing `now_rfc3339()` to keep tests passing. |
| `crates/vrm-s3/tests/blake3_verify.rs` | Create | Unit tests on the BLAKE3 verification helper (the only logic in 2E that's testable without S3). |
| `scripts/pull-goldens.sh` | Modify | Replace the TODO stub with a wrapper around `cargo run --release --bin pull-goldens -p vrm-s3`. |
| `crates/vrm-s3/Cargo.toml` | (No change) | Existing `[[bin]] push-goldens` declaration will auto-discover the new bin/ source. Cargo handles `src/bin/*.rs` without explicit `[[bin]]` stanzas. |

---

## Section A — BLAKE3 verification helper

### Task A1: `verify_blake3` in push_pull.rs (TDD)

**Files:**
- Modify: `crates/vrm-s3/src/push_pull.rs`
- Create: `crates/vrm-s3/tests/blake3_verify.rs`

A small public function that takes raw bytes + a `"blake3:<hex>"` ref string and confirms the bytes hash to that ref. Exists so the pull binary doesn't reinvent the parsing; lives in `push_pull.rs` because it's the natural sibling of `push_png` (which produces the ref) and `pull_png` (which consumes it).

- [ ] **Step 1: Failing tests**

`crates/vrm-s3/tests/blake3_verify.rs`:

```rust
use vrm_s3::push_pull::verify_blake3;

#[test]
fn matching_hash_returns_ok() {
    let bytes = b"hello world";
    let hash = blake3::hash(bytes);
    let expected = format!("blake3:{}", hash.to_hex());
    verify_blake3(bytes, &expected).expect("matching hash should verify");
}

#[test]
fn mismatching_hash_errors() {
    let bytes = b"hello world";
    let wrong = "blake3:0000000000000000000000000000000000000000000000000000000000000000";
    let err = verify_blake3(bytes, wrong).expect_err("mismatched hash must error");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("hash mismatch") || msg.contains("blake3"),
        "expected mismatch/blake3 in error, got: {msg}"
    );
}

#[test]
fn malformed_prefix_errors() {
    let bytes = b"x";
    let err = verify_blake3(bytes, "sha256:abc").expect_err("non-blake3 prefix must error");
    assert!(err.to_string().contains("blake3:"), "got: {err}");
}

#[test]
fn missing_prefix_errors() {
    let bytes = b"x";
    let err = verify_blake3(bytes, "abc").expect_err("missing prefix must error");
    assert!(err.to_string().contains("blake3:"), "got: {err}");
}
```

- [ ] **Step 2: Run failing tests**

```bash
cargo test -p vrm-s3 --test blake3_verify
```

Expected: compile error — `verify_blake3` doesn't exist.

- [ ] **Step 3: Implement**

Append to `crates/vrm-s3/src/push_pull.rs`:

```rust
/// Verify that `bytes` hash to the BLAKE3 ref `expected` (shape:
/// `"blake3:<64-hex-chars>"`). Returns Err with a descriptive message on
/// mismatch or malformed input.
pub fn verify_blake3(bytes: &[u8], expected: &str) -> Result<()> {
    let hex = expected
        .strip_prefix("blake3:")
        .ok_or_else(|| anyhow::anyhow!("expected blake3: prefix, got: {expected}"))?;
    let actual = blake3::hash(bytes);
    let actual_hex = actual.to_hex();
    if actual_hex.as_str() == hex {
        Ok(())
    } else {
        anyhow::bail!(
            "blake3 hash mismatch: expected {hex}, got {actual_hex}"
        );
    }
}
```

- [ ] **Step 4: Tests pass**

```bash
cargo test -p vrm-s3 --test blake3_verify
```

Expected: 4 tests pass.

- [ ] **Step 5: Workspace clean**

```bash
cargo test -p vrm-s3
cargo clippy -p vrm-s3 --all-targets -- -D warnings
cargo fmt --all -- --check
```

All green.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-s3/src/push_pull.rs crates/vrm-s3/tests/blake3_verify.rs
git commit -m "feat(vrm-s3): verify_blake3 helper for content-address verification"
```

---

## Section B — pull-goldens binary

### Task B1: `pull-goldens` Rust binary

**Files:**
- Create: `crates/vrm-s3/src/bin/pull-goldens.rs`

clap-derived async binary. Reads manifest path (default `goldens/manifest.json`), output-dir, optional `--renderer` filter (skipped scope guard above — keep flag but document it as future-use). Walks entries; for each, downloads to `<output-dir>/<test_id>/<renderer_name>.png` and verifies BLAKE3. Emits NDJSON progress on stderr; final JSON summary on stdout.

- [ ] **Step 1: Implement**

`crates/vrm-s3/src/bin/pull-goldens.rs`:

```rust
//! Pull every entry in goldens/manifest.json from S3 to a local mirror,
//! verifying BLAKE3 against the manifest's claim for each file. Exits
//! non-zero on the first hash mismatch (do not silently overwrite a
//! local file with bytes that don't match the manifest's content ref).

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use clap::Parser;
use serde_json::json;
use vrm_s3::{
    manifest::{Manifest, ManifestEntry},
    push_pull::{pull_png, verify_blake3},
};

#[derive(Debug, Parser)]
#[command(version, about = "Pull goldens listed in manifest.json from S3 to a local mirror")]
struct Args {
    /// Path to the manifest file. Defaults to goldens/manifest.json in cwd.
    #[arg(long, default_value = "goldens/manifest.json")]
    manifest: Utf8PathBuf,

    /// Local mirror directory. PNGs land at <output-dir>/<test_id>/<renderer>.png.
    #[arg(long)]
    output_dir: Utf8PathBuf,

    /// NDJSON progress on stderr; final JSON summary on stdout. If unset,
    /// human-readable text only.
    #[arg(long)]
    json: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let manifest_bytes = std::fs::read(args.manifest.as_std_path())
        .with_context(|| format!("read manifest: {}", args.manifest))?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("parse manifest: {}", args.manifest))?;

    std::fs::create_dir_all(args.output_dir.as_std_path())?;

    let total = manifest.entries.len();
    let mut pulled: Vec<Utf8PathBuf> = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();

    for (i, entry) in manifest.entries.iter().enumerate() {
        let dest = compute_dest(&args.output_dir, entry);
        if args.json {
            let evt = json!({
                "event": "progress",
                "op": "pull-goldens",
                "index": i,
                "total": total,
                "test_id": entry.test_id,
                "renderer_name": entry.renderer_name,
                "image_url": entry.image_url,
                "dest": dest,
            });
            eprintln!("{}", serde_json::to_string(&evt)?);
        } else {
            eprintln!(
                "[{:3}/{}] {} ({}) → {}",
                i + 1,
                total,
                entry.test_id,
                entry.renderer_name,
                dest
            );
        }

        match pull_one(&entry.image_url, &dest, &entry.image_blake3).await {
            Ok(()) => pulled.push(dest),
            Err(e) => {
                failures.push((
                    format!("{}/{}", entry.test_id, entry.renderer_name),
                    e.to_string(),
                ));
            }
        }
    }

    if args.json {
        let summary = json!({
            "ok": failures.is_empty(),
            "pulled": pulled,
            "failures": failures.iter().map(|(k, v)| json!({"entry": k, "error": v})).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string(&summary)?);
    } else {
        println!(
            "pulled {} entries to {}; {} failures",
            pulled.len(),
            args.output_dir,
            failures.len()
        );
    }

    if !failures.is_empty() {
        for (entry, err) in &failures {
            eprintln!("FAIL {entry}: {err}");
        }
        anyhow::bail!("{} of {} pulls failed", failures.len(), total);
    }

    Ok(())
}

async fn pull_one(image_url: &str, dest: &Utf8PathBuf, expected_blake3: &str) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent.as_std_path())?;
    }
    pull_png(image_url, dest).await?;
    let bytes = std::fs::read(dest.as_std_path())
        .with_context(|| format!("read downloaded file: {dest}"))?;
    verify_blake3(&bytes, expected_blake3)
        .with_context(|| format!("verify {dest} against manifest"))?;
    Ok(())
}

fn compute_dest(output_dir: &Utf8PathBuf, entry: &ManifestEntry) -> Utf8PathBuf {
    output_dir
        .join(&entry.test_id)
        .join(format!("{}.png", entry.renderer_name))
}
```

- [ ] **Step 2: Build**

```bash
cargo build --release --bin pull-goldens -p vrm-s3
```

Expected: clean build.

- [ ] **Step 3: Smoke-test the binary against an empty manifest**

```bash
mkdir -p /tmp/pull-empty
cat > /tmp/pull-empty/manifest.json <<'EOF'
{"version": 1, "entries": []}
EOF
mkdir -p /tmp/pull-mirror
./target/release/pull-goldens --manifest /tmp/pull-empty/manifest.json --output-dir /tmp/pull-mirror
echo "exit: $?"
```

Expected: `pulled 0 entries to /tmp/pull-mirror; 0 failures`, exit 0.

JSON mode:

```bash
./target/release/pull-goldens --manifest /tmp/pull-empty/manifest.json --output-dir /tmp/pull-mirror --json
```

Expected: `{"failures":[],"ok":true,"pulled":[]}`.

> **Caveat for the implementing engineer:** there is no local fixture with a real S3 URL we can fetch. A non-empty manifest test requires real S3 credentials and a real bucket. We skip that here; the empty-manifest path exercises everything except `pull_one` itself, and `pull_png` + `verify_blake3` are unit-tested independently (`verify_blake3` in Task A1, `pull_png` exists from Phase 1 J2 — its AWS SDK code is treated as trusted).

- [ ] **Step 4: Workspace clean**

```bash
cargo clippy -p vrm-s3 --all-targets -- -D warnings
cargo fmt --all -- --check
```

Both clean.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-s3/src/bin/pull-goldens.rs
git commit -m "feat(vrm-s3): pull-goldens binary downloads + verifies manifest entries"
```

---

## Section C — Shell wrapper + docs

### Task C1: Replace `scripts/pull-goldens.sh` stub

**Files:**
- Modify: `scripts/pull-goldens.sh`

The current file is a 3-line stub that echoes a TODO. Replace with a real wrapper.

- [ ] **Step 1: Replace the file**

`scripts/pull-goldens.sh`:

```bash
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
```

- [ ] **Step 2: Smoke-test with empty manifest**

```bash
chmod +x scripts/pull-goldens.sh
mkdir -p /tmp/wrap-test
cat > /tmp/wrap-test/manifest.json <<'EOF'
{"version": 1, "entries": []}
EOF
mkdir -p /tmp/wrap-out
./scripts/pull-goldens.sh /tmp/wrap-out /tmp/wrap-test/manifest.json
```

Expected: cargo builds (cache hits, so fast), the binary runs, prints `pulled 0 entries`, exits 0.

Missing-manifest case:

```bash
./scripts/pull-goldens.sh /tmp/wrap-out /tmp/wrap-test/nonexistent.json
echo "exit: $?"
```

Expected: stderr message, exit 2.

- [ ] **Step 3: Commit**

```bash
git add scripts/pull-goldens.sh
git commit -m "feat(scripts): real pull-goldens shell wrapper around the cargo binary"
```

---

### Task C2: Document the new command

**Files:**
- Modify: `CONTRIBUTING.md`
- Modify: `README.md` (link to the new flow)

- [ ] **Step 1: Update CONTRIBUTING.md**

Find the existing "Submitting renderer renders" section in `CONTRIBUTING.md`. After it, add a new section:

```markdown
## Pulling goldens for offline diff

If you want to run the diff engine locally against the published golden corpus without re-rendering, pull every PNG to a local mirror:

\`\`\`bash
./scripts/pull-goldens.sh /tmp/goldens-mirror
\`\`\`

This reads `goldens/manifest.json`, downloads each entry from S3 to `/tmp/goldens-mirror/<test_id>/<renderer_name>.png`, and verifies BLAKE3 content addressing. A hash mismatch exits non-zero with a clear pointer at the bad entry.

Then drive the runner's diff against a local render:

\`\`\`bash
cargo run -p vrm-runner -- diff \
  --plan path/to/plan.yaml \
  --render path/to/your-render.png \
  --reference /tmp/goldens-mirror/<test_id>/<renderer_name>.png \
  --json
\`\`\`

Requires AWS credentials with `s3:GetObject` on the bucket(s) referenced by the manifest. Reviewers without write access can request a read-only IAM role.
```

- [ ] **Step 2: Update README.md**

In `README.md`, find the row for `goldens/manifest.json` in the repo layout table. The row currently reads:

> | `goldens/manifest.json` | In-repo manifest pointing to S3-hosted golden images; bootstrapped empty in v0.1, populated as renderer maintainers submit. |

Append to the end of the cell text: ` Use \`scripts/pull-goldens.sh\` to mirror locally.`

The full new cell:

> | `goldens/manifest.json` | In-repo manifest pointing to S3-hosted golden images; bootstrapped empty in v0.1, populated as renderer maintainers submit. Use `scripts/pull-goldens.sh` to mirror locally. |

- [ ] **Step 3: Commit**

```bash
git add CONTRIBUTING.md README.md
git commit -m "docs: pull-goldens flow for offline diff against published corpus"
```

---

## Self-Review

**Spec coverage:**

| 2E goal | Task |
|---|---|
| BLAKE3 verification helper | A1 |
| pull-goldens binary | B1 |
| Shell wrapper | C1 |
| Documentation | C2 |

**Placeholder scan:** none. All code blocks complete; tests assert behavior. The "no real S3 fixture" caveat in B1 Step 3 is honest and references the unit-tested helpers that cover the testable logic.

**Type consistency:**

- `verify_blake3(bytes: &[u8], expected: &str) -> Result<()>` consistent in A1 + B1.
- `pull_png(url: &str, dest: &Utf8Path) -> Result<()>` from Phase 1 J2 — consumed in B1's `pull_one` via `pull_png(image_url, dest).await?`. The function takes `&Utf8Path`; we pass `&Utf8PathBuf` which auto-derefs. Verified by `cargo build` in Step 2.
- `Manifest` and `ManifestEntry` from `vrm-s3::manifest` — already shipped in Phase 1 J1. B1 imports them by their full paths.

**YAGNI guards:**

- ✅ No concurrent downloads.
- ✅ No incremental sync (every run re-downloads).
- ✅ No `--renderer` filter.
- ✅ No `--dry-run`.
- ✅ No retry-on-error (AWS SDK has its own).
- ✅ pull_png is reused, not rewritten.

**Risk register:**

- **Real-S3 integration coverage.** We test `verify_blake3` and the empty-manifest path. The "pull from real S3" path is exercised only by the operator running the wrapper against a live manifest. If the AWS SDK ever changes behavior in a breaking way, we'll notice — but we're not adding integration tests that require a real bucket. Same trade-off as `push-goldens` (Phase 1 J3).
- **Permissions documentation.** CONTRIBUTING.md mentions `s3:GetObject`; we don't enforce or validate this in code. If a reviewer hits an `AccessDenied` error, the AWS SDK's error message points at the right fix.
- **Manifest URL parsing.** `pull_png` from Phase 1 J2 already handles `s3://bucket/key` parsing — we don't need to reimplement.

---

## Execution Handoff

Plan saved to `docs/superpowers/plans/2026-05-10-phase2e-pull-goldens.md`. Two execution options:

1. **Subagent-Driven** — fresh subagent per task. 4 tasks; A1 first, then B1/C1 can be done in either order, C2 last.
2. **Inline Execution (recommended)** — small contained task. ~10-15 minutes inline.
