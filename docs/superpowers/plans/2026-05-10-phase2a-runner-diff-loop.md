# Phase 2A — Runner Diff Loop + Plan→Diff Bridge

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the runner's open loop: today `execute-test-plan` produces a PNG and stops. After this plan, the runner consumes a reference PNG, runs SSIM + bbox-relative property assertions from the test plan, and emits a `DiffResult` JSON document — making every subsequent renderer adapter (L3 VRMMetalKit, three-vrm, etc.) instantly produce a pass/fail signal.

**Architecture:** Three pieces wire together: (1) a type bridge from `vrm_test_plan::PropertyAssertion`/`BboxRegion` to `vrm_diff_engine::property::PropertyAssertion`/`BboxRegion` (identical shapes, currently duplicated), (2) a `runner::diff` module orchestrating SSIM + per-property evaluation against a reference PNG, and (3) two CLI surfaces — a standalone `vrm-runner diff` subcommand for offline use, plus a new `--reference` flag on `execute-test-plan` that runs the diff inline. The work consolidates the duplicated `BboxRegion`/`PropertyAssertion` types: `vrm-diff-engine` becomes the source of truth and `vrm-test-plan` re-exports.

**Tech Stack:** Rust 2021 (1.88), existing crates: `vrm-test-plan`, `vrm-diff-engine`, `vrm-runner`, `vrm-ops`. No new external dependencies. Tests build PNGs in-memory with the `image` crate (already a workspace dep).

**YAGNI scope guards:**
- v0.2 SSIM is single-renderer-vs-reference, not consensus mode (consensus is Phase 2B+).
- `BboxRegion` consolidation: re-export from one canonical location, don't introduce a new shared crate just for two enums.
- The `diff` subcommand reads PNGs from disk; no S3 fetch in this round (`pull-goldens.sh` is a separate Phase 2 task).
- `DiffResult` writes to stdout when `--json` is set; no file output, no manifest update inline (manifest update is the operator's responsibility via `push-goldens` + a future signing step).

---

## File Layout

| File | Status | Responsibility |
|---|---|---|
| `crates/vrm-test-plan/src/lib.rs` | Modify | Remove local `BboxRegion`/`PropertyAssertion` enum/struct definitions; re-export from `vrm-diff-engine`. |
| `crates/vrm-test-plan/Cargo.toml` | Modify | Add `vrm-diff-engine` as a path dep. |
| `crates/vrm-diff-engine/src/property.rs` | Modify | Add `pub use` of `BboxRegion`/`PropertyAssertion` so external crates can name them; no behavioral change. |
| `crates/vrm-runner/Cargo.toml` | Modify | Confirm `vrm-diff-engine` path dep is present (it already is). |
| `crates/vrm-runner/src/diff.rs` | Create | Orchestrator: takes plan + render PNG path + reference PNG path, returns `DiffResult`. |
| `crates/vrm-runner/src/lib.rs` | Modify | Declare `pub mod diff;`. |
| `crates/vrm-runner/src/cli.rs` | Modify | Add `Cmd::Diff` variant; extend `Cmd::ExecuteTestPlan` with optional `--reference` arg; update `describe` catalog with `diff` op + `output_schema`. |
| `crates/vrm-runner/src/execute.rs` | Modify | When `reference` is present in `ExecuteOptions`, call `diff::diff_one` after the render and include `DiffResult` in `ExecuteResult`. |
| `crates/vrm-runner/tests/diff_integration.rs` | Create | End-to-end: synthesize two PNGs in-memory, build a synthetic plan, call `diff::diff_one`, assert SSIM ~ 1.0 (identical) or expected band (different). |
| `scripts/smoke.sh` | Modify | After the self-diff sanity step, exercise the full `runner diff` subcommand against the placeholder PNG (self-diff). |

---

## Section A — Type bridge consolidation

### Task A1: Move `BboxRegion`/`PropertyAssertion` to `vrm-diff-engine` (consolidation, TDD)

**Files:**
- Modify: `crates/vrm-test-plan/Cargo.toml`
- Modify: `crates/vrm-test-plan/src/lib.rs`
- Modify: `crates/vrm-diff-engine/src/property.rs`
- Modify: `crates/vrm-test-plan/tests/roundtrip.rs` (verify YAML round-trip still works after the swap)

The consolidation is mechanically simple: `vrm-diff-engine::property` already owns `BboxRegion` and `PropertyAssertion`. `vrm-test-plan` has its own copies. After this task, `vrm-test-plan` re-exports from `vrm-diff-engine`. The YAML serialization shape must NOT change (existing test plans on disk must still parse).

- [ ] **Step 1: Confirm shape parity**

The two `BboxRegion` enums and `PropertyAssertion` structs already have identical fields and identical `serde` attributes. Confirm by reading both:

```bash
grep -A 20 "pub enum BboxRegion" crates/vrm-test-plan/src/lib.rs
grep -A 20 "pub enum BboxRegion" crates/vrm-diff-engine/src/property.rs
grep -A 8 "pub struct PropertyAssertion" crates/vrm-test-plan/src/lib.rs
grep -A 8 "pub struct PropertyAssertion" crates/vrm-diff-engine/src/property.rs
```

Both must match in field order, names, types, and `#[serde]` attributes. **If they differ, stop and align — DO NOT silently change YAML semantics.**

- [ ] **Step 2: Add path dep**

`crates/vrm-test-plan/Cargo.toml` — add to `[dependencies]`:

```toml
vrm-diff-engine = { path = "../vrm-diff-engine" }
```

- [ ] **Step 3: Replace local definitions with re-exports**

In `crates/vrm-test-plan/src/lib.rs`, find the `pub struct PropertyAssertion { ... }` and `pub enum BboxRegion { ... }` blocks. Delete them. Replace with:

```rust
pub use vrm_diff_engine::property::{BboxRegion, PropertyAssertion};
```

Keep all OTHER types (`TestPlan`, `Camera`, `Lighting`, etc.) where they are.

- [ ] **Step 4: Verify YAML round-trip still works**

Run: `cargo test -p vrm-test-plan`

Expected: existing `roundtrip.rs` tests still pass. The serde output shape is unchanged because `vrm-diff-engine`'s definitions have identical `#[serde]` attributes.

If a test fails (likely cause: a `#[serde(rename_all)]` mismatch you missed), align the diff-engine side to match the test-plan side, since on-disk test plans depend on the test-plan crate's wire format. Re-run.

- [ ] **Step 5: Verify the workspace still compiles**

Run: `cargo build --workspace`

Expected: clean build. The `vrm-diff-engine` types are now reachable via either `vrm_diff_engine::property::BboxRegion` or `vrm_test_plan::BboxRegion`; both refer to the same type.

- [ ] **Step 6: Run all tests + clippy + fmt**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-test-plan/Cargo.toml crates/vrm-test-plan/src/lib.rs
git commit -m "refactor(test-plan): consolidate BboxRegion/PropertyAssertion in vrm-diff-engine"
```

---

## Section B — Diff orchestrator

### Task B1: `diff::diff_one` core function (TDD)

**Files:**
- Create: `crates/vrm-runner/src/diff.rs`
- Modify: `crates/vrm-runner/src/lib.rs`
- Create: `crates/vrm-runner/tests/diff_integration.rs`

The orchestrator is small: load both PNGs (via `image::open`), call `ssim_pngs` for the score, evaluate each `plan.properties[i]` against the **render** PNG (not the reference — properties measure absolute behavior, not similarity), compose into `DiffResult`.

Per `vrm-test-plan::Diff`, the threshold and reference renderer name come from the test plan; `diff_one` reports whether SSIM passed the threshold.

- [ ] **Step 1: Failing integration test**

`crates/vrm-runner/tests/diff_integration.rs`:

```rust
//! Verifies the runner's diff-engine bridge: synthesize two PNGs, run
//! diff_one against a synthetic plan, assert structural results.

use camino::Utf8PathBuf;
use vrm_runner::diff::diff_one;
use vrm_test_plan::{
    AmbientLight, Camera, ColorSpace, Diff, DiffMode, DirectionalLight, Lighting, Output,
    PostProcessing, TestPlan, ToneMapping,
};

fn make_solid_png(path: &std::path::Path, w: u32, h: u32, rgb: [u8; 3]) {
    image::RgbImage::from_fn(w, h, |_, _| image::Rgb(rgb))
        .save(path)
        .expect("save png");
}

fn synthetic_plan(id: &str, threshold: f32) -> TestPlan {
    TestPlan {
        id: id.into(),
        spec_section: "test".into(),
        asset: "synthetic.vrm".into(),
        camera: Camera {
            position: [0.0, 0.0, 1.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_degrees: 30.0,
        },
        lighting: Lighting {
            directional: DirectionalLight {
                dir: [0.0, -1.0, 0.0],
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
            width: 64,
            height: 64,
            color_space: ColorSpace::Linear,
            msaa: 4,
        },
        diff: Diff {
            mode: DiffMode::Ssim,
            threshold,
            reference_renderer: "test-renderer".into(),
        },
        ignore_renderers: Vec::new(),
        properties: Vec::new(),
    }
}

#[test]
fn identical_pngs_pass_ssim_threshold() {
    let dir = tempfile::tempdir().unwrap();
    let render = dir.path().join("render.png");
    let reference = dir.path().join("reference.png");

    // Avatar grey on magenta sentinel background (matches diff-engine
    // bbox-detection convention).
    let mut img = image::RgbImage::from_pixel(64, 64, image::Rgb([255, 0, 255]));
    for y in 16..48 {
        for x in 16..48 {
            img.put_pixel(x, y, image::Rgb([128, 128, 128]));
        }
    }
    img.save(&render).unwrap();
    img.save(&reference).unwrap();

    let plan = synthetic_plan("identical_test", 0.985);
    let render_path = Utf8PathBuf::from_path_buf(render).unwrap();
    let reference_path = Utf8PathBuf::from_path_buf(reference).unwrap();

    let result = diff_one(&plan, &render_path, &reference_path, "test-renderer").unwrap();
    assert_eq!(result.test_id, "identical_test");
    assert_eq!(result.renderer, "test-renderer");
    assert_eq!(result.reference_renderer, "test-renderer");
    assert!(result.ssim > 0.99, "identical PNGs SSIM should be ~1, got {}", result.ssim);
    assert!(result.ssim_passed);
    assert!(result.overall_passed());
}

#[test]
fn different_pngs_fail_ssim_threshold() {
    let dir = tempfile::tempdir().unwrap();
    let render = dir.path().join("render.png");
    let reference = dir.path().join("reference.png");

    make_solid_png(&render, 64, 64, [0, 0, 0]);
    make_solid_png(&reference, 64, 64, [255, 255, 255]);

    let plan = synthetic_plan("different_test", 0.985);
    let render_path = Utf8PathBuf::from_path_buf(render).unwrap();
    let reference_path = Utf8PathBuf::from_path_buf(reference).unwrap();

    let result = diff_one(&plan, &render_path, &reference_path, "test-renderer").unwrap();
    assert!(result.ssim < 0.5, "black vs white SSIM should be low, got {}", result.ssim);
    assert!(!result.ssim_passed);
    assert!(!result.overall_passed());
}

#[test]
fn property_assertion_is_evaluated_on_render() {
    use vrm_diff_engine::property::{BboxRegion, PropertyAssertion};

    let dir = tempfile::tempdir().unwrap();
    let render = dir.path().join("render.png");
    let reference = dir.path().join("reference.png");

    // Avatar at known luminance ~0.5 on magenta sentinel.
    let mut img = image::RgbImage::from_pixel(64, 64, image::Rgb([255, 0, 255]));
    for y in 16..48 {
        for x in 16..48 {
            img.put_pixel(x, y, image::Rgb([128, 128, 128]));
        }
    }
    img.save(&render).unwrap();
    img.save(&reference).unwrap();

    let mut plan = synthetic_plan("with_property", 0.985);
    plan.properties.push(PropertyAssertion {
        name: "avg_lum_full".into(),
        region: BboxRegion::BboxFull,
        expected: 128.0 / 255.0,
        tolerance: 0.05,
    });

    let render_path = Utf8PathBuf::from_path_buf(render).unwrap();
    let reference_path = Utf8PathBuf::from_path_buf(reference).unwrap();

    let result = diff_one(&plan, &render_path, &reference_path, "test-renderer").unwrap();
    assert_eq!(result.properties.len(), 1);
    let prop = &result.properties[0];
    assert_eq!(prop.name, "avg_lum_full");
    assert!(prop.passed, "expected ~0.5, got actual={}", prop.actual);
    assert!(result.overall_passed());
}
```

- [ ] **Step 2: Run failing test**

`cargo test -p vrm-runner --test diff_integration` → compile error (`diff` module + `diff_one` don't exist).

- [ ] **Step 3: Implement `diff::diff_one`**

`crates/vrm-runner/src/diff.rs`:

```rust
//! Bridges a `vrm_test_plan::TestPlan` plus two rendered PNGs (the
//! produced render and a known-good reference) into a `DiffResult`. SSIM
//! compares render vs reference; property assertions are evaluated on
//! the render alone (they measure absolute renderer behavior, not
//! similarity to a reference).

use anyhow::{Context, Result};
use camino::Utf8Path;
use vrm_diff_engine::property::eval_property;
use vrm_diff_engine::result::DiffResult;
use vrm_diff_engine::ssim::ssim_pngs;
use vrm_test_plan::TestPlan;

pub fn diff_one(
    plan: &TestPlan,
    render: &Utf8Path,
    reference: &Utf8Path,
    renderer: &str,
) -> Result<DiffResult> {
    let ssim = ssim_pngs(render, reference)
        .with_context(|| format!("ssim render={render} reference={reference}"))?
        as f32;
    let ssim_passed = ssim >= plan.diff.threshold;

    let mut properties = Vec::with_capacity(plan.properties.len());
    if !plan.properties.is_empty() {
        let render_img = image::open(render.as_std_path())
            .with_context(|| format!("decode render: {render}"))?
            .to_rgb8();
        for assertion in &plan.properties {
            let result = eval_property(&render_img, assertion)
                .with_context(|| format!("eval_property '{}'", assertion.name))?;
            properties.push(result);
        }
    }

    Ok(DiffResult {
        test_id: plan.id.clone(),
        renderer: renderer.into(),
        reference_renderer: plan.diff.reference_renderer.clone(),
        ssim,
        ssim_threshold: plan.diff.threshold,
        ssim_passed,
        properties,
    })
}
```

- [ ] **Step 4: Wire into lib.rs**

Add to `crates/vrm-runner/src/lib.rs`:

```rust
pub mod diff;
```

- [ ] **Step 5: Tests pass**

```bash
cargo test -p vrm-runner --test diff_integration
```

Expected: all 3 tests green.

- [ ] **Step 6: Workspace clean**

```bash
cargo clippy -p vrm-runner --all-targets -- -D warnings
cargo fmt -p vrm-runner -- --check
```

Both clean.

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-runner/src/diff.rs crates/vrm-runner/src/lib.rs crates/vrm-runner/tests/diff_integration.rs
git commit -m "feat(runner): diff_one orchestrator bridging plan + render + reference to DiffResult"
```

---

## Section C — CLI surfaces

### Task C1: Standalone `vrm-runner diff` subcommand

**Files:**
- Modify: `crates/vrm-runner/src/cli.rs`

A new top-level subcommand: `vrm-runner diff --plan <plan.yaml> --render <render.png> --reference <ref.png> --renderer-name <name> [--json]`. Reads plan + both PNGs, calls `diff_one`, emits `DiffResult` JSON to stdout (or human text without `--json`). Exit code 0 if `overall_passed()`, 1 otherwise.

- [ ] **Step 1: Add subcommand variant**

In `Cmd` enum:

```rust
/// Diff a render PNG against a reference PNG using a test plan.
Diff {
    #[arg(long)]
    plan: Utf8PathBuf,
    #[arg(long)]
    render: Utf8PathBuf,
    #[arg(long)]
    reference: Utf8PathBuf,
    #[arg(long, default_value = "vrm-metal-kit")]
    renderer_name: String,
    #[arg(long)]
    json: bool,
},
```

- [ ] **Step 2: Implement handler in `run()`**

Add the arm:

```rust
Cmd::Diff {
    plan,
    render,
    reference,
    renderer_name,
    json: emit_json,
} => {
    use crate::diff::diff_one;
    use crate::execute::load_plan;

    let plan_value = load_plan(&plan)?;
    let result = diff_one(&plan_value, &render, &reference, &renderer_name)?;

    let passed = result.overall_passed();

    if emit_json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!(
            "{}: SSIM={:.4} (threshold {:.4}, {}), {} property assertion(s) {}",
            result.test_id,
            result.ssim,
            result.ssim_threshold,
            if result.ssim_passed { "PASS" } else { "FAIL" },
            result.properties.len(),
            if result.properties.iter().all(|p| p.passed) { "PASS" } else { "FAIL" },
        );
    }

    if !passed {
        std::process::exit(1);
    }
    Ok(())
}
```

- [ ] **Step 3: Update `describe` catalog**

In the `Describe` arm of `run()`, add `diff` to the operations object:

```json
"diff": {
    "summary": "Diff a render PNG against a reference PNG using a test plan; emit DiffResult JSON",
    "input_schema": {
        "type": "object",
        "required": ["plan", "render", "reference"],
        "properties": {
            "plan": { "type": "string" },
            "render": { "type": "string" },
            "reference": { "type": "string" },
            "renderer_name": { "type": "string" }
        }
    },
    "output_schema": {
        "type": "object",
        "properties": {
            "test_id": { "type": "string" },
            "renderer": { "type": "string" },
            "reference_renderer": { "type": "string" },
            "ssim": { "type": "number" },
            "ssim_threshold": { "type": "number" },
            "ssim_passed": { "type": "boolean" },
            "properties": { "type": "array" }
        }
    }
}
```

- [ ] **Step 4: Smoke-test the CLI**

```bash
cargo build -p vrm-runner
cargo run -p vrm-runner -- describe --format json | python3 -c "import json,sys; d=json.load(sys.stdin); print(sorted(d['operations'].keys()))"
```

Expected: `['describe', 'diff', 'execute-test-plan', 'plan-test-plan']` (or whichever subset is currently registered — `diff` must appear).

```bash
# Identical-PNG self-diff: synthesize a placeholder PNG, run the CLI.
mkdir -p /tmp/c1-smoke
python3 -c "
import struct, zlib
def png(w, h, rgb):
    sig = b'\x89PNG\r\n\x1a\n'
    ihdr = struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0)
    raw = b''.join(b'\x00' + bytes(rgb*w) for _ in range(h))
    idat = zlib.compress(raw)
    def chunk(t,d):
        c = zlib.crc32(t+d)
        return struct.pack('>I',len(d))+t+d+struct.pack('>I',c)
    return sig + chunk(b'IHDR', ihdr) + chunk(b'IDAT', idat) + chunk(b'IEND', b'')
import sys
sys.stdout.buffer.write(png(64,64,[128,128,128]))
" > /tmp/c1-smoke/render.png
cp /tmp/c1-smoke/render.png /tmp/c1-smoke/reference.png

# Generate a plan via asset-generator
cargo run -p vrm-asset-generator -- emit-default --id c1_smoke --output-dir /tmp/c1-smoke --json

cargo run -p vrm-runner -- diff \
  --plan /tmp/c1-smoke/c1_smoke.test.yaml \
  --render /tmp/c1-smoke/render.png \
  --reference /tmp/c1-smoke/reference.png \
  --renderer-name test-renderer \
  --json
echo "exit: $?"
```

Expected: stdout contains a `DiffResult` JSON with `ssim` ~ 1.0, `ssim_passed: true`, and the process exits 0.

- [ ] **Step 5: Workspace clean**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

All green.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-runner/src/cli.rs
git commit -m "feat(runner): add diff subcommand with JSON output and exit-code gating"
```

---

### Task C2: Inline diff in `execute-test-plan` via `--reference`

**Files:**
- Modify: `crates/vrm-runner/src/execute.rs`
- Modify: `crates/vrm-runner/src/cli.rs`

Extend `ExecuteOptions` with an optional `reference: Option<Utf8PathBuf>`. When set, `execute_plan` runs the existing render path, then calls `diff_one`, populating a new `Option<DiffResult>` field on `ExecuteResult`. The `execute-test-plan` CLI subcommand gets a `--reference` flag.

- [ ] **Step 1: Extend `ExecuteOptions` and `ExecuteResult`**

In `crates/vrm-runner/src/execute.rs`, modify:

```rust
#[derive(Debug, Clone)]
pub struct ExecuteOptions {
    pub adapter_bin: Utf8PathBuf,
    pub adapter_args: Vec<String>,
    pub asset_dir: Utf8PathBuf,
    pub output_dir: Utf8PathBuf,
    pub renderer_name: String,
    pub emit_progress_ndjson: bool,
    /// If provided, diff the produced render against this reference PNG and
    /// include the result in `ExecuteResult::diff`.
    pub reference: Option<Utf8PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ExecuteResult {
    pub test_id: String,
    pub renderer: String,
    pub output_png: Utf8PathBuf,
    pub actual_color_space: vrm_ops::tools::ColorSpace,
    /// Populated only when `ExecuteOptions::reference` was set.
    pub diff: Option<vrm_diff_engine::result::DiffResult>,
}
```

- [ ] **Step 2: Run the diff after the render in `execute_plan`**

In `execute_plan`, after `Adapter::shutdown()` returns, before constructing `ExecuteResult`:

```rust
let diff = if let Some(reference) = &opts.reference {
    progress(opts, "diff", &plan.id, json!({ "reference": reference }));
    let diff = crate::diff::diff_one(plan, &png, reference, &opts.renderer_name)
        .map_err(|e| anyhow::anyhow!("diff error: {e}"))?;
    Some(diff)
} else {
    None
};

Ok(ExecuteResult {
    test_id: plan.id.clone(),
    renderer: opts.renderer_name.clone(),
    output_png: Utf8PathBuf::from(render.output_path),
    actual_color_space: render.actual_color_space,
    diff,
})
```

(If `Adapter::shutdown()` consumed `self`, the diff happens after that line; the adapter is no longer needed for the diff step.)

- [ ] **Step 3: Add `--reference` to the `ExecuteTestPlan` CLI variant**

In `crates/vrm-runner/src/cli.rs`, extend the variant:

```rust
ExecuteTestPlan {
    #[arg(long)]
    plan: Utf8PathBuf,
    #[arg(long)]
    adapter_bin: Utf8PathBuf,
    #[arg(long, value_delimiter = ' ', num_args = 0..)]
    adapter_args: Vec<String>,
    #[arg(long)]
    asset_dir: Utf8PathBuf,
    #[arg(long)]
    output_dir: Utf8PathBuf,
    #[arg(long, default_value = "vrm-metal-kit")]
    renderer_name: String,
    /// Optional reference PNG to diff against.
    #[arg(long)]
    reference: Option<Utf8PathBuf>,
    #[arg(long)]
    json: bool,
},
```

In the handler, pass `reference` into `ExecuteOptions` and include `diff` in the JSON summary when present:

```rust
let opts = ExecuteOptions {
    adapter_bin,
    adapter_args,
    asset_dir,
    output_dir,
    renderer_name,
    emit_progress_ndjson: emit_json,
    reference,
};
let result = execute_plan(&plan_value, &opts)?;
if emit_json {
    let mut summary = serde_json::json!({
        "ok": true,
        "test_id": result.test_id,
        "renderer": result.renderer,
        "output_png": result.output_png,
        "actual_color_space": format!("{:?}", result.actual_color_space)
    });
    if let Some(diff) = &result.diff {
        summary["diff"] = serde_json::to_value(diff)?;
        summary["overall_passed"] = serde_json::Value::Bool(diff.overall_passed());
    }
    println!("{}", serde_json::to_string(&summary)?);
} else {
    println!("rendered {} → {}", result.test_id, result.output_png);
    if let Some(diff) = &result.diff {
        println!(
            "  diff: SSIM={:.4} ({}), overall {}",
            diff.ssim,
            if diff.ssim_passed { "PASS" } else { "FAIL" },
            if diff.overall_passed() { "PASS" } else { "FAIL" }
        );
    }
}
```

Update the `execute-test-plan` entry in the `describe` catalog:

```json
"execute-test-plan": {
    "summary": "Execute a YAML test plan against one renderer adapter; optionally diff against a reference PNG",
    "input_schema": {
        "type": "object",
        "required": ["plan", "adapter_bin", "asset_dir", "output_dir"],
        "properties": {
            "plan": { "type": "string" },
            "adapter_bin": { "type": "string" },
            "adapter_args": { "type": "array", "items": { "type": "string" } },
            "asset_dir": { "type": "string" },
            "output_dir": { "type": "string" },
            "reference": { "type": "string" }
        }
    },
    "output_schema": {
        "type": "object",
        "properties": {
            "ok": { "type": "boolean" },
            "test_id": { "type": "string" },
            "renderer": { "type": "string" },
            "output_png": { "type": "string" },
            "actual_color_space": { "type": "string" },
            "diff": { "type": ["object", "null"] },
            "overall_passed": { "type": "boolean" }
        }
    }
}
```

- [ ] **Step 4: Workspace clean**

```bash
cargo build -p vrm-runner
cargo test -p vrm-runner
cargo clippy -p vrm-runner --all-targets -- -D warnings
cargo fmt -p vrm-runner -- --check
```

All green.

- [ ] **Step 5: Smoke**

```bash
cargo run -p vrm-runner -- describe --format json \
  | python3 -c "import json,sys; d=json.load(sys.stdin); op=d['operations']['execute-test-plan']; print('input has reference?:','reference' in op['input_schema']['properties']); print('output has diff?:','diff' in op['output_schema']['properties'])"
```

Expected: both `True`.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-runner/src/execute.rs crates/vrm-runner/src/cli.rs
git commit -m "feat(runner): execute-test-plan --reference inlines diff into result"
```

---

## Section D — Smoke + docs

### Task D1: Exercise the diff loop in `scripts/smoke.sh`

**Files:**
- Modify: `scripts/smoke.sh`

After the existing `self_diff` step (which exercises `vrm_diff_engine::ssim::ssim_pngs` directly), add a step that exercises the new `vrm-runner diff` CLI: take the placeholder PNG, copy it as both `render.png` and `reference.png`, run the runner's `diff` subcommand against the existing test plan, parse the JSON output to confirm `ssim_passed: true`.

- [ ] **Step 1: Add the diff-loop step**

Locate the section in `scripts/smoke.sh` that runs the `self_diff` example. After it, add:

```bash
echo "==> Running runner diff loop"
RUNNER_RENDER="$OUTPUTS/runner_diff_render.png"
RUNNER_REF="$OUTPUTS/runner_diff_reference.png"
cp "$PNG" "$RUNNER_RENDER"
cp "$PNG" "$RUNNER_REF"

DIFF_PLAN="$ASSETS/smoke_default.test.yaml"
if [ -f "$DIFF_PLAN" ]; then
    DIFF_OUT=$(cargo run --release -p vrm-runner -- diff \
        --plan "$DIFF_PLAN" \
        --render "$RUNNER_RENDER" \
        --reference "$RUNNER_REF" \
        --renderer-name smoke-test \
        --json) || {
        echo "smoke: runner diff failed (unexpected — should self-diff to SSIM~1)" >&2
        exit 1
    }
    echo "$DIFF_OUT" | python3 -c "
import json, sys
d = json.load(sys.stdin)
assert d['ssim_passed'], f'expected ssim_passed=True, got {d}'
assert d['ssim'] > 0.99, f'expected SSIM ~1, got {d[\"ssim\"]}'
print(f'  SSIM={d[\"ssim\"]:.4f}, properties={len(d[\"properties\"])}, overall PASS')
"
else
    echo "  (skipped: no plan at $DIFF_PLAN)"
fi
```

Place this after the existing `cargo run --release -p vrm-diff-engine --example self_diff` invocation.

- [ ] **Step 2: Run the smoke**

```bash
SMOKE_SKIP_RENDER=1 ./scripts/smoke.sh
```

Expected: previous output + a new `==> Running runner diff loop` section that prints `SSIM=1.0000, properties=N, overall PASS`. Exit 0.

- [ ] **Step 3: Commit**

```bash
git add scripts/smoke.sh
git commit -m "chore(smoke): exercise runner diff CLI on self-diff during smoke run"
```

---

### Task D2: Document the diff loop in operation-contract + README

**Files:**
- Modify: `docs/operation-contract.md`
- Modify: `README.md`

Add a brief section to `docs/operation-contract.md` describing the new `diff` operation (with the same JSON shapes as the rest of the contract). Update `README.md`'s "What this is" section to mention diff/pass-fail signal as part of the runner's job.

- [ ] **Step 1: Update operation-contract.md**

Find the section listing required Phase 1 operations. Add a new top-level section after it:

```markdown
## Runner-only operations

These are exposed by `vrm-runner` but not by renderer adapters. They orchestrate adapter calls and produce derived artifacts.

### `diff`

```json
{
  "input": {
    "plan": "string (path to test plan YAML)",
    "render": "string (path to render PNG)",
    "reference": "string (path to reference PNG)",
    "renderer_name": "string"
  },
  "output": {
    "test_id": "string",
    "renderer": "string",
    "reference_renderer": "string",
    "ssim": "number",
    "ssim_threshold": "number",
    "ssim_passed": "boolean",
    "properties": "array of PropertyResult"
  }
}
```

`diff` runs SSIM between `render` and `reference`, then evaluates each property assertion in the plan against the render image. Exits non-zero when `overall_passed` is false; agents and CI use the exit code as the pass/fail signal.

`execute-test-plan` accepts an optional `--reference` flag that runs `diff` inline after the render and includes the `DiffResult` in its JSON output.
```

- [ ] **Step 2: Update README.md**

Find the bullet list under "What this is" describing the runner. Replace:

> An **agent-first conformance runner** that drives every supported renderer through the same test plan via a uniform operation catalog (structured CLI with `--json` mode + thin MCP wrapper, both backed by one core).

with:

> An **agent-first conformance runner** that drives every supported renderer through the same test plan via a uniform operation catalog, then diffs the result (SSIM + property assertions) against a reference PNG to produce a pass/fail signal — structured CLI with `--json` mode + thin MCP wrapper, both backed by one core.

- [ ] **Step 3: Commit**

```bash
git add docs/operation-contract.md README.md
git commit -m "docs: document runner diff operation in contract + README"
```

---

## Self-Review

**Spec coverage:**

| Phase 2A goal | Task |
|---|---|
| Type bridge `vrm_test_plan` ↔ `vrm_diff_engine` | A1 |
| Diff orchestrator (`diff_one`) | B1 |
| Standalone `vrm-runner diff` subcommand | C1 |
| Inline diff via `execute-test-plan --reference` | C2 |
| End-to-end smoke exercises the diff loop | D1 |
| Contract + README docs | D2 |

**Placeholder scan:** none. All code blocks contain complete content; tests verify behavior, not just structure.

**Type consistency:**

- `diff_one(plan, render, reference, renderer)` signature is consistent across B1, C1, C2.
- `ExecuteOptions::reference: Option<Utf8PathBuf>` and `ExecuteResult::diff: Option<DiffResult>` are introduced together in C2 and the CLI handler propagates both.
- After A1, `vrm_test_plan::PropertyAssertion` and `vrm_diff_engine::property::PropertyAssertion` are the SAME type — call sites in B1 import via either path.
- `DiffMode::Ssim` (from existing `vrm_test_plan::Diff`) is implicitly assumed in `diff_one`. The plan's threshold + reference_renderer fields drive behavior; consensus mode (`DiffMode::Consensus`) is out of scope for v0.2 and would need a separate code path.

**YAGNI guards:**

- ✅ No new external crates.
- ✅ No file output beyond stdout for the standalone `diff` subcommand.
- ✅ No S3 fetch (reference must be local).
- ✅ No consensus mode.
- ✅ The `diff` subcommand is an addition; existing `execute-test-plan` behavior is preserved when `--reference` is absent.

**Risk register:**

- **A1 YAML drift.** If the two `BboxRegion` enums had a serde rename mismatch we missed, existing test plans on disk would fail to parse after the swap. Mitigation: A1 Step 1 explicitly compares them and Step 4 runs the existing round-trip test. Stop and align if anything fails.
- **PNG decode performance.** `image::open` on a 1024×1024 PNG is ~5-10 ms; for a single-test diff this is fine. Property-heavy plans (Phase 2 spring bone scenarios) may want a "decode once, eval many" path; defer.
- **Exit-code semantics.** `diff` exits non-zero on failure; `execute-test-plan --reference` does NOT change its exit code based on diff result (it returns 0 on render success even if diff failed). This is intentional: the JSON includes `overall_passed` for callers that care; the CLI's job is "did the pipeline run." Document if this surprises anyone.

---

## Execution Handoff

Plan saved to `docs/superpowers/plans/2026-05-10-phase2a-runner-diff-loop.md`. Two execution options:

1. **Subagent-Driven (recommended)** — fresh subagent per task, two-stage review, ~6 tasks total. Should land in one session.
2. **Inline Execution** — execute tasks in this session via `superpowers:executing-plans`.

Critical path: A1 → B1 → C1 → C2 → D1 → D2 (no parallelization here; each builds on the prior). Total estimated tasks: 6.
