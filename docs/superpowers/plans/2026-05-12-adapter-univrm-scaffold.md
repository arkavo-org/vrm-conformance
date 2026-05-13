# UniVRM Adapter Scaffold (L1+L2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the L1+L2 scaffold of the UniVRM (Unity) adapter — a working `vrm-runner execute-test-batch` subcommand backed by mock-binary contract tests, a Unity project skeleton that compiles cleanly with UniVRM v0.131+ on the package path, and a `.github/workflows/univrm.yml` CI workflow that build-validates the project. L3 (Phase 1 ops real) and L4 (Phase 2 spring-bone real) are follow-up plans authored once an engineer with Unity installed has verified the UniVRM v0.131 C# API surface against the installed package.

**Architecture:** Batched one-shot adapter per [`docs/superpowers/specs/2026-05-12-adapter-univrm-design.md`](../specs/2026-05-12-adapter-univrm-design.md) and the engine-idiom-divergence principle in [`rfcs/0003`](../../../rfcs/0003-engine-idiom-divergence.md). The Rust runner builds a JSON manifest, invokes `adapters/univrm/launcher.sh` with the manifest path, the launcher invokes Unity Editor in `-batchmode -executeMethod Conformance.RunBatch`, the C# entry point writes `results.ndjson` with one line per test (`_meta` envelope first), and the runner ingests results. No persistent IPC, no Rust shim crate (Unity-side handles everything Unity; Rust-side handles everything Rust; filesystem-as-protocol between them).

**Tech Stack:** Rust 2021 (MSRV 1.88) for the runner subcommand and contract tests. Bash for `launcher.sh` and the mock fixtures. Unity 2022.3 LTS + Built-in RP + UniVRM v0.131+ (UPM git URL) for the project skeleton; no C# rendering code in L1+L2 — `Conformance.RunBatch` is a stub that returns all tests as `Unimplemented` with `data.phase: "L3"`. CI runs on `macos-latest` via `game-ci/unity-actions` for build-validate.

**YAGNI scope guards for this plan:**
- **No real Unity rendering** — `Conformance.RunBatch` stubs every test as `Unimplemented`. Real rendering is L3.
- **No UniVRM API calls** — the Unity project depends on UniVRM v0.131+ via UPM (must resolve cleanly for the build to compile), but no C# code in this plan invokes UniVRM types. L3 starts touching `Vrm10.LoadPathAsync` and friends.
- **No spring-bone physics** — L4.
- **No launcher binary resolution heuristics beyond `UNITY_BIN` env + default path** — if Unity isn't found, the launcher exits non-zero. The runner reports that as a batch-level failure.
- **Mock contract tests only** — the Rust integration test suite uses shell-script fixtures that emit known `results.ndjson` output without touching Unity. The local `scripts/smoke-univrm.sh` integration test that exercises real Unity arrives with L3.

**Scope deliverables at end of plan:**
1. `cargo run -p vrm-runner -- execute-test-batch --plans corpus/ --adapter-bin <mock> --output-dir <out>` produces a local manifest from a known-good mock fixture.
2. `cargo test -p vrm-runner --test execute_test_batch` runs four contract tests (happy path, partial output, malformed `_meta`, missing-manifest) without Unity installed.
3. `Unity -batchmode -projectPath adapters/univrm/UniVRMConformance -executeMethod Conformance.RunBatch -- manifest.json results.ndjson -quit` (on a developer machine with Unity installed) writes a `_meta` line + N `Unimplemented` lines without crashing.
4. `.github/workflows/univrm.yml` opens the Unity project in batchmode, runs EditMode tests, asserts zero compile errors, exits clean.
5. `adapters/univrm/README.md` documents the build + run sequence in the same shape as `adapters/godot-vrm/README.md`.

---

## File Structure

**New files:**
- `crates/vrm-runner/src/execute_batch.rs` — batched-mode execution logic (manifest builder, subprocess invocation, NDJSON ingestion).
- `crates/vrm-runner/tests/execute_test_batch.rs` — Rust contract tests against mock fixtures.
- `crates/vrm-runner/tests/fixtures/mock-univrm-ok.sh` — happy-path mock.
- `crates/vrm-runner/tests/fixtures/mock-univrm-partial.sh` — exits non-zero after partial output.
- `crates/vrm-runner/tests/fixtures/mock-univrm-bad-meta.sh` — emits malformed `_meta` line.
- `crates/vrm-runner/tests/fixtures/mock-univrm-missing-meta.sh` — emits no `_meta` line at all.
- `adapters/univrm/launcher.sh` — Bash wrapper that resolves Unity binary and invokes batchmode.
- `adapters/univrm/README.md` — adapter usage + build sequence.
- `adapters/univrm/.gitignore` — ignore Unity-generated `Library/`, `Temp/`, `Logs/`.
- `adapters/univrm/UniVRMConformance/ProjectSettings/ProjectVersion.txt` — Unity version pin.
- `adapters/univrm/UniVRMConformance/Packages/manifest.json` — UPM package pin (UniVRM v0.131+).
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs` — `RunBatch` stub returning all `Unimplemented`.
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.asmdef` — assembly definition.
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/RunBatchStubTest.cs` — EditMode test that exercises the stub.
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/Conformance.Tests.EditMode.asmdef` — test assembly definition.
- `.github/workflows/univrm.yml` — CI workflow (build-validate + EditMode tests on macos-latest).

**Modified files:**
- `crates/vrm-runner/src/cli.rs` — add `ExecuteTestBatch` subcommand variant + dispatch.
- `crates/vrm-runner/src/lib.rs` — add `pub mod execute_batch;`.
- `crates/vrm-runner/Cargo.toml` — add `blake3` dev-dep + workspace dep (used by the new module for PNG content addressing).
- `Cargo.toml` (workspace root) — confirm no member additions needed (the new code lives inside the existing `vrm-runner` crate).

Each file has one responsibility; everything Rust lives in `crates/vrm-runner/`, everything Unity lives under `adapters/univrm/UniVRMConformance/`, everything shell-glue lives in `adapters/univrm/` or `crates/vrm-runner/tests/fixtures/`.

---

## Task 1: Add `ExecuteTestBatch` CLI variant returning `NotImplemented`

**Files:**
- Modify: `crates/vrm-runner/src/cli.rs:14` (extend the `Cmd` enum) and `crates/vrm-runner/src/cli.rs:100` (extend the `run` match)

- [ ] **Step 1: Write the failing test**

Add to a new file `crates/vrm-runner/tests/execute_test_batch.rs`:

```rust
//! Contract tests for `vrm-runner execute-test-batch`. Tests use mock
//! shell-script fixtures so they run without Unity installed.

use std::path::PathBuf;
use std::process::Command;

fn runner_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_vrm-runner"))
}

#[test]
fn execute_test_batch_subcommand_is_registered() {
    // The subcommand must parse — clap should accept the flag set even
    // if the implementation is a stub. Failing here means the CLI
    // surface doesn't exist yet.
    let out = Command::new(runner_bin())
        .args(["execute-test-batch", "--help"])
        .output()
        .expect("spawn runner");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "execute-test-batch --help should succeed; stdout={stdout} stderr={stderr}"
    );
    assert!(
        stdout.contains("--plans"),
        "help must mention --plans flag; got: {stdout}"
    );
    assert!(
        stdout.contains("--adapter-bin"),
        "help must mention --adapter-bin flag; got: {stdout}"
    );
    assert!(
        stdout.contains("--output-dir"),
        "help must mention --output-dir flag; got: {stdout}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vrm-runner --test execute_test_batch execute_test_batch_subcommand_is_registered
```

Expected: FAIL — `execute-test-batch` is not a recognized subcommand, clap exits non-zero.

- [ ] **Step 3: Add the CLI variant**

Edit `crates/vrm-runner/src/cli.rs`. Locate the `Cmd` enum (starts at line 14) and append a new variant after the existing `Describe` variant:

```rust
    /// Execute a batched corpus through a batch-mode adapter (UniVRM).
    /// Mirrors `ExecuteTestPlan` shape but takes a directory of plans
    /// and invokes the adapter once for the whole batch. See
    /// `docs/superpowers/specs/2026-05-12-adapter-univrm-design.md`.
    ExecuteTestBatch {
        /// Directory containing `*.test.yaml` test plans. Each plan is
        /// paired with its sibling `.vrm` (same stem).
        #[arg(long)]
        plans: Utf8PathBuf,
        /// Path to the adapter launcher (e.g.
        /// `adapters/univrm/launcher.sh` for real Unity, or a mock
        /// fixture for tests).
        #[arg(long)]
        adapter_bin: Utf8PathBuf,
        /// Directory where rendered PNGs and the per-renderer local
        /// manifest are written.
        #[arg(long)]
        output_dir: Utf8PathBuf,
        /// Renderer name recorded in the local manifest.
        #[arg(long, default_value = "univrm")]
        renderer_name: String,
        /// Emit JSON summary to stdout.
        #[arg(long)]
        json: bool,
    },
```

Then locate the `run` function's `match cli.command` (line 101) and add a dispatch arm before the closing brace of the match:

```rust
        Cmd::ExecuteTestBatch {
            plans: _,
            adapter_bin: _,
            output_dir: _,
            renderer_name: _,
            json: _,
        } => {
            // Stub: real implementation lands in Task 8 (execute_batch::run).
            anyhow::bail!("execute-test-batch: not yet implemented (stub)");
        }
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p vrm-runner --test execute_test_batch execute_test_batch_subcommand_is_registered
```

Expected: PASS — the help text lists the three flags.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-runner/src/cli.rs crates/vrm-runner/tests/execute_test_batch.rs
git commit -m "feat(vrm-runner): add execute-test-batch CLI subcommand stub"
```

---

## Task 2: Create `execute_batch` module with the manifest builder

**Files:**
- Create: `crates/vrm-runner/src/execute_batch.rs`
- Modify: `crates/vrm-runner/src/lib.rs:3` (add module declaration)

- [ ] **Step 1: Write the failing test**

Append to `crates/vrm-runner/tests/execute_test_batch.rs`:

```rust
use vrm_runner::execute_batch::{build_manifest, BatchManifest};
use vrm_test_plan::{
    AmbientLight, Camera, Diff, DiffMode, DirectionalLight, Lighting, Output, PostProcessing,
    TestPlan, ToneMapping, ColorSpace,
};
use camino::Utf8PathBuf;

fn synthetic_plan(id: &str) -> TestPlan {
    TestPlan {
        id: id.into(),
        spec_section: "VRMC_materials_mtoon".into(),
        asset: format!("{id}.vrm"),
        camera: Camera {
            position: [0.0, 1.4, 1.5],
            target: [0.0, 1.4, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_degrees: 30.0,
        },
        lighting: Lighting {
            directional: DirectionalLight {
                dir: [-0.3, -0.6, -0.7],
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
            },
            ambient: AmbientLight {
                color: [0.5, 0.5, 0.5],
                intensity: 0.3,
            },
            cast_shadows: false,
            receive_shadows: false,
        },
        post_processing: PostProcessing {
            tone_mapping: ToneMapping::None,
            exposure: 1.0,
        },
        output: Output {
            width: 1024,
            height: 1024,
            color_space: ColorSpace::Srgb,
            msaa: 4,
        },
        diff: Diff {
            mode: DiffMode::Ssim,
            threshold: 0.985,
            reference_renderer: "vrm-metal-kit".into(),
        },
        ignore_renderers: Vec::new(),
        properties: Vec::new(),
        physics: None,
        animation: None,
    }
}

#[test]
fn manifest_carries_two_entries_with_absolute_paths() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let vrm_a = tmp.path().join("a.vrm");
    let vrm_b = tmp.path().join("b.vrm");
    std::fs::write(&vrm_a, b"fake vrm").unwrap();
    std::fs::write(&vrm_b, b"fake vrm").unwrap();
    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let manifest = build_manifest(
        &[
            (synthetic_plan("a"), Utf8PathBuf::from_path_buf(vrm_a.clone()).unwrap()),
            (synthetic_plan("b"), Utf8PathBuf::from_path_buf(vrm_b.clone()).unwrap()),
        ],
        Utf8PathBuf::from_path_buf(output_dir.clone()).unwrap(),
        "univrm".into(),
    );

    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.renderer_name, "univrm");
    assert_eq!(manifest.tests.len(), 2);
    assert_eq!(manifest.tests[0].test_id, "a");
    assert!(
        manifest.tests[0].vrm_path.as_str().starts_with('/'),
        "vrm_path should be absolute, got: {}",
        manifest.tests[0].vrm_path
    );
    assert!(
        manifest.output_dir.as_str().starts_with('/'),
        "output_dir should be absolute, got: {}",
        manifest.output_dir
    );
}

#[test]
fn manifest_serializes_to_expected_json_shape() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let vrm = tmp.path().join("x.vrm");
    std::fs::write(&vrm, b"fake").unwrap();
    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let manifest = build_manifest(
        &[(synthetic_plan("x"), Utf8PathBuf::from_path_buf(vrm).unwrap())],
        Utf8PathBuf::from_path_buf(output_dir).unwrap(),
        "univrm".into(),
    );

    let json = serde_json::to_value(&manifest).expect("serialize");
    assert_eq!(json["manifest_version"], 1);
    assert_eq!(json["renderer_name"], "univrm");
    assert_eq!(json["tests"][0]["test_id"], "x");
    assert_eq!(json["tests"][0]["camera"]["position"][2], 1.5);
    assert_eq!(json["tests"][0]["output"]["color_space"], "srgb");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vrm-runner --test execute_test_batch manifest
```

Expected: FAIL — `vrm_runner::execute_batch` does not exist.

- [ ] **Step 3: Create the module**

Create `crates/vrm-runner/src/execute_batch.rs`:

```rust
//! Batched-mode execution: builds a JSON manifest of test_ids, invokes
//! the adapter once for the whole batch, ingests the NDJSON results file
//! the adapter writes. See
//! `docs/superpowers/specs/2026-05-12-adapter-univrm-design.md` for the
//! design rationale (engine-idiom divergence; Unity batch mode is
//! idiomatic for "run, do work, exit").

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use vrm_test_plan::{
    AmbientLight, AnimationConfig, Camera, DirectionalLight, Lighting, Output, PhysicsConfig,
    PostProcessing, TestPlan,
};

/// Top-level JSON document the Rust runner writes for the adapter to
/// consume. Schema version is pinned at the top so future changes can
/// be detected by Unity-side code.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BatchManifest {
    pub manifest_version: u32,
    pub output_dir: Utf8PathBuf,
    pub renderer_name: String,
    pub renderer_version: Option<String>,
    pub tests: Vec<BatchTestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BatchTestEntry {
    pub test_id: String,
    pub vrm_path: Utf8PathBuf,
    pub spec_section: String,
    pub camera: Camera,
    pub lighting: Lighting,
    pub post_processing: PostProcessing,
    pub output: Output,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub physics: Option<PhysicsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<AnimationConfig>,
}

/// Build the manifest from a slice of `(plan, vrm_path)` pairs.
/// Caller is responsible for plan/.vrm pairing and for ensuring
/// `output_dir` exists; this function only translates types.
pub fn build_manifest(
    pairs: &[(TestPlan, Utf8PathBuf)],
    output_dir: Utf8PathBuf,
    renderer_name: String,
) -> BatchManifest {
    let tests = pairs
        .iter()
        .map(|(plan, vrm_path)| BatchTestEntry {
            test_id: plan.id.clone(),
            vrm_path: absolutize(vrm_path),
            spec_section: plan.spec_section.clone(),
            camera: plan.camera,
            lighting: plan.lighting.clone(),
            post_processing: plan.post_processing.clone(),
            output: plan.output,
            physics: plan.physics.clone(),
            animation: plan.animation.clone(),
        })
        .collect();

    BatchManifest {
        manifest_version: 1,
        output_dir: absolutize(&output_dir),
        renderer_name,
        renderer_version: None,
        tests,
    }
}

fn absolutize(p: &Utf8PathBuf) -> Utf8PathBuf {
    let std_path = p.as_std_path();
    let abs = if std_path.is_absolute() {
        std_path.to_path_buf()
    } else {
        std::env::current_dir()
            .expect("current_dir")
            .join(std_path)
    };
    Utf8PathBuf::from_path_buf(abs).expect("absolute path is utf-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolutize_handles_already_absolute_path() {
        let p = Utf8PathBuf::from("/tmp/already_abs");
        let out = absolutize(&p);
        assert_eq!(out, Utf8PathBuf::from("/tmp/already_abs"));
    }

    #[test]
    fn absolutize_joins_relative_path_with_cwd() {
        let p = Utf8PathBuf::from("relative/path");
        let out = absolutize(&p);
        assert!(out.as_str().starts_with('/'));
        assert!(out.as_str().ends_with("relative/path"));
    }
}
```

Then edit `crates/vrm-runner/src/lib.rs` to add `pub mod execute_batch;` after the existing `pub mod execute;`:

```rust
//! Conformance runner: reads test plans, drives renderer adapters, runs diff engine.

pub mod adapter;
pub mod cli;
pub mod diff;
pub mod execute;
pub mod execute_batch;
pub mod plan_to_ops;
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p vrm-runner --test execute_test_batch manifest
cargo test -p vrm-runner --lib execute_batch
```

Expected: PASS on both.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-runner/src/execute_batch.rs crates/vrm-runner/src/lib.rs crates/vrm-runner/tests/execute_test_batch.rs
git commit -m "feat(vrm-runner): batch manifest builder + types"
```

---

## Task 3: NDJSON results parser

**Files:**
- Modify: `crates/vrm-runner/src/execute_batch.rs` (append parser types + function)
- Modify: `crates/vrm-runner/tests/execute_test_batch.rs` (append tests)

- [ ] **Step 1: Write the failing test**

Append to `crates/vrm-runner/tests/execute_test_batch.rs`:

```rust
use vrm_runner::execute_batch::{parse_results_ndjson, BatchResults, ResultEntry, ResultStatus};

#[test]
fn parse_meta_envelope_and_two_entries() {
    let input = r#"{"_meta":true,"manifest_version":1,"renderer_name":"univrm","renderer_version":"v0.131.2","unity_version":"2022.3.50f1","render_pipeline":"Built-in RP","total_tests":2}
{"test_id":"a","status":"ok","output_path":"/tmp/a.png","blake3":"blake3:aaa","actual_color_space":"Srgb","render_seconds":0.18}
{"test_id":"b","status":"error","error":{"code":-32000,"message":"Unimplemented","data":{"phase":"L3"}}}
"#;
    let parsed: BatchResults = parse_results_ndjson(input).expect("parse");
    assert_eq!(parsed.meta.manifest_version, 1);
    assert_eq!(parsed.meta.renderer_name, "univrm");
    assert_eq!(parsed.meta.total_tests, 2);
    assert_eq!(parsed.entries.len(), 2);
    assert_eq!(parsed.entries[0].test_id, "a");
    assert!(matches!(parsed.entries[0].status, ResultStatus::Ok));
    assert!(matches!(parsed.entries[1].status, ResultStatus::Error));
    assert_eq!(parsed.entries[1].error.as_ref().unwrap().code, -32000);
}

#[test]
fn parse_rejects_missing_meta_envelope() {
    // First line is a test result, not a `_meta` envelope. Parser
    // must reject — the runner needs the meta line to validate the
    // batch before ingesting entries.
    let input = r#"{"test_id":"a","status":"ok","output_path":"/tmp/a.png"}
"#;
    let err = parse_results_ndjson(input).expect_err("must reject");
    assert!(
        err.to_string().contains("_meta"),
        "error should mention _meta, got: {err}"
    );
}

#[test]
fn parse_tolerates_partial_output_below_total_tests() {
    // _meta says total_tests=3 but only 1 entry follows (Unity crashed
    // mid-batch). Parser succeeds; the caller is responsible for
    // detecting the count mismatch.
    let input = r#"{"_meta":true,"manifest_version":1,"renderer_name":"univrm","renderer_version":"v0.131.2","unity_version":"2022.3.50f1","render_pipeline":"Built-in RP","total_tests":3}
{"test_id":"a","status":"ok","output_path":"/tmp/a.png","blake3":"blake3:aaa","actual_color_space":"Srgb","render_seconds":0.18}
"#;
    let parsed = parse_results_ndjson(input).expect("parse");
    assert_eq!(parsed.meta.total_tests, 3);
    assert_eq!(parsed.entries.len(), 1);
}

#[test]
fn parse_skips_blank_lines() {
    let input = "{\"_meta\":true,\"manifest_version\":1,\"renderer_name\":\"univrm\",\"renderer_version\":\"v0.131.2\",\"unity_version\":\"2022.3.50f1\",\"render_pipeline\":\"Built-in RP\",\"total_tests\":1}\n\n{\"test_id\":\"a\",\"status\":\"ok\",\"output_path\":\"/tmp/a.png\",\"blake3\":\"blake3:aaa\",\"actual_color_space\":\"Srgb\",\"render_seconds\":0.18}\n\n";
    let parsed = parse_results_ndjson(input).expect("parse");
    assert_eq!(parsed.entries.len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vrm-runner --test execute_test_batch parse_
```

Expected: FAIL — `parse_results_ndjson` and the supporting types don't exist.

- [ ] **Step 3: Add the parser**

Append to `crates/vrm-runner/src/execute_batch.rs`:

```rust
// =====================================================================
// Results parsing (Unity → runner)
// =====================================================================

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct BatchResultsMeta {
    pub manifest_version: u32,
    pub renderer_name: String,
    pub renderer_version: String,
    pub unity_version: String,
    pub render_pipeline: String,
    pub total_tests: usize,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ResultEntry {
    pub test_id: String,
    pub status: ResultStatus,
    #[serde(default)]
    pub output_path: Option<String>,
    #[serde(default)]
    pub blake3: Option<String>,
    #[serde(default)]
    pub actual_color_space: Option<String>,
    #[serde(default)]
    pub render_seconds: Option<f32>,
    #[serde(default)]
    pub error: Option<ResultError>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ResultStatus {
    Ok,
    Error,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ResultError {
    pub code: i32,
    pub message: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct BatchResults {
    pub meta: BatchResultsMeta,
    pub entries: Vec<ResultEntry>,
}

/// Parse the NDJSON results file. Line 1 must be the `_meta` envelope
/// (a JSON object with `"_meta": true`); subsequent non-blank lines
/// are per-test result entries. Blank lines are tolerated.
///
/// Returns an error if (a) the file is empty, (b) line 1 is not a
/// `_meta` envelope, or (c) any line fails to parse as JSON. Does
/// **not** validate that `entries.len() == meta.total_tests` —
/// partial output is a defined success condition; the caller
/// reconciles it.
pub fn parse_results_ndjson(s: &str) -> anyhow::Result<BatchResults> {
    let mut lines = s.lines().filter(|l| !l.trim().is_empty());
    let meta_line = lines
        .next()
        .ok_or_else(|| anyhow::anyhow!("results file is empty; expected _meta envelope"))?;
    let meta_value: serde_json::Value = serde_json::from_str(meta_line)
        .map_err(|e| anyhow::anyhow!("failed to parse first line as JSON: {e}; line={meta_line}"))?;
    if meta_value.get("_meta").and_then(|v| v.as_bool()) != Some(true) {
        anyhow::bail!(
            "first line must be a _meta envelope (object with \"_meta\": true); got: {meta_line}"
        );
    }
    let meta: BatchResultsMeta = serde_json::from_value(meta_value)
        .map_err(|e| anyhow::anyhow!("_meta envelope deserialization failed: {e}"))?;
    let mut entries = Vec::new();
    for (i, line) in lines.enumerate() {
        let entry: ResultEntry = serde_json::from_str(line).map_err(|e| {
            anyhow::anyhow!("failed to parse entry line {}: {e}; line={line}", i + 2)
        })?;
        entries.push(entry);
    }
    Ok(BatchResults { meta, entries })
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p vrm-runner --test execute_test_batch parse_
```

Expected: PASS on all four.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-runner/src/execute_batch.rs crates/vrm-runner/tests/execute_test_batch.rs
git commit -m "feat(vrm-runner): NDJSON results parser for batched mode"
```

---

## Task 4: Local manifest writer (per-renderer goldens-cache output)

**Files:**
- Modify: `crates/vrm-runner/src/execute_batch.rs` (append local-manifest writer)
- Modify: `crates/vrm-runner/Cargo.toml` (add `blake3` dep)
- Modify: `crates/vrm-runner/tests/execute_test_batch.rs` (append test)

- [ ] **Step 1: Add the dependency**

Edit `crates/vrm-runner/Cargo.toml`. Find the `[dependencies]` block and add `blake3` (already in workspace deps per `Cargo.toml` root):

```toml
[dependencies]
anyhow.workspace = true
blake3.workspace = true
clap.workspace = true
```

(Keep the existing entries; just insert the `blake3` line in alphabetical order.)

- [ ] **Step 2: Write the failing test**

Append to `crates/vrm-runner/tests/execute_test_batch.rs`:

```rust
use vrm_runner::execute_batch::{write_local_manifest, LocalManifestEntry};
use std::fs;

#[test]
fn local_manifest_writes_expected_shape() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let output_dir = tmp.path();

    // Synthesize a tiny "PNG" so BLAKE3 has something to hash.
    let png_a = output_dir.join("a.png");
    fs::write(&png_a, b"fake png bytes").unwrap();

    let entries = vec![LocalManifestEntry {
        test_id: "a".into(),
        renderer_name: "univrm".into(),
        renderer_version: "v0.131.2".into(),
        output_path: png_a.to_string_lossy().to_string(),
        blake3: None, // writer fills this from the file on disk
        actual_color_space: "Srgb".into(),
        status: "ok".into(),
        error_message: None,
    }];

    let manifest_path = output_dir.join("local-manifest.json");
    write_local_manifest(&manifest_path, &entries).expect("write");

    let written = fs::read_to_string(&manifest_path).expect("read manifest back");
    let v: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert_eq!(v["entries"][0]["test_id"], "a");
    assert_eq!(v["entries"][0]["renderer_name"], "univrm");
    assert!(
        v["entries"][0]["blake3"].as_str().unwrap().starts_with("blake3:"),
        "blake3 should be prefixed; got: {:?}",
        v["entries"][0]["blake3"]
    );
}
```

- [ ] **Step 3: Run test to verify it fails**

```bash
cargo test -p vrm-runner --test execute_test_batch local_manifest
```

Expected: FAIL — `write_local_manifest` and `LocalManifestEntry` don't exist.

- [ ] **Step 4: Add the writer**

Append to `crates/vrm-runner/src/execute_batch.rs`:

```rust
// =====================================================================
// Local manifest writer (per-renderer goldens-cache output)
// =====================================================================

use std::path::Path;

/// One entry in `goldens-cache/<renderer>/local-manifest.json`.
/// Format mirrors what `scripts/bootstrap-goldens.sh` already produces
/// for the per-test adapters; UniVRM writes the same shape so the
/// downstream consensus tooling doesn't need a UniVRM-specific path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalManifestEntry {
    pub test_id: String,
    pub renderer_name: String,
    pub renderer_version: String,
    pub output_path: String,
    /// If `None` at write time, the writer hashes the file at
    /// `output_path` and fills this in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blake3: Option<String>,
    pub actual_color_space: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct LocalManifestEnvelope<'a> {
    manifest_version: u32,
    entries: &'a [LocalManifestEntry],
}

/// Write the per-renderer local manifest. Computes BLAKE3 for any
/// entry whose `blake3` field is `None` by reading the PNG bytes at
/// `output_path`.
pub fn write_local_manifest(
    path: &Path,
    entries: &[LocalManifestEntry],
) -> anyhow::Result<()> {
    let mut materialized: Vec<LocalManifestEntry> = Vec::with_capacity(entries.len());
    for entry in entries {
        let mut e = entry.clone();
        if e.blake3.is_none() && e.status == "ok" {
            let bytes = std::fs::read(&e.output_path)
                .map_err(|err| anyhow::anyhow!("read {}: {err}", e.output_path))?;
            let hash = blake3::hash(&bytes);
            e.blake3 = Some(format!("blake3:{}", hash.to_hex()));
        }
        materialized.push(e);
    }
    let envelope = LocalManifestEnvelope {
        manifest_version: 1,
        entries: &materialized,
    };
    let bytes = serde_json::to_vec_pretty(&envelope)?;
    std::fs::write(path, bytes)?;
    Ok(())
}
```

- [ ] **Step 5: Run test to verify it passes**

```bash
cargo test -p vrm-runner --test execute_test_batch local_manifest
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-runner/Cargo.toml crates/vrm-runner/src/execute_batch.rs crates/vrm-runner/tests/execute_test_batch.rs
git commit -m "feat(vrm-runner): per-renderer local-manifest writer for batched mode"
```

---

## Task 5: Mock fixtures (4 shell scripts)

**Files:**
- Create: `crates/vrm-runner/tests/fixtures/mock-univrm-ok.sh`
- Create: `crates/vrm-runner/tests/fixtures/mock-univrm-partial.sh`
- Create: `crates/vrm-runner/tests/fixtures/mock-univrm-bad-meta.sh`
- Create: `crates/vrm-runner/tests/fixtures/mock-univrm-missing-meta.sh`

These shell scripts stand in for the real Unity batch invocation in contract tests. Each takes two CLI args: the manifest path and the results path. They write a deterministic `results.ndjson` (and any fake PNGs the manifest mentions) and exit.

- [ ] **Step 1: Create the happy-path mock**

Create `crates/vrm-runner/tests/fixtures/mock-univrm-ok.sh`:

```bash
#!/usr/bin/env bash
# Mock UniVRM adapter — happy path. Reads the manifest (arg 1), emits a
# valid results.ndjson with one "ok" entry per test_id (arg 2), and
# writes a placeholder PNG to each entry's output_path.
#
# Used by crates/vrm-runner/tests/execute_test_batch.rs to verify the
# runner-side contract without a real Unity install.

set -euo pipefail

manifest="$1"
results="$2"

# Tiny PNG: a 1x1 magenta pixel (matches the cross-adapter sentinel).
# Decoded bytes of a minimal PNG file:
PNG_BYTES_HEX="89504e470d0a1a0a0000000d49484452000000010000000108020000009077531d0000000c4944415478daedc1010100000080900052bdc1000000000049454e44ae426082"

# Resolve manifest fields via /usr/bin/python3 (universally available on
# macOS + most Linux CI runners). Avoids a jq dep.
python3 - "$manifest" "$results" "$PNG_BYTES_HEX" <<'PY'
import json, os, sys, binascii

manifest_path, results_path, png_hex = sys.argv[1], sys.argv[2], sys.argv[3]
with open(manifest_path) as f:
    m = json.load(f)

png_bytes = binascii.unhexlify(png_hex)
output_dir = m["output_dir"]
os.makedirs(output_dir, exist_ok=True)

with open(results_path, "w") as out:
    out.write(json.dumps({
        "_meta": True,
        "manifest_version": 1,
        "renderer_name": m["renderer_name"],
        "renderer_version": "mock-v0.131.2",
        "unity_version": "mock-2022.3.50f1",
        "render_pipeline": "Built-in RP",
        "total_tests": len(m["tests"]),
    }) + "\n")
    for t in m["tests"]:
        png_path = os.path.join(output_dir, f"{t['test_id']}.png")
        with open(png_path, "wb") as p:
            p.write(png_bytes)
        out.write(json.dumps({
            "test_id": t["test_id"],
            "status": "ok",
            "output_path": png_path,
            "actual_color_space": t["output"]["color_space"].capitalize(),
            "render_seconds": 0.01,
        }) + "\n")
PY
```

- [ ] **Step 2: Make it executable + create the others**

```bash
chmod +x crates/vrm-runner/tests/fixtures/mock-univrm-ok.sh
```

Create `crates/vrm-runner/tests/fixtures/mock-univrm-partial.sh`:

```bash
#!/usr/bin/env bash
# Mock UniVRM adapter — partial output. Writes _meta declaring N tests
# but only emits the first ceil(N/2) entries, then exits non-zero
# (simulates Unity crash mid-batch). The runner must detect the
# mismatch via meta.total_tests and report the missing entries as
# errors.

set -euo pipefail

manifest="$1"
results="$2"

python3 - "$manifest" "$results" <<'PY'
import json, os, sys, math

manifest_path, results_path = sys.argv[1], sys.argv[2]
with open(manifest_path) as f:
    m = json.load(f)

total = len(m["tests"])
emit_count = math.ceil(total / 2)
output_dir = m["output_dir"]
os.makedirs(output_dir, exist_ok=True)

with open(results_path, "w") as out:
    out.write(json.dumps({
        "_meta": True,
        "manifest_version": 1,
        "renderer_name": m["renderer_name"],
        "renderer_version": "mock-v0.131.2",
        "unity_version": "mock-2022.3.50f1",
        "render_pipeline": "Built-in RP",
        "total_tests": total,
    }) + "\n")
    for t in m["tests"][:emit_count]:
        png_path = os.path.join(output_dir, f"{t['test_id']}.png")
        with open(png_path, "wb") as p:
            p.write(b"")  # empty file ok for the mock
        out.write(json.dumps({
            "test_id": t["test_id"],
            "status": "ok",
            "output_path": png_path,
            "actual_color_space": "Srgb",
            "render_seconds": 0.01,
        }) + "\n")
PY

# Exit non-zero to simulate Unity crash. Runner should still parse what
# was written.
exit 1
```

```bash
chmod +x crates/vrm-runner/tests/fixtures/mock-univrm-partial.sh
```

Create `crates/vrm-runner/tests/fixtures/mock-univrm-bad-meta.sh`:

```bash
#!/usr/bin/env bash
# Mock UniVRM adapter — emits a malformed _meta envelope (missing
# total_tests). Parser must reject the batch with a clear error.

set -euo pipefail

results="$2"

cat > "$results" <<'EOF'
{"_meta":true,"manifest_version":1,"renderer_name":"univrm","renderer_version":"mock-v0.131.2","unity_version":"mock-2022.3.50f1","render_pipeline":"Built-in RP"}
EOF
```

```bash
chmod +x crates/vrm-runner/tests/fixtures/mock-univrm-bad-meta.sh
```

Create `crates/vrm-runner/tests/fixtures/mock-univrm-missing-meta.sh`:

```bash
#!/usr/bin/env bash
# Mock UniVRM adapter — emits a results file whose first line is NOT a
# _meta envelope. Parser must reject.

set -euo pipefail

results="$2"

cat > "$results" <<'EOF'
{"test_id":"a","status":"ok","output_path":"/tmp/a.png"}
EOF
```

```bash
chmod +x crates/vrm-runner/tests/fixtures/mock-univrm-missing-meta.sh
```

- [ ] **Step 3: Sanity-check by running each script manually**

```bash
cd $(mktemp -d) && cat > manifest.json <<'EOF'
{"manifest_version":1,"output_dir":"./out","renderer_name":"univrm","tests":[{"test_id":"a","vrm_path":"/tmp/a.vrm","spec_section":"x","camera":{"position":[0,0,0],"target":[0,0,0],"up":[0,1,0],"fov_degrees":30},"lighting":{"directional":{"dir":[0,0,0],"color":[1,1,1],"intensity":1},"ambient":{"color":[0,0,0],"intensity":0},"cast_shadows":false,"receive_shadows":false},"post_processing":{"tone_mapping":"none","exposure":1.0},"output":{"width":256,"height":256,"color_space":"srgb","msaa":4}}]}
EOF
mkdir -p out
$OLDPWD/crates/vrm-runner/tests/fixtures/mock-univrm-ok.sh manifest.json results.ndjson
cat results.ndjson
ls out/
```

Expected: `results.ndjson` contains a `_meta` line + one `ok` entry; `out/a.png` exists.

- [ ] **Step 4: Commit**

```bash
git add crates/vrm-runner/tests/fixtures/
git commit -m "test(vrm-runner): mock UniVRM adapter fixtures (4 scripts)"
```

---

## Task 6: Contract test — happy path (mock-ok)

**Files:**
- Modify: `crates/vrm-runner/tests/execute_test_batch.rs` (append test)
- Modify: `crates/vrm-runner/src/execute_batch.rs` (add `run` function)

- [ ] **Step 1: Write the failing test**

Append to `crates/vrm-runner/tests/execute_test_batch.rs`:

```rust
use vrm_runner::execute_batch::{run as run_batch, RunOptions};
use camino::Utf8Path;

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().unwrap().parent().unwrap().to_path_buf()
}

fn fixture(name: &str) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(
        workspace_root()
            .join("crates/vrm-runner/tests/fixtures")
            .join(name),
    )
    .unwrap()
}

fn write_synthetic_plan_files(dir: &std::path::Path, id: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let plan = synthetic_plan(id);
    let plan_path = dir.join(format!("{id}.test.yaml"));
    let vrm_path = dir.join(format!("{id}.vrm"));
    std::fs::write(&plan_path, serde_yml::to_string(&plan).unwrap()).unwrap();
    std::fs::write(&vrm_path, b"fake vrm").unwrap();
    (plan_path, vrm_path)
}

#[test]
fn happy_path_writes_local_manifest_with_blake3() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plans_dir = tmp.path().join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    write_synthetic_plan_files(&plans_dir, "alpha");
    write_synthetic_plan_files(&plans_dir, "bravo");

    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let opts = RunOptions {
        plans_dir: Utf8PathBuf::from_path_buf(plans_dir).unwrap(),
        adapter_bin: fixture("mock-univrm-ok.sh"),
        output_dir: Utf8PathBuf::from_path_buf(output_dir.clone()).unwrap(),
        renderer_name: "univrm".into(),
    };

    let summary = run_batch(&opts).expect("run");
    assert_eq!(summary.total_tests, 2);
    assert_eq!(summary.ok_count, 2);
    assert_eq!(summary.error_count, 0);

    let manifest_path = output_dir.join("local-manifest.json");
    let manifest_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    let entries = manifest_json["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    for entry in entries {
        assert_eq!(entry["renderer_name"], "univrm");
        assert!(
            entry["blake3"].as_str().unwrap().starts_with("blake3:"),
            "blake3 should be filled in; got: {:?}",
            entry["blake3"]
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vrm-runner --test execute_test_batch happy_path
```

Expected: FAIL — `run` and `RunOptions` don't exist.

- [ ] **Step 3: Add the `run` function**

Append to `crates/vrm-runner/src/execute_batch.rs`:

```rust
// =====================================================================
// Top-level `run` — discovery, manifest emission, adapter invocation,
// results ingestion, local-manifest writing
// =====================================================================

use std::process::Command;

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub plans_dir: Utf8PathBuf,
    pub adapter_bin: Utf8PathBuf,
    pub output_dir: Utf8PathBuf,
    pub renderer_name: String,
}

#[derive(Debug, Clone)]
pub struct RunSummary {
    pub total_tests: usize,
    pub ok_count: usize,
    pub error_count: usize,
    pub local_manifest_path: Utf8PathBuf,
}

/// Discover `*.test.yaml` files in `opts.plans_dir`, pair each with a
/// sibling `.vrm` (same stem), build the manifest, invoke the adapter
/// binary, parse the results NDJSON, and write the per-renderer local
/// manifest. Returns a summary; does not panic on per-test errors
/// (those land in the local manifest as `status: "error"`).
pub fn run(opts: &RunOptions) -> anyhow::Result<RunSummary> {
    let pairs = discover_plan_vrm_pairs(&opts.plans_dir)?;
    if pairs.is_empty() {
        anyhow::bail!(
            "no *.test.yaml files found in {}; nothing to run",
            opts.plans_dir
        );
    }

    std::fs::create_dir_all(opts.output_dir.as_std_path())?;

    let manifest = build_manifest(&pairs, opts.output_dir.clone(), opts.renderer_name.clone());

    // Write manifest + results paths into the output dir so they end up
    // in a stable location (helps debugging; not protocol-load-bearing).
    let manifest_path = opts.output_dir.join("manifest.json");
    let results_path = opts.output_dir.join("results.ndjson");
    std::fs::write(
        manifest_path.as_std_path(),
        serde_json::to_vec_pretty(&manifest)?,
    )?;

    // Invoke the adapter. Two positional args: manifest path, results
    // path. The mock fixtures and the real launcher.sh both use this
    // contract.
    let status = Command::new(opts.adapter_bin.as_std_path())
        .arg(manifest_path.as_std_path())
        .arg(results_path.as_std_path())
        .status();

    // Read whatever the adapter wrote, even if the exit was non-zero —
    // partial output is a defined success condition.
    let parsed = match std::fs::read_to_string(results_path.as_std_path()) {
        Ok(s) => parse_results_ndjson(&s)?,
        Err(e) => {
            anyhow::bail!(
                "adapter ({}) did not produce a readable results file at {}: {e}",
                opts.adapter_bin,
                results_path
            );
        }
    };
    // The adapter's exit code is informational at this layer. We don't
    // fail the run on non-zero exit because partial output is expected
    // to surface as per-test errors. Future scope: surface the exit
    // code in the summary so callers can choose to gate on it.
    let _ = status;

    // Build local-manifest entries from the parsed results, padding
    // missing test_ids (declared in _meta but not present in entries)
    // as errors.
    let local_entries = reconcile_to_local_manifest(
        &parsed,
        &pairs,
        &opts.renderer_name,
    );
    let local_manifest_path = opts.output_dir.join("local-manifest.json");
    write_local_manifest(local_manifest_path.as_std_path(), &local_entries)?;

    let ok_count = local_entries.iter().filter(|e| e.status == "ok").count();
    let error_count = local_entries.len() - ok_count;
    Ok(RunSummary {
        total_tests: local_entries.len(),
        ok_count,
        error_count,
        local_manifest_path,
    })
}

fn discover_plan_vrm_pairs(
    plans_dir: &Utf8PathBuf,
) -> anyhow::Result<Vec<(TestPlan, Utf8PathBuf)>> {
    let mut pairs = Vec::new();
    for entry in std::fs::read_dir(plans_dir.as_std_path())? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("yaml") {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) if n.ends_with(".test.yaml") => n,
            _ => continue,
        };
        let stem = name.trim_end_matches(".test.yaml");
        let vrm_path = plans_dir.join(format!("{stem}.vrm"));
        if !vrm_path.exists() {
            tracing::warn!(
                "skipping {}: sibling .vrm not found at {vrm_path}",
                path.display()
            );
            continue;
        }
        let plan_bytes = std::fs::read(&path)?;
        let plan: TestPlan = serde_yml::from_slice(&plan_bytes).map_err(|e| {
            anyhow::anyhow!("parse {}: {e}", path.display())
        })?;
        pairs.push((plan, vrm_path));
    }
    // Stable ordering — discovery order is filesystem-dependent.
    pairs.sort_by(|a, b| a.0.id.cmp(&b.0.id));
    Ok(pairs)
}

fn reconcile_to_local_manifest(
    parsed: &BatchResults,
    pairs: &[(TestPlan, Utf8PathBuf)],
    renderer_name: &str,
) -> Vec<LocalManifestEntry> {
    use std::collections::HashMap;
    let by_id: HashMap<&str, &ResultEntry> = parsed
        .entries
        .iter()
        .map(|e| (e.test_id.as_str(), e))
        .collect();
    let mut out = Vec::with_capacity(pairs.len());
    for (plan, _vrm) in pairs {
        if let Some(entry) = by_id.get(plan.id.as_str()) {
            match entry.status {
                ResultStatus::Ok => out.push(LocalManifestEntry {
                    test_id: entry.test_id.clone(),
                    renderer_name: renderer_name.to_string(),
                    renderer_version: parsed.meta.renderer_version.clone(),
                    output_path: entry.output_path.clone().unwrap_or_default(),
                    blake3: entry.blake3.clone(),
                    actual_color_space: entry
                        .actual_color_space
                        .clone()
                        .unwrap_or_else(|| "unknown".into()),
                    status: "ok".into(),
                    error_message: None,
                }),
                ResultStatus::Error => out.push(LocalManifestEntry {
                    test_id: entry.test_id.clone(),
                    renderer_name: renderer_name.to_string(),
                    renderer_version: parsed.meta.renderer_version.clone(),
                    output_path: String::new(),
                    blake3: None,
                    actual_color_space: "n/a".into(),
                    status: "error".into(),
                    error_message: entry
                        .error
                        .as_ref()
                        .map(|e| format!("code={} message={}", e.code, e.message)),
                }),
            }
        } else {
            // Declared in the plan dir; missing from the results file.
            // Treat as "batch terminated before this test ran."
            out.push(LocalManifestEntry {
                test_id: plan.id.clone(),
                renderer_name: renderer_name.to_string(),
                renderer_version: parsed.meta.renderer_version.clone(),
                output_path: String::new(),
                blake3: None,
                actual_color_space: "n/a".into(),
                status: "error".into(),
                error_message: Some("batch terminated before this test ran".into()),
            });
        }
    }
    out
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p vrm-runner --test execute_test_batch happy_path
```

Expected: PASS — both test entries get `status: "ok"`, both have BLAKE3 hashes.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-runner/src/execute_batch.rs crates/vrm-runner/tests/execute_test_batch.rs
git commit -m "feat(vrm-runner): top-level execute_batch::run + happy-path contract test"
```

---

## Task 7: Contract test — partial output (mock-partial)

**Files:**
- Modify: `crates/vrm-runner/tests/execute_test_batch.rs` (append test)

- [ ] **Step 1: Write the failing test (it may pass already; the test asserts behavior the implementation should already provide)**

Append to `crates/vrm-runner/tests/execute_test_batch.rs`:

```rust
#[test]
fn partial_output_marks_missing_tests_as_errors() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plans_dir = tmp.path().join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    write_synthetic_plan_files(&plans_dir, "a1");
    write_synthetic_plan_files(&plans_dir, "a2");
    write_synthetic_plan_files(&plans_dir, "a3");

    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let opts = RunOptions {
        plans_dir: Utf8PathBuf::from_path_buf(plans_dir).unwrap(),
        adapter_bin: fixture("mock-univrm-partial.sh"),
        output_dir: Utf8PathBuf::from_path_buf(output_dir.clone()).unwrap(),
        renderer_name: "univrm".into(),
    };

    let summary = run_batch(&opts).expect("run");
    assert_eq!(summary.total_tests, 3, "all 3 declared tests should appear");
    // mock-partial emits ceil(3/2) = 2 ok entries; 1 missing.
    assert_eq!(summary.ok_count, 2);
    assert_eq!(summary.error_count, 1);

    let manifest_path = output_dir.join("local-manifest.json");
    let manifest_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    let entries = manifest_json["entries"].as_array().unwrap();
    let error_entry = entries.iter().find(|e| e["status"] == "error").unwrap();
    assert!(
        error_entry["error_message"]
            .as_str()
            .unwrap()
            .contains("batch terminated"),
        "missing test should be marked as batch-terminated; got: {:?}",
        error_entry
    );
}
```

- [ ] **Step 2: Run test to verify it passes**

```bash
cargo test -p vrm-runner --test execute_test_batch partial_output
```

Expected: PASS — `reconcile_to_local_manifest` already pads missing entries with `error_message: "batch terminated before this test ran"`.

- [ ] **Step 3: Commit**

```bash
git add crates/vrm-runner/tests/execute_test_batch.rs
git commit -m "test(vrm-runner): partial-output mock asserts batch-terminated reconciliation"
```

---

## Task 8: Contract test — malformed `_meta` (mock-bad-meta)

**Files:**
- Modify: `crates/vrm-runner/tests/execute_test_batch.rs` (append test)

- [ ] **Step 1: Write the failing test**

Append to `crates/vrm-runner/tests/execute_test_batch.rs`:

```rust
#[test]
fn malformed_meta_fails_with_clear_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plans_dir = tmp.path().join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    write_synthetic_plan_files(&plans_dir, "z");

    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let opts = RunOptions {
        plans_dir: Utf8PathBuf::from_path_buf(plans_dir).unwrap(),
        adapter_bin: fixture("mock-univrm-bad-meta.sh"),
        output_dir: Utf8PathBuf::from_path_buf(output_dir).unwrap(),
        renderer_name: "univrm".into(),
    };

    let err = run_batch(&opts).expect_err("should fail on missing total_tests");
    let msg = err.to_string();
    assert!(
        msg.contains("total_tests") || msg.contains("_meta"),
        "error message should mention the missing field or _meta envelope; got: {msg}"
    );
}

#[test]
fn missing_meta_envelope_fails_with_clear_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plans_dir = tmp.path().join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    write_synthetic_plan_files(&plans_dir, "y");

    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let opts = RunOptions {
        plans_dir: Utf8PathBuf::from_path_buf(plans_dir).unwrap(),
        adapter_bin: fixture("mock-univrm-missing-meta.sh"),
        output_dir: Utf8PathBuf::from_path_buf(output_dir).unwrap(),
        renderer_name: "univrm".into(),
    };

    let err = run_batch(&opts).expect_err("should fail without _meta envelope");
    assert!(
        err.to_string().contains("_meta"),
        "error message should mention _meta; got: {err}"
    );
}
```

- [ ] **Step 2: Run tests to verify they pass**

```bash
cargo test -p vrm-runner --test execute_test_batch malformed_meta missing_meta_envelope
```

Expected: PASS on both. The parser added in Task 3 already rejects both shapes; this test locks the contract.

- [ ] **Step 3: Commit**

```bash
git add crates/vrm-runner/tests/execute_test_batch.rs
git commit -m "test(vrm-runner): malformed _meta and missing-meta envelopes both rejected"
```

---

## Task 9: Wire the CLI dispatch to `execute_batch::run`

**Files:**
- Modify: `crates/vrm-runner/src/cli.rs` (replace the stub bail with a real dispatch)

- [ ] **Step 1: Write the failing test**

Append to `crates/vrm-runner/tests/execute_test_batch.rs`:

```rust
#[test]
fn cli_invocation_end_to_end_with_mock() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plans_dir = tmp.path().join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    write_synthetic_plan_files(&plans_dir, "cli1");

    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let out = Command::new(runner_bin())
        .args([
            "execute-test-batch",
            "--plans",
            plans_dir.to_str().unwrap(),
            "--adapter-bin",
            fixture("mock-univrm-ok.sh").as_str(),
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--renderer-name",
            "univrm",
            "--json",
        ])
        .output()
        .expect("spawn runner");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "runner should succeed end-to-end; stdout={stdout} stderr={stderr}"
    );
    let summary: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("JSON summary on stdout");
    assert_eq!(summary["total_tests"], 1);
    assert_eq!(summary["ok_count"], 1);
    assert_eq!(summary["error_count"], 0);
    assert!(
        output_dir.join("local-manifest.json").exists(),
        "local-manifest.json should be written"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vrm-runner --test execute_test_batch cli_invocation_end_to_end
```

Expected: FAIL — the CLI dispatch still bails with "not yet implemented (stub)".

- [ ] **Step 3: Replace the stub dispatch**

Edit `crates/vrm-runner/src/cli.rs`. Locate the `Cmd::ExecuteTestBatch { .. } =>` arm in the `run` function (added in Task 1) and replace it with:

```rust
        Cmd::ExecuteTestBatch {
            plans,
            adapter_bin,
            output_dir,
            renderer_name,
            json: emit_json,
        } => {
            let opts = crate::execute_batch::RunOptions {
                plans_dir: plans,
                adapter_bin,
                output_dir,
                renderer_name,
            };
            let summary = crate::execute_batch::run(&opts)?;
            if emit_json {
                let payload = json!({
                    "ok": summary.error_count == 0,
                    "total_tests": summary.total_tests,
                    "ok_count": summary.ok_count,
                    "error_count": summary.error_count,
                    "local_manifest": summary.local_manifest_path,
                });
                println!("{}", serde_json::to_string(&payload)?);
            } else {
                println!(
                    "batched {} tests: {} ok, {} error → {}",
                    summary.total_tests,
                    summary.ok_count,
                    summary.error_count,
                    summary.local_manifest_path
                );
            }
            Ok(())
        }
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p vrm-runner --test execute_test_batch cli_invocation_end_to_end
```

Expected: PASS — the CLI runs the batch through the mock adapter end-to-end and emits the JSON summary.

- [ ] **Step 5: Run the full contract test suite to confirm no regressions**

```bash
cargo test -p vrm-runner --test execute_test_batch
```

Expected: PASS on all (>= 10 tests across happy, partial, malformed-meta, missing-meta, end-to-end).

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-runner/src/cli.rs crates/vrm-runner/tests/execute_test_batch.rs
git commit -m "feat(vrm-runner): wire execute-test-batch CLI dispatch to execute_batch::run"
```

---

## Task 10: Create `adapters/univrm/launcher.sh`

**Files:**
- Create: `adapters/univrm/launcher.sh`

The launcher is the bridge between the runner's `--adapter-bin` flag and Unity Editor. It resolves the Unity binary (via `UNITY_BIN` env or default install path), the Unity project path (relative to the launcher), and forwards the manifest + results paths to Unity via `-batchmode -executeMethod Conformance.RunBatch --`. In L1+L2 the C# entry point is a stub that writes a `_meta` + N `Unimplemented` lines.

- [ ] **Step 1: Create the directory + launcher**

```bash
mkdir -p adapters/univrm
```

Create `adapters/univrm/launcher.sh`:

```bash
#!/usr/bin/env bash
# UniVRM adapter launcher — invokes Unity Editor in batchmode to run
# Conformance.RunBatch against a manifest. Filesystem-as-protocol;
# arguments: $1 = manifest.json, $2 = results.ndjson.
#
# UNITY_BIN env override: explicit path to the Unity binary.
# UNITY_VERSION env: which Hub-installed version to default to
#   (only used if UNITY_BIN is unset).
#
# Defaults to 2022.3.50f1 (the version pinned in
# UniVRMConformance/ProjectSettings/ProjectVersion.txt). If Unity isn't
# installed at the default path, the launcher prints a clear error and
# exits 127 — the runner reports this as a batch-level failure.

set -euo pipefail

manifest="${1:?manifest path required}"
results="${2:?results path required}"

UNITY_VERSION="${UNITY_VERSION:-2022.3.50f1}"
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
  -executeMethod Conformance.RunBatch \
  -logFile - \
  -- \
  "$manifest" \
  "$results" \
  2>&1 | tee "$log_path"

unity_exit=${PIPESTATUS[0]}
exit "$unity_exit"
```

- [ ] **Step 2: Make it executable**

```bash
chmod +x adapters/univrm/launcher.sh
```

- [ ] **Step 3: Smoke-check the missing-Unity path**

```bash
UNITY_BIN=/definitely/not/a/real/path adapters/univrm/launcher.sh /tmp/nonexistent /tmp/results.ndjson
echo "exit: $?"
```

Expected: stderr says "Unity binary not executable at /definitely/not/a/real/path"; exit code 127.

- [ ] **Step 4: Commit**

```bash
git add adapters/univrm/launcher.sh
git commit -m "feat(adapters/univrm): launcher script wrapping Unity batchmode"
```

---

## Task 11: Create the Unity project skeleton

**Files:**
- Create: `adapters/univrm/UniVRMConformance/ProjectSettings/ProjectVersion.txt`
- Create: `adapters/univrm/UniVRMConformance/Packages/manifest.json`
- Create: `adapters/univrm/.gitignore`

The Unity project is minimal at L1: pinned Unity version, UniVRM via UPM, no scenes, no assets beyond what Tasks 12–13 add. CI will open it in batchmode and confirm it compiles.

- [ ] **Step 1: Create the version pin**

```bash
mkdir -p adapters/univrm/UniVRMConformance/ProjectSettings
mkdir -p adapters/univrm/UniVRMConformance/Packages
mkdir -p adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime
mkdir -p adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode
```

Create `adapters/univrm/UniVRMConformance/ProjectSettings/ProjectVersion.txt`:

```
m_EditorVersion: 2022.3.50f1
m_EditorVersionWithRevision: 2022.3.50f1 (179a13042d2c)
```

- [ ] **Step 2: Create the UPM manifest pinning UniVRM**

Create `adapters/univrm/UniVRMConformance/Packages/manifest.json`:

```json
{
  "dependencies": {
    "com.vrmc.univrm": "https://github.com/vrm-c/UniVRM.git?path=/Assets/VRM10#v0.131.2",
    "com.vrmc.vrmshaders": "https://github.com/vrm-c/UniVRM.git?path=/Assets/VRMShaders#v0.131.2",
    "com.unity.test-framework": "1.4.5",
    "com.unity.ide.rider": "3.0.31",
    "com.unity.ide.visualstudio": "2.0.22",
    "com.unity.modules.imageconversion": "1.0.0",
    "com.unity.modules.jsonserialize": "1.0.0",
    "com.unity.modules.physics": "1.0.0",
    "com.unity.modules.unitywebrequest": "1.0.0"
  },
  "scopedRegistries": []
}
```

- [ ] **Step 3: Add .gitignore for Unity-generated paths**

Create `adapters/univrm/.gitignore`:

```gitignore
# Unity-generated, machine-specific or rebuildable; never commit.
UniVRMConformance/Library/
UniVRMConformance/Temp/
UniVRMConformance/Logs/
UniVRMConformance/UserSettings/
UniVRMConformance/obj/
UniVRMConformance/*.csproj
UniVRMConformance/*.sln

# Launcher postmortems
last-run.log
```

- [ ] **Step 4: Verify nothing else has snuck in**

```bash
ls -la adapters/univrm/UniVRMConformance/
ls adapters/univrm/UniVRMConformance/ProjectSettings/
ls adapters/univrm/UniVRMConformance/Packages/
```

Expected output: only `ProjectSettings/`, `Packages/`, `Assets/` — no `Library/` (would mean Unity opened the project before commit) and no `*.meta` (will appear once Unity opens the project; tracked by Unity's import process, not by L1 setup).

- [ ] **Step 5: Commit**

```bash
git add adapters/univrm/UniVRMConformance/ProjectSettings/ProjectVersion.txt \
        adapters/univrm/UniVRMConformance/Packages/manifest.json \
        adapters/univrm/.gitignore
git commit -m "feat(adapters/univrm): Unity 2022.3 project skeleton with UniVRM v0.131.2 pinned"
```

---

## Task 12: `Conformance.cs` stub entry point

**Files:**
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs`
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.asmdef`

The C# entry point reads the manifest path from `Environment.GetCommandLineArgs()`, writes a `_meta` line + one `Unimplemented` line per test, fsyncs after each line, and exits. No UniVRM APIs touched at L1+L2.

- [ ] **Step 1: Create the assembly definition**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.asmdef`:

```json
{
  "name": "Conformance",
  "rootNamespace": "Conformance",
  "references": [],
  "includePlatforms": [],
  "excludePlatforms": [],
  "allowUnsafeCode": false,
  "overrideReferences": false,
  "precompiledReferences": [],
  "autoReferenced": true,
  "defineConstraints": [],
  "versionDefines": [],
  "noEngineReferences": false
}
```

- [ ] **Step 2: Create the stub entry point**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs`:

```csharp
// L1+L2 stub: parses the manifest, writes one `_meta` line + one
// `Unimplemented` entry per test_id, exits cleanly. Real rendering
// arrives in L3; spring-bone physics in L4.
//
// Invoked from launcher.sh as:
//   Unity -batchmode -projectPath ... -executeMethod Conformance.RunBatch -- manifest.json results.ndjson
//
// The `--` separator routes everything after it into
// `Environment.GetCommandLineArgs()` as plain strings (Unity passes
// everything past `--` through unmodified).

using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using UnityEditor;
using UnityEngine;

namespace Conformance
{
    public static class Conformance
    {
        public static void RunBatch()
        {
            try
            {
                var args = ExtractAdapterArgs();
                if (args.Count < 2)
                {
                    Debug.LogError(
                        $"Conformance.RunBatch: expected 2 args (manifest, results); got {args.Count}");
                    EditorApplication.Exit(2);
                    return;
                }
                var manifestPath = args[0];
                var resultsPath = args[1];

                var manifestJson = File.ReadAllText(manifestPath);
                var manifest = JsonUtility.FromJson<ManifestDto>(manifestJson);
                if (manifest == null || manifest.tests == null)
                {
                    Debug.LogError($"Conformance.RunBatch: failed to parse manifest at {manifestPath}");
                    EditorApplication.Exit(3);
                    return;
                }

                using var stream = new FileStream(
                    resultsPath, FileMode.Create, FileAccess.Write, FileShare.Read);

                // _meta envelope (line 1).
                var meta = new MetaDto
                {
                    _meta = true,
                    manifest_version = manifest.manifest_version,
                    renderer_name = manifest.renderer_name,
                    renderer_version = "L1L2-stub",
                    unity_version = Application.unityVersion,
                    render_pipeline = "Built-in RP",
                    total_tests = manifest.tests.Length,
                };
                WriteLine(stream, JsonUtility.ToJson(meta));

                // One entry per test_id: all Unimplemented at this layer.
                foreach (var t in manifest.tests)
                {
                    var entry = new EntryDto
                    {
                        test_id = t.test_id,
                        status = "error",
                        error = new ErrorDto
                        {
                            code = -32000,
                            message = "Unimplemented (L1+L2 stub)",
                            data = new ErrorDataDto { phase = "L3" },
                        },
                    };
                    WriteLine(stream, JsonUtility.ToJson(entry));
                }

                EditorApplication.Exit(0);
            }
            catch (Exception e)
            {
                Debug.LogError($"Conformance.RunBatch: unhandled exception: {e}");
                EditorApplication.Exit(1);
            }
        }

        private static List<string> ExtractAdapterArgs()
        {
            var args = Environment.GetCommandLineArgs();
            var result = new List<string>();
            var capture = false;
            foreach (var a in args)
            {
                if (capture)
                {
                    result.Add(a);
                }
                else if (a == "--")
                {
                    capture = true;
                }
            }
            return result;
        }

        private static void WriteLine(FileStream stream, string json)
        {
            var bytes = Encoding.UTF8.GetBytes(json + "\n");
            stream.Write(bytes, 0, bytes.Length);
            // Flush-to-disk after each entry: survives OOM kill / segfault
            // mid-batch. See docs/superpowers/specs/2026-05-12-adapter-
            // univrm-design.md "Partial output" for rationale.
            stream.Flush(flushToDisk: true);
        }
    }

    [Serializable]
    internal class ManifestDto
    {
        public int manifest_version;
        public string output_dir;
        public string renderer_name;
        public string renderer_version;
        public TestEntryDto[] tests;
    }

    [Serializable]
    internal class TestEntryDto
    {
        public string test_id;
        public string vrm_path;
        public string spec_section;
    }

    [Serializable]
    internal class MetaDto
    {
        public bool _meta;
        public int manifest_version;
        public string renderer_name;
        public string renderer_version;
        public string unity_version;
        public string render_pipeline;
        public int total_tests;
    }

    [Serializable]
    internal class EntryDto
    {
        public string test_id;
        public string status;
        public ErrorDto error;
    }

    [Serializable]
    internal class ErrorDto
    {
        public int code;
        public string message;
        public ErrorDataDto data;
    }

    [Serializable]
    internal class ErrorDataDto
    {
        public string phase;
    }
}
```

- [ ] **Step 3: (Cannot verify locally without Unity)**

This file isn't compiled until Unity opens the project (Task 14 wires the CI workflow for that). No local cargo/Rust test asserts this file.

- [ ] **Step 4: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/
git commit -m "feat(adapters/univrm): Conformance.RunBatch L1+L2 stub (all tests Unimplemented)"
```

---

## Task 13: EditMode test for the stub entry point

**Files:**
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/Conformance.Tests.EditMode.asmdef`
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/ManifestRoundtripTest.cs`

Tests that compile inside Unity. They don't exercise the full `RunBatch` end-to-end (that requires a real subprocess invocation, which the CI workflow handles in Task 14) but they lock down the DTO contract: a fixture `manifest.json` round-trips through `JsonUtility.FromJson<ManifestDto>` without loss.

- [ ] **Step 1: Create the test assembly definition**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/Conformance.Tests.EditMode.asmdef`:

```json
{
  "name": "Conformance.Tests.EditMode",
  "rootNamespace": "Conformance.Tests",
  "references": [
    "Conformance",
    "UnityEngine.TestRunner",
    "UnityEditor.TestRunner"
  ],
  "includePlatforms": ["Editor"],
  "excludePlatforms": [],
  "allowUnsafeCode": false,
  "overrideReferences": true,
  "precompiledReferences": [
    "nunit.framework.dll"
  ],
  "autoReferenced": false,
  "defineConstraints": ["UNITY_INCLUDE_TESTS"],
  "versionDefines": [],
  "noEngineReferences": false
}
```

- [ ] **Step 2: Create the round-trip test**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/ManifestRoundtripTest.cs`:

```csharp
// EditMode test: locks the Manifest DTO contract by serializing a
// known-good manifest body through JsonUtility and asserting key
// fields survive the round-trip. Runs inside Unity Test Framework;
// no scene, no Play mode.

using NUnit.Framework;
using UnityEngine;

namespace Conformance.Tests
{
    public class ManifestRoundtripTest
    {
        private const string FixtureJson = @"{
            ""manifest_version"": 1,
            ""output_dir"": ""/tmp/out"",
            ""renderer_name"": ""univrm"",
            ""tests"": [
                { ""test_id"": ""alpha"", ""vrm_path"": ""/tmp/alpha.vrm"", ""spec_section"": ""VRMC_materials_mtoon"" },
                { ""test_id"": ""bravo"", ""vrm_path"": ""/tmp/bravo.vrm"", ""spec_section"": ""VRMC_materials_mtoon"" }
            ]
        }";

        [Test]
        public void ManifestDeserializesPreservingTestIds()
        {
            var manifest = JsonUtility.FromJson<ManifestDto>(FixtureJson);
            Assert.IsNotNull(manifest, "manifest should parse");
            Assert.AreEqual(1, manifest.manifest_version);
            Assert.AreEqual("univrm", manifest.renderer_name);
            Assert.AreEqual(2, manifest.tests.Length);
            Assert.AreEqual("alpha", manifest.tests[0].test_id);
            Assert.AreEqual("/tmp/alpha.vrm", manifest.tests[0].vrm_path);
            Assert.AreEqual("bravo", manifest.tests[1].test_id);
        }
    }
}
```

- [ ] **Step 3: (Cannot verify locally without Unity)**

The test is exercised by the CI workflow added in Task 14. Locally, an engineer with Unity 2022.3.50f1 installed can verify via:

```bash
"$UNITY_BIN" \
  -batchmode \
  -projectPath adapters/univrm/UniVRMConformance \
  -runTests \
  -testPlatform EditMode \
  -testResults /tmp/results.xml \
  -logFile -
```

Expected: NUnit XML lists `ManifestDeserializesPreservingTestIds` with `result="Passed"`. Engineers without Unity skip this step; CI catches regressions.

- [ ] **Step 4: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/
git commit -m "test(adapters/univrm): EditMode manifest round-trip test"
```

---

## Task 14: CI workflow — `univrm.yml`

**Files:**
- Create: `.github/workflows/univrm.yml`

The CI workflow opens the Unity project in batchmode via `game-ci/unity-actions`, asserts the C# project compiles (no errors, no missing packages), and runs EditMode tests. Does **not** attempt rendering — GitHub-hosted runners lack the GPU+display config needed for `-batchmode` (without `-nographics`) with Metal. Render-path coverage lives in `scripts/smoke-univrm.sh` (added in L3).

- [ ] **Step 1: Create the workflow**

Create `.github/workflows/univrm.yml`:

```yaml
name: univrm

# Builds the Unity project in batchmode + runs EditMode tests on
# macos-latest. No rendering — that needs a display + Metal context
# the GitHub-hosted runner doesn't reliably provide for batchmode
# without -nographics. Render-path coverage runs locally on the
# maintainer's Mac Studio via scripts/smoke-univrm.sh (L3 plan).
#
# Modelled on .github/workflows/swift.yml (vrm-metal-kit's build-only
# CI shape) and .github/workflows/godot-vrm.yml (game-engine-driven
# adapter shape).
#
# No untrusted-input usage: this workflow does not read PR titles,
# commit messages, issue bodies, or any other user-controlled fields
# into run: commands.

on:
  pull_request:
    paths:
      - 'adapters/univrm/**'
      - 'crates/vrm-runner/**'
      - '.github/workflows/univrm.yml'
  push:
    branches: [main]
    paths:
      - 'adapters/univrm/**'
      - 'crates/vrm-runner/**'
      - '.github/workflows/univrm.yml'

jobs:
  build-validate:
    runs-on: macos-latest

    # Unity license activation requires three secrets to be set as
    # repo-level secrets. With Unity Personal these are free; the
    # game-ci docs describe the activation flow:
    # https://game.ci/docs/github/activation
    #
    # If the secrets are not set, this job will exit non-zero on the
    # `game-ci/unity-test-runner` step. This is intentional — until
    # someone activates a license, the workflow correctly reports
    # "not testable in CI."
    env:
      UNITY_LICENSE: ${{ secrets.UNITY_LICENSE }}
      UNITY_EMAIL: ${{ secrets.UNITY_EMAIL }}
      UNITY_PASSWORD: ${{ secrets.UNITY_PASSWORD }}

    steps:
      - uses: actions/checkout@v4
        with:
          lfs: false

      # Cache the Library/ folder. Without this, every CI run re-imports
      # UniVRM from its UPM git URL (~5+ minutes).
      - uses: actions/cache@v4
        with:
          path: adapters/univrm/UniVRMConformance/Library
          key: Library-univrm-${{ hashFiles('adapters/univrm/UniVRMConformance/Packages/manifest.json') }}
          restore-keys: |
            Library-univrm-

      - name: Run EditMode tests
        uses: game-ci/unity-test-runner@v4
        with:
          projectPath: adapters/univrm/UniVRMConformance
          unityVersion: 2022.3.50f1
          testMode: EditMode
          artifactsPath: artifacts/univrm-test-results
          githubToken: ${{ secrets.GITHUB_TOKEN }}

      - name: Upload test artifacts
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: univrm-test-results
          path: artifacts/univrm-test-results
```

- [ ] **Step 2: Smoke-check the workflow file syntax locally**

```bash
# `actionlint` is the canonical GitHub Actions linter; install via
# `brew install actionlint` if not present.
actionlint .github/workflows/univrm.yml
```

Expected: no errors. If `actionlint` isn't installed, skip and rely on GitHub's parse-on-push.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/univrm.yml
git commit -m "ci(univrm): build-validate workflow + EditMode tests via game-ci"
```

---

## Task 15: Adapter README

**Files:**
- Create: `adapters/univrm/README.md`

Mirrors `adapters/godot-vrm/README.md` in shape: status table, runtime dependency, build instructions, test commands, runner-invocation example, status footnotes.

- [ ] **Step 1: Create the README**

Create `adapters/univrm/README.md`:

```markdown
# univrm renderer adapter

Bridges [UniVRM](https://github.com/vrm-c/UniVRM) (the VRM consortium reference implementation, Unity-based) to the project's renderer-agnostic operation contract documented at [`docs/operation-contract.md`](../../docs/operation-contract.md).

Architecture differs from the [three-vrm](../three-vrm/README.md) and [vrm-metal-kit](../vrm-metal-kit/README.md) adapters: Unity batch mode is idiomatic for "run, do work, exit," and Unity's stdout pollution makes in-process JSON-RPC fragile (see [`rfcs/0003`](../../rfcs/0003-engine-idiom-divergence.md)). This adapter uses **batched one-shot**: the Rust runner builds a JSON manifest of test_ids, invokes Unity once for the whole batch, the C# entry point writes per-test results to an NDJSON file, exits. Filesystem-as-protocol; no stdio, no TCP, no Rust shim.

```
runner ──manifest.json──▶ launcher.sh ──▶ Unity -batchmode -executeMethod Conformance.RunBatch
runner ◀───results.ndjson──────────────── Unity (incrementally written, fsync per entry)
```

## Status

| Phase | Status |
|---|---|
| L1 — project skeleton + UPM pin | scaffolded |
| L2 — Rust runner subcommand + NDJSON contract + mock-fixture tests | scaffolded |
| L3 — Phase 1 ops real (`load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`) | deferred |
| L4 — Phase 2 spring-bone physics ops | deferred |

Through L2, the runner side speaks the full batch contract end-to-end against mock-binary fixtures (`crates/vrm-runner/tests/fixtures/mock-univrm-*.sh`). The Unity side compiles cleanly with UniVRM v0.131.2 pinned via UPM but `Conformance.RunBatch` is a stub — every test returns `-32000 Unimplemented` with `data.phase: "L3"`. Real rendering arrives in the L3 plan.

## Runtime dependency

Unity 2022.3.50f1 must be installed via Unity Hub. The launcher resolves the binary at `/Applications/Unity/Hub/Editor/2022.3.50f1/Unity.app/Contents/MacOS/Unity` by default; override with `UNITY_BIN` env or `UNITY_VERSION` env (the Hub install path is derived from `UNITY_VERSION`).

License: Unity Personal (free for individuals/orgs under $200K USD/yr). Activation is a one-time manual flow per machine; see [Unity's docs](https://docs.unity3d.com/Manual/ManagingYourUnityLicense.html). Lapses every ~6 months and needs re-activation; no automated regression test for the lapse path.

## Build

The Rust side (subcommand + tests) builds as part of the workspace:

```bash
cargo build -p vrm-runner --release
```

The Unity project is opened lazily by the launcher when Unity is invoked. On first open, Unity will re-import UniVRM from its UPM git URL (~5 minutes); subsequent opens use the cached `Library/` directory.

## Tests

```bash
# Rust contract tests (no Unity needed)
cargo test -p vrm-runner --test execute_test_batch

# Unity EditMode tests (requires Unity 2022.3.50f1 installed)
"$UNITY_BIN" \
  -batchmode \
  -projectPath adapters/univrm/UniVRMConformance \
  -runTests \
  -testPlatform EditMode \
  -testResults /tmp/univrm-test-results.xml \
  -logFile -

# Smoke (L3+, requires real rendering)
scripts/smoke-univrm.sh   # not present until L3
```

CI (`.github/workflows/univrm.yml`) runs both the Rust contract tests (via the workspace `rust.yml`) and the Unity EditMode tests (via `game-ci/unity-test-runner`) on `macos-latest`.

## How the runner invokes it

```bash
cargo run -p vrm-runner -- execute-test-batch \
  --plans <corpus-dir>/ \
  --adapter-bin adapters/univrm/launcher.sh \
  --output-dir goldens-cache/univrm/ \
  --renderer-name univrm \
  --json
```

`<corpus-dir>` is a directory of `*.test.yaml` + matching `.vrm` files (one pair per test_id), as produced by `cargo run -p vrm-asset-generator -- emit-sweep --output-dir <corpus-dir>`.

## How it's *not* like other adapters

| Adapter | Adapter shape | Engine idiom fit |
|---|---|---|
| three-vrm | Direct JSON-RPC over stdio | Node.js reserves stdout cleanly |
| vrm-metal-kit | Direct JSON-RPC over stdio | Swift Foundation reserves stdout cleanly |
| godot-vrm | Persistent JSON-RPC over TCP via Rust shim | Godot --headless is idiomatic long-running |
| **univrm** | **Batched one-shot via filesystem** | **Unity batch mode is idiomatic "run, do work, exit"** |

See [`rfcs/0003`](../../rfcs/0003-engine-idiom-divergence.md) for the principle this divergence sits beneath.
```

- [ ] **Step 2: Commit**

```bash
git add adapters/univrm/README.md
git commit -m "docs(adapters/univrm): README mirrors godot-vrm shape"
```

---

## Task 16: Workspace fmt + clippy + full test pass

**Files:** none (verification only).

- [ ] **Step 1: Run fmt**

```bash
cargo fmt --all
```

If there are unrelated drift, do not fix them in this plan — they belong in a separate cleanup commit. The new code (Tasks 2, 3, 4, 6, 9) should fmt-clean by construction.

- [ ] **Step 2: Run clippy with `-D warnings`**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: zero warnings on the new code. If clippy fires on anything in `execute_batch.rs` or the new tests, fix inline before continuing.

- [ ] **Step 3: Run the full workspace test suite**

```bash
cargo test --workspace
```

Expected: all tests green, including the >= 10 new tests in `execute_test_batch.rs`.

- [ ] **Step 4: (Local-only, if Unity is installed) verify the stub end-to-end against real Unity**

```bash
# Generate a tiny corpus.
tmp=$(mktemp -d)
cargo run -p vrm-asset-generator --release -- emit-default --id e2e-stub --output-dir "$tmp/plans"

# Run the batch through the real launcher.
mkdir -p "$tmp/out"
cargo run -p vrm-runner --release -- execute-test-batch \
  --plans "$tmp/plans" \
  --adapter-bin adapters/univrm/launcher.sh \
  --output-dir "$tmp/out" \
  --renderer-name univrm \
  --json
```

Expected: JSON summary on stdout shows `total_tests: 1, ok_count: 0, error_count: 1` (the stub marks the one test as `Unimplemented`); `$tmp/out/local-manifest.json` exists with one entry whose `status: "error"` and `error_message` contains `Unimplemented (L1+L2 stub)`.

Engineers without Unity installed skip this step. CI workflow (Task 14) handles the equivalent verification automatically.

- [ ] **Step 5: Confirm git status is clean before final commit**

```bash
git status
```

Expected: working tree clean (all Tasks 1–15 committed).

---

## Spec self-review pass

After completing Tasks 1–16, the plan covers:

- ✅ **Batched one-shot architecture (spec § Adapter shape)** — Task 1 introduces the subcommand; Tasks 2, 6, 9 implement the Rust orchestration; Tasks 10–13 build the Unity side.
- ✅ **Manifest schema (spec § Components → `manifest.json`)** — Task 2 defines `BatchManifest` + `BatchTestEntry`; Task 9 wires the CLI to emit it.
- ✅ **NDJSON results format with `_meta` envelope + fsync (spec § Result file format)** — Task 3 (parser), Task 12 (writer with `Flush(flushToDisk: true)`).
- ✅ **Error envelope conventions (spec § Error handling)** — Task 12 emits `-32000 Unimplemented` with `data.phase`, the exact convention sourced from godot-vrm/vrm-metal-kit; the parser in Task 3 understands the same shape.
- ✅ **Partial-output handling (spec § Partial output)** — Task 7 contract test asserts the runner pads missing entries with `batch terminated before this test ran`.
- ✅ **Cross-renderer coord convention (spec § Coordinate-system convention)** — **NOT in this plan.** Requires touching SceneSetup.cs which exists only as a `Conformance.RunBatch` stub at L1+L2; arrives in L3.
- ✅ **Color-space handling (spec § Color-space handling)** — **NOT in this plan.** Requires Capture.cs + project color-space setting; L3.
- ✅ **MSAA, tone mapping, magenta sentinel (spec § MSAA, tone mapping, magenta sentinel)** — **NOT in this plan.** L3.
- ✅ **CI guardrails (spec § Testing → CI guardrails)** — Task 14.
- ✅ **Three test layers (spec § Testing)** — Layer 1 (Unity EditMode): Task 13. Layer 2 (Rust contract tests with mocks): Tasks 5–9. Layer 3 (local integration via real Unity): **NOT in this plan**; arrives with L3 + `scripts/smoke-univrm.sh`.

**Placeholders scan:** no TBD/TODO/FIXME tokens in the plan; every code block is complete. The "Cannot verify locally without Unity" steps (Tasks 12, 13) are honest acknowledgments that CI is the source of truth for those specific verifications, not placeholders for missing content.

**Type consistency:** `BatchManifest`/`BatchTestEntry`/`BatchResultsMeta`/`ResultEntry`/`ResultStatus`/`ResultError`/`LocalManifestEntry`/`RunOptions`/`RunSummary` — all introduced in Tasks 2–6 and referenced consistently in later tasks. The Unity-side DTOs (`ManifestDto`/`TestEntryDto`/`MetaDto`/`EntryDto`/`ErrorDto`/`ErrorDataDto`) live in Task 12 and don't cross the language boundary except via JSON shape (which Task 3's parser deserializes through the Rust types).

**Spec coverage gaps (deliberate, deferred to L3+):**
- C# `SceneSetup` + glTF→Unity Z-mirror conversion
- C# `Capture` PNG readback with sRGB target handling
- C# UniVRM v0.131 API calls (`Vrm10.LoadPathAsync`, MToon material binding)
- C# `PhysicsDriver` manual VRMSpringBone stepping
- `scripts/smoke-univrm.sh` local-only integration test
- `scripts/bootstrap-goldens.sh` `RUN_UNIVRM=1` flag

These gaps are tracked in `adapters/univrm/README.md`'s Status table (L3 + L4 rows marked "deferred"). A follow-up plan (`docs/superpowers/plans/2026-XX-XX-adapter-univrm-L3.md`) picks up the L3 scope.
