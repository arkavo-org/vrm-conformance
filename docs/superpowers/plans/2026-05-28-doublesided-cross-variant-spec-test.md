# doubleSided Cross-Variant Spec Test Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the `material.doubleSided` back-face-culling spec requirement observable, by emitting an open-quad asset pair (false/true) viewed from behind and asserting their renders diverge via a cross-variant SSIM.

**Architecture:** A new generator emit path puts the existing `quad()` fixture on its own (no morphs) with a camera on the −Z side so the quad's back face is in frame. The two variants differ only in `double_sided`. A new `cross_variant` block on the test plan (on the `false` variant only) declares "this render and the sibling's MUST differ below `max_ssim`". A new `cross_variant_diff` in the diff engine performs the inverted SSIM assertion, surfaced through a new `cross-variant-diff` runner subcommand. UniVRM is the reference golden; the mock renderer is not involved.

**Tech Stack:** Rust (workspace 1.88), `serde`/`serde_yml`, `image` + `image-compare` (SSIM), `clap` (CLI), `camino` paths.

**Reference spec:** `docs/superpowers/specs/2026-05-28-doublesided-cross-variant-spec-test-design.md`

---

### Task 1: Quad-only emit path (`emit_vrm_doublesided_quad`)

**Files:**
- Modify: `crates/vrm-asset-generator/src/emit.rs` (add function near `emit_vrm_with_lookat_type`, ~line 328; add test in the existing `#[cfg(test)]` block that already uses `tempdir`/`extract_json_chunk`)

- [ ] **Step 1: Write the failing test**

Add to the test module in `emit.rs` that already imports `tempfile::tempdir` and `camino::Utf8Path` (the `collider_emit_tests` module at ~line 1786, or add a new `#[cfg(test)] mod doublesided_quad_tests` at end of file):

```rust
#[cfg(test)]
mod doublesided_quad_tests {
    use super::*;
    use crate::params::MToonParams;
    use camino::Utf8Path;
    use tempfile::tempdir;

    #[test]
    fn doublesided_quad_emit_has_quad_geom_no_morphs_and_double_sided_flag() {
        let mut params = MToonParams::defaults("ds_quad_test");
        params.double_sided = true;
        let tmp = tempdir().unwrap();
        let vrm_path = Utf8Path::from_path(tmp.path()).unwrap().join("out.vrm");
        emit_vrm_doublesided_quad(&params, &vrm_path).unwrap();

        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();

        // No morph targets on the quad primitive (a confounder we deliberately drop).
        let prim = &doc["meshes"][0]["primitives"][0];
        assert!(
            prim.get("targets").is_none(),
            "quad primitive must carry no morph targets"
        );
        // Material carries the doubleSided flag verbatim.
        assert_eq!(doc["materials"][0]["doubleSided"], serde_json::json!(true));
        // Quad geometry: accessor 0 = POSITION (4 verts), accessor 3 = indices (6).
        assert_eq!(doc["accessors"][0]["count"], serde_json::json!(4));
        assert_eq!(doc["accessors"][3]["count"], serde_json::json!(6));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-asset-generator doublesided_quad_emit_has_quad_geom -- --nocapture`
Expected: FAIL — `cannot find function emit_vrm_doublesided_quad in this scope`.

- [ ] **Step 3: Write minimal implementation**

Add to `emit.rs` after `emit_vrm_with_lookat_type` (it is the closest template: `pack_mesh` + no morph targets). `pack_mesh`, `vrmc_vrm`, `base_material`, `minimal_skeleton`, `write_glb`, `GlbDocument` are already imported at the top of the file:

```rust
/// Emit a `.vrm` GLB carrying a single open quad (no morphs) as the only
/// renderable, for the doubleSided back-face-culling spec test.
///
/// Unlike `emit_vrm` (closed sphere + viseme morphs), this is an open
/// single-quad surface whose front face points +Z. Paired with a camera on
/// the −Z side (see `build_doublesided_quad_test_plan`), the quad's BACK face
/// is in frame, so back-face culling becomes observable: `doubleSided=false`
/// culls it (all-background frame), `doubleSided=true` renders it. The minimal
/// humanoid skeleton is retained only to satisfy VRMC_vrm validation; the rest
/// pose is pure translation, so the quad's +Z normal survives into world space.
pub fn emit_vrm_doublesided_quad(params: &MToonParams, output: &Utf8Path) -> Result<()> {
    let mesh = crate::mesh::quad(0.3);
    let packed = pack_mesh(&mesh);

    let skeleton = minimal_skeleton();
    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();
    let head_node = skeleton.bone_to_node["head"];

    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_quad", params.id),
        "mesh": 0
    }));
    let head = &mut nodes[head_node];
    let mut head_children = head["children"].as_array().cloned().unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": ["KHR_materials_unlit", "VRMC_vrm", "VRMC_materials_mtoon"],
        "extensionsRequired": ["VRMC_vrm"],
        "scene": 0,
        "scenes": [
            { "nodes": [skeleton.root_node] }
        ],
        "nodes": nodes,
        "meshes": [
            {
                "name": format!("{}_geom", params.id),
                "primitives": [
                    {
                        "attributes": {
                            "POSITION": 0,
                            "NORMAL": 1,
                            "TEXCOORD_0": 2
                        },
                        "indices": 3,
                        "material": 0,
                        "mode": 4
                    }
                ]
            }
        ],
        "materials": [base_material(params)],
        "extensions": {
            "VRMC_vrm": vrmc_vrm(&params.id, &skeleton.bone_to_node, mesh_node_index)
        }
    });

    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, glb)?;
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vrm-asset-generator doublesided_quad_emit_has_quad_geom`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/emit.rs
git commit -m "feat(generator): emit_vrm_doublesided_quad — open quad, no morphs, doubleSided flag"
```

---

### Task 2: `cross_variant` test-plan schema

**Files:**
- Modify: `crates/vrm-test-plan/src/lib.rs` (add field to `TestPlan` after `render_sequence` at line 31; add `CrossVariantAssertion` struct after the `TestPlan` struct; add `cross_variant: None` to `make_minimal_plan()` in the test module; add a serde test)
- Modify (compile fixups — each has a `TestPlan { ... }` literal that needs `cross_variant: None`): `crates/vrm-asset-generator/src/sidecar.rs` (`build_default_test_plan`, ~line 95), `crates/vrm-test-plan/tests/roundtrip.rs`, `crates/vrm-runner/tests/diff_integration.rs`, `crates/vrm-runner/tests/execute_test_batch.rs`, `crates/vrm-runner/tests/camera_convention.rs`, `crates/vrm-runner/src/cli.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod` in `crates/vrm-test-plan/src/lib.rs` (the one containing `make_minimal_plan`):

```rust
#[test]
fn cross_variant_round_trips_and_is_omitted_when_absent() {
    // Absent → key not serialized (keeps the existing corpus byte-stable).
    let plan = make_minimal_plan();
    assert!(plan.cross_variant.is_none());
    let yaml = serde_yml::to_string(&plan).unwrap();
    assert!(
        !yaml.contains("cross_variant"),
        "cross_variant must be omitted when None"
    );

    // Present → round-trips through YAML.
    let mut plan2 = make_minimal_plan();
    plan2.cross_variant = Some(CrossVariantAssertion {
        sibling_id: "sibling_xyz".into(),
        max_ssim: 0.85,
    });
    let yaml2 = serde_yml::to_string(&plan2).unwrap();
    let back: TestPlan = serde_yml::from_str(&yaml2).unwrap();
    let cv = back.cross_variant.expect("cross_variant present after round-trip");
    assert_eq!(cv.sibling_id, "sibling_xyz");
    assert!((cv.max_ssim - 0.85).abs() < 1e-6);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-test-plan cross_variant_round_trips`
Expected: FAIL to compile — `cannot find type CrossVariantAssertion` / no field `cross_variant`.

- [ ] **Step 3: Write minimal implementation**

In `crates/vrm-test-plan/src/lib.rs`, add the field to `TestPlan` immediately after the `render_sequence` field (line 31):

```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_sequence: Option<RenderSequenceBlock>,
    /// Cross-variant SSIM assertion. When present, this test's render and the
    /// render of `sibling_id` (SAME renderer) MUST visibly differ. See
    /// `CrossVariantAssertion`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cross_variant: Option<CrossVariantAssertion>,
}
```

(Delete the old closing `}` of `TestPlan` that followed `render_sequence` — the field is inserted before it.)

Then add the struct immediately after the `TestPlan` struct's closing brace:

```rust
/// Cross-variant SSIM assertion: the render of THIS test and the render of
/// `sibling_id` (same renderer) MUST visibly differ. Pass iff their SSIM is
/// at or below `max_ssim`. Used by the doubleSided back-face-culling spec
/// test, where doubleSided=false culls the surface (all-background frame) and
/// doubleSided=true renders it — a conformant renderer's two outputs diverge;
/// a name-heuristic renderer's do not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossVariantAssertion {
    /// Test id (and asset stem) of the sibling variant to compare against.
    pub sibling_id: String,
    /// Pass iff `ssim(this_render, sibling_render) <= max_ssim`.
    pub max_ssim: f32,
}
```

In the same file's test module, add `cross_variant: None,` to the `make_minimal_plan()` literal (after `render_sequence: None,`).

- [ ] **Step 4: Fix the other `TestPlan` literals**

Add `cross_variant: None,` to each `TestPlan { ... }` struct literal in these files (after the `render_sequence: ...` line):
- `crates/vrm-asset-generator/src/sidecar.rs` — `build_default_test_plan` (the literal ending at ~line 96, after `render_sequence: None,`).
- `crates/vrm-test-plan/tests/roundtrip.rs`
- `crates/vrm-runner/tests/diff_integration.rs`
- `crates/vrm-runner/tests/execute_test_batch.rs`
- `crates/vrm-runner/tests/camera_convention.rs`
- `crates/vrm-runner/src/cli.rs`

Then run `cargo build --workspace --tests` and add the field to any remaining literal the compiler flags (error: `missing field cross_variant`).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vrm-test-plan cross_variant_round_trips && cargo build --workspace --tests`
Expected: test PASS; workspace builds clean.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-test-plan/src/lib.rs crates/vrm-asset-generator/src/sidecar.rs crates/vrm-runner
git commit -m "feat(test-plan): cross_variant SSIM assertion on TestPlan (serde-default, omitted when absent)"
```

---

### Task 3: doubleSided quad test-plan builder + sidecar emit

**Files:**
- Modify: `crates/vrm-asset-generator/src/sidecar.rs` (add `build_doublesided_quad_test_plan`; add `CrossVariantAssertion` to the `use vrm_test_plan::{...}` import; add test)
- Modify: `crates/vrm-asset-generator/src/emit.rs` (add `emit_with_sidecars_doublesided_quad`)

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod` in `sidecar.rs` (e.g. `extended_plan_tests`, which already `use super::*` and `MToonParams`):

```rust
#[test]
fn doublesided_quad_plan_has_back_camera_and_cross_variant_on_false_only() {
    let p = MToonParams::defaults("doublesided_quad_false");
    let plan =
        build_doublesided_quad_test_plan(&p, "doublesided_quad_false.vrm", Some("doublesided_quad_true"));
    // Camera sits behind the quad (−Z side) so the back face is in frame.
    assert!(
        plan.camera.position[2] < 0.0,
        "camera must be on the -Z side, got {:?}",
        plan.camera.position
    );
    let cv = plan
        .cross_variant
        .as_ref()
        .expect("false variant carries cross_variant");
    assert_eq!(cv.sibling_id, "doublesided_quad_true");
    assert!((cv.max_ssim - 0.85).abs() < 1e-6);
    assert!(plan.validate().is_ok());

    // True variant carries no cross_variant (single, non-redundant declaration).
    let p_true = MToonParams::defaults("doublesided_quad_true");
    let plan_true =
        build_doublesided_quad_test_plan(&p_true, "doublesided_quad_true.vrm", None);
    assert!(plan_true.cross_variant.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-asset-generator doublesided_quad_plan_has_back_camera`
Expected: FAIL to compile — `cannot find function build_doublesided_quad_test_plan`.

- [ ] **Step 3: Write minimal implementation**

In `sidecar.rs`, add `CrossVariantAssertion` to the existing `use vrm_test_plan::{...}` import list (top of file). Then add the builder (after `build_default_test_plan`):

```rust
/// Build the test plan for one doubleSided back-face-culling spec-test variant.
///
/// Camera sits on the −Z side looking toward +Z, so it views the quad's BACK
/// face (the quad's front normal is +Z). Camera-behind is deliberate over
/// rotating the quad 180° — it avoids confounding with VMK's documented
/// 180°-flip bug (VMK#299). When `cross_variant_sibling` is set (the `false`
/// variant), attaches a CrossVariantAssertion requiring the two renders to
/// diverge at or below SSIM 0.85.
pub fn build_doublesided_quad_test_plan(
    params: &MToonParams,
    asset_relpath: &str,
    cross_variant_sibling: Option<&str>,
) -> TestPlan {
    let mut plan = build_default_test_plan(params, asset_relpath);
    plan.spec_section = "glTF material.doubleSided (back-face culling)".into();
    plan.camera = Camera {
        position: [0.0, 1.36, -1.5],
        target: [0.0, 1.36, 0.0],
        up: [0.0, 1.0, 0.0],
        fov_degrees: 30.0,
    };
    if let Some(sibling) = cross_variant_sibling {
        plan.cross_variant = Some(CrossVariantAssertion {
            sibling_id: sibling.to_string(),
            max_ssim: 0.85,
        });
    }
    plan
}
```

In `emit.rs`, add (near the other `emit_with_sidecars_*` functions, ~line 368):

```rust
/// Emit the doubleSided quad triplet (`.vrm` + `.meta.json` + `.test.yaml`).
/// `cross_variant_sibling` names the opposite variant for the cross-variant
/// SSIM assertion; set it on the `false` variant only.
pub fn emit_with_sidecars_doublesided_quad(
    params: &MToonParams,
    stem: &Utf8Path,
    cross_variant_sibling: Option<&str>,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_doublesided_quad(params, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(params, None, &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = crate::sidecar::build_doublesided_quad_test_plan(
        params,
        &asset_relpath,
        cross_variant_sibling,
    );
    write_test_yaml(&plan, &yaml_path)?;
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vrm-asset-generator doublesided_quad_plan_has_back_camera`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/sidecar.rs crates/vrm-asset-generator/src/emit.rs
git commit -m "feat(generator): doubleSided quad plan builder (back camera + cross_variant) and sidecar emit"
```

---

### Task 4: `cross_variant_diff` in the diff engine

**Files:**
- Create: `crates/vrm-diff-engine/src/cross_variant.rs`
- Modify: `crates/vrm-diff-engine/src/lib.rs` (add `pub mod cross_variant;`)

- [ ] **Step 1: Write the failing test (inside the new module)**

Create `crates/vrm-diff-engine/src/cross_variant.rs` with ONLY the test module first (so it fails to compile against the not-yet-written fn):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    /// Write a 16×16 magenta PNG, optionally with a gray filled square in the
    /// centre (mimicking a rendered quad over the magenta background sentinel).
    fn write_png(dir: &camino::Utf8Path, name: &str, with_center_square: bool) -> camino::Utf8PathBuf {
        let mut img = RgbImage::new(16, 16);
        for px in img.pixels_mut() {
            *px = Rgb([255, 0, 255]);
        }
        if with_center_square {
            for y in 4..12 {
                for x in 4..12 {
                    img.put_pixel(x, y, Rgb([128, 128, 128]));
                }
            }
        }
        let path = dir.join(name);
        img.save(path.as_std_path()).unwrap();
        path
    }

    #[test]
    fn identical_renders_fail_must_differ_assertion() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = camino::Utf8Path::from_path(tmp.path()).unwrap();
        let a = write_png(dir, "a.png", false);
        let b = write_png(dir, "b.png", false);
        let r = cross_variant_diff(&a, &b, 0.85).unwrap();
        assert!(
            !r.passed,
            "identical renders must NOT pass a must-differ assertion (ssim={})",
            r.ssim
        );
    }

    #[test]
    fn divergent_renders_pass_must_differ_assertion() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = camino::Utf8Path::from_path(tmp.path()).unwrap();
        let culled = write_png(dir, "culled.png", false); // all background
        let shown = write_png(dir, "shown.png", true); // quad in frame
        let r = cross_variant_diff(&culled, &shown, 0.85).unwrap();
        assert!(
            r.passed,
            "culled (background) vs shown (quad) must diverge below 0.85 (ssim={})",
            r.ssim
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-diff-engine cross_variant`
Expected: FAIL to compile — `cannot find function cross_variant_diff` (and module not declared).

- [ ] **Step 3: Write minimal implementation**

Prepend the implementation to `crates/vrm-diff-engine/src/cross_variant.rs` (above the test module):

```rust
//! Cross-variant SSIM: assert two renders of the SAME renderer DIFFER.
//!
//! The inverse of the normal conformance diff (which passes when SSIM is
//! high). Used by the doubleSided back-face-culling spec test: the
//! doubleSided=false render (culled → all-background) and the doubleSided=true
//! render (surface shown) of a conformant renderer MUST diverge. Pass iff
//! their SSIM is at or below `max_ssim`.

use crate::ssim::{ssim_pngs, SsimError};
use camino::Utf8Path;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CrossVariantResult {
    pub ssim: f64,
    pub max_ssim: f64,
    pub passed: bool,
}

/// Compare two renders and pass iff they visibly DIFFER (ssim <= max_ssim).
pub fn cross_variant_diff(
    false_png: &Utf8Path,
    true_png: &Utf8Path,
    max_ssim: f64,
) -> Result<CrossVariantResult, SsimError> {
    let ssim = ssim_pngs(false_png, true_png)?;
    Ok(CrossVariantResult {
        ssim,
        max_ssim,
        passed: ssim <= max_ssim,
    })
}
```

Add to `crates/vrm-diff-engine/src/lib.rs` (with the other `pub mod` lines, alphabetical near `consensus`):

```rust
pub mod cross_variant;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vrm-diff-engine cross_variant`
Expected: PASS (both tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-diff-engine/src/cross_variant.rs crates/vrm-diff-engine/src/lib.rs
git commit -m "feat(diff-engine): cross_variant_diff — inverted SSIM assertion (renders must differ)"
```

---

### Task 5: `cross-variant-diff` runner subcommand

**Files:**
- Modify: `crates/vrm-runner/src/cli.rs` (add `Cmd::CrossVariantDiff` variant near `Cmd::Diff` ~line 97; add handler near the `Cmd::Diff` handler ~line 447; add a `describe` catalog entry)

- [ ] **Step 1: Write the failing test**

Add a runner integration test. Create `crates/vrm-runner/tests/cross_variant_diff.rs`:

```rust
//! cross-variant-diff subcommand: passes (exit 0) when the two renders differ,
//! fails (exit 1) when they are identical. max_ssim is read from the plan.

use std::process::Command;

fn write_plan(dir: &std::path::Path) -> std::path::PathBuf {
    // Build the real plan via the generator's public builder, then serialize
    // it — avoids hand-authoring YAML (ConformanceStatus is an internally
    // tagged enum, and several fields are serde-default). vrm-asset-generator
    // is already a dev-dependency of vrm-runner; serde_yml is a regular dep.
    use vrm_asset_generator::params::MToonParams;
    use vrm_asset_generator::sidecar::build_doublesided_quad_test_plan;
    let params = MToonParams::defaults("doublesided_quad_false");
    let plan = build_doublesided_quad_test_plan(
        &params,
        "doublesided_quad_false.vrm",
        Some("doublesided_quad_true"),
    );
    let yaml = serde_yml::to_string(&plan).unwrap();
    let p = dir.join("plan.test.yaml");
    std::fs::write(&p, yaml).unwrap();
    p
}

fn write_png(path: &std::path::Path, with_square: bool) {
    use image::{Rgb, RgbImage};
    let mut img = RgbImage::new(16, 16);
    for px in img.pixels_mut() {
        *px = Rgb([255, 0, 255]);
    }
    if with_square {
        for y in 4..12 {
            for x in 4..12 {
                img.put_pixel(x, y, Rgb([128, 128, 128]));
            }
        }
    }
    img.save(path).unwrap();
}

#[test]
fn cross_variant_diff_passes_when_renders_differ() {
    let tmp = tempfile::tempdir().unwrap();
    let plan = write_plan(tmp.path());
    let f = tmp.path().join("false.png");
    let t = tmp.path().join("true.png");
    write_png(&f, false); // culled → background
    write_png(&t, true); // shown → quad

    let status = Command::new(env!("CARGO_BIN_EXE_vrm-runner"))
        .args([
            "cross-variant-diff",
            "--plan",
            plan.to_str().unwrap(),
            "--render-false",
            f.to_str().unwrap(),
            "--render-true",
            t.to_str().unwrap(),
            "--json",
        ])
        .status()
        .unwrap();
    assert!(status.success(), "differing renders should exit 0");
}

#[test]
fn cross_variant_diff_fails_when_renders_identical() {
    let tmp = tempfile::tempdir().unwrap();
    let plan = write_plan(tmp.path());
    let f = tmp.path().join("false.png");
    let t = tmp.path().join("true.png");
    write_png(&f, false);
    write_png(&t, false); // identical → must-differ assertion fails

    let status = Command::new(env!("CARGO_BIN_EXE_vrm-runner"))
        .args([
            "cross-variant-diff",
            "--plan",
            plan.to_str().unwrap(),
            "--render-false",
            f.to_str().unwrap(),
            "--render-true",
            t.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(!status.success(), "identical renders should exit non-zero");
}
```

No `Cargo.toml` changes are needed: `vrm-runner` already declares `tempfile`, `image`, and `vrm-asset-generator` under `[dev-dependencies]`, and `serde_yml` as a regular dependency (available to integration tests).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-runner --test cross_variant_diff`
Expected: FAIL — the subcommand does not exist (clap errors, non-zero exit / unrecognized subcommand), so `cross_variant_diff_passes_when_renders_differ` fails its `assert!(status.success())`.

- [ ] **Step 3: Write minimal implementation**

In `crates/vrm-runner/src/cli.rs`, add the variant to `enum Cmd` (after `Diff { ... }`, ~line 97):

```rust
    /// Cross-variant SSIM: assert the false-variant and true-variant renders of
    /// the SAME renderer DIFFER. Reads `max_ssim` from the plan's `cross_variant`
    /// block. Exits non-zero when the renders do NOT diverge (ssim > max_ssim).
    /// Used by the doubleSided back-face-culling spec test.
    CrossVariantDiff {
        #[arg(long)]
        plan: Utf8PathBuf,
        #[arg(long)]
        render_false: Utf8PathBuf,
        #[arg(long)]
        render_true: Utf8PathBuf,
        #[arg(long, default_value = "univrm")]
        renderer_name: String,
        #[arg(long)]
        json: bool,
    },
```

Add the handler arm to the `match cmd { ... }` (after the `Cmd::Diff { ... } => { ... }` arm, ~line 447):

```rust
        Cmd::CrossVariantDiff {
            plan,
            render_false,
            render_true,
            renderer_name,
            json: emit_json,
        } => {
            use vrm_diff_engine::cross_variant::cross_variant_diff;
            let plan_value = load_plan(&plan)?;
            let cv = plan_value.cross_variant.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "plan '{}' has no cross_variant block; not a cross-variant spec test",
                    plan_value.id
                )
            })?;
            let result = cross_variant_diff(&render_false, &render_true, cv.max_ssim as f64)?;

            if emit_json {
                #[derive(serde::Serialize)]
                struct CrossVariantEnvelope<'a> {
                    test_id: &'a str,
                    renderer: &'a str,
                    sibling_id: &'a str,
                    cross_variant: vrm_diff_engine::cross_variant::CrossVariantResult,
                }
                let envelope = CrossVariantEnvelope {
                    test_id: &plan_value.id,
                    renderer: &renderer_name,
                    sibling_id: &cv.sibling_id,
                    cross_variant: result.clone(),
                };
                println!("{}", serde_json::to_string(&envelope)?);
            } else {
                println!(
                    "{}: cross-variant SSIM={:.4} (max {:.4}, {}) vs {}",
                    plan_value.id,
                    result.ssim,
                    result.max_ssim,
                    if result.passed { "PASS" } else { "FAIL" },
                    cv.sibling_id,
                );
            }

            if !result.passed {
                std::process::exit(1);
            }
            Ok(())
        }
```

Add a `describe` catalog entry. In the `Cmd::Describe` handler's big `json!({ ... "operations": { ... } })` literal, add a key alongside the existing ones (e.g. after `"consensus-diff"`):

```rust
                    "cross-variant-diff": {
                        "summary": "Assert two renders of the SAME renderer DIFFER (inverted SSIM). Reads max_ssim from the plan's cross_variant block; passes iff ssim(--render-false, --render-true) <= max_ssim. Exits non-zero when the renders do not diverge. Used by the doubleSided back-face-culling spec test."
                    },
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vrm-runner --test cross_variant_diff`
Expected: PASS (both tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-runner/src/cli.rs crates/vrm-runner/tests/cross_variant_diff.rs
git commit -m "feat(runner): cross-variant-diff subcommand (reads max_ssim from plan, exit-gated)"
```

---

### Task 6: `emit-doublesided-spec-test` CLI arm + pair emitter

**Files:**
- Modify: `crates/vrm-asset-generator/src/emit.rs` (add `emit_doublesided_spec_test_pair`; add `Utf8PathBuf` to the `camino` import on line 11; add a test)
- Modify: `crates/vrm-asset-generator/src/cli.rs` (add `Cmd::EmitDoublesidedSpecTest` variant near `EmitMaterialNameClassificationSweep` ~line 381; add handler ~line 1897; add a `describe` catalog entry)

- [ ] **Step 1: Write the failing test**

Add to the `doublesided_quad_tests` module created in Task 1 (`emit.rs`):

```rust
    #[test]
    fn doublesided_spec_test_pair_emits_two_triplets_false_has_cross_variant() {
        let tmp = tempdir().unwrap();
        let dir = Utf8Path::from_path(tmp.path()).unwrap();
        emit_doublesided_spec_test_pair(dir).unwrap();

        for id in ["doublesided_quad_false", "doublesided_quad_true"] {
            assert!(dir.join(format!("{id}.vrm")).exists(), "{id}.vrm missing");
            assert!(dir.join(format!("{id}.test.yaml")).exists(), "{id}.test.yaml missing");
            assert!(dir.join(format!("{id}.meta.json")).exists(), "{id}.meta.json missing");
        }

        // The false plan declares the cross-variant assertion; the true plan does not.
        let false_yaml =
            std::fs::read_to_string(dir.join("doublesided_quad_false.test.yaml")).unwrap();
        assert!(false_yaml.contains("cross_variant"));
        assert!(false_yaml.contains("doublesided_quad_true"));
        let true_yaml =
            std::fs::read_to_string(dir.join("doublesided_quad_true.test.yaml")).unwrap();
        assert!(!true_yaml.contains("cross_variant"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-asset-generator doublesided_spec_test_pair_emits_two_triplets`
Expected: FAIL to compile — `cannot find function emit_doublesided_spec_test_pair`.

- [ ] **Step 3: Write minimal implementation**

In `emit.rs`, change the `camino` import on line 11 from `use camino::Utf8Path;` to `use camino::{Utf8Path, Utf8PathBuf};`. Then add the pair emitter (near `emit_with_sidecars_doublesided_quad`):

```rust
/// Emit the doubleSided back-face-culling spec-test pair: two triplets,
/// `doublesided_quad_false` and `doublesided_quad_true`, identical except for
/// the `double_sided` flag and the `false` variant's cross_variant block.
/// Returns the emitted stems (without extension). UniVRM is the reference golden.
pub fn emit_doublesided_spec_test_pair(output_dir: &Utf8Path) -> Result<Vec<Utf8PathBuf>> {
    std::fs::create_dir_all(output_dir)?;

    let mut false_params = MToonParams::defaults("doublesided_quad_false");
    false_params.double_sided = false;
    let mut true_params = MToonParams::defaults("doublesided_quad_true");
    true_params.double_sided = true;

    let false_stem = output_dir.join("doublesided_quad_false");
    emit_with_sidecars_doublesided_quad(&false_params, &false_stem, Some("doublesided_quad_true"))?;

    let true_stem = output_dir.join("doublesided_quad_true");
    emit_with_sidecars_doublesided_quad(&true_params, &true_stem, None)?;

    Ok(vec![false_stem, true_stem])
}
```

(`MToonParams` is already in scope via `use crate::params::MToonParams;` at the top of `emit.rs`.)

In `cli.rs`, add the variant to `enum Cmd` (after `EmitMaterialNameClassificationSweep`):

```rust
    /// Emit the doubleSided back-face-culling spec-test pair (2 triplets).
    ///
    /// `doublesided_quad_{false,true}`: an open quad viewed from BEHIND (camera
    /// on the −Z side), so back-face culling is observable. doubleSided=false
    /// culls the quad (all-background frame); doubleSided=true renders it. The
    /// false variant's plan carries a cross_variant block requiring the two
    /// renders to diverge (cross-variant SSIM ≤ 0.85). See
    /// docs/superpowers/specs/2026-05-28-doublesided-cross-variant-spec-test-design.md.
    EmitDoublesidedSpecTest {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },
```

Add the handler arm (after the `Cmd::EmitMaterialNameClassificationSweep { ... } => { ... }` arm, ~line 1897), modeled on it:

```rust
        Cmd::EmitDoublesidedSpecTest {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::emit_doublesided_spec_test_pair;
            let stems = emit_doublesided_spec_test_pair(&output_dir)?;
            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": stems.len(),
                    "output_dir": output_dir,
                    "assets": stems
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} doubleSided spec-test assets to {}",
                    stems.len(),
                    output_dir
                );
            }
            Ok(())
        }
```

(Confirm the handler module imports `emit_with_sidecars`/emit functions the way the existing arms do — the `use crate::emit::emit_doublesided_spec_test_pair;` inside the arm keeps it self-contained.)

Add a `describe` catalog entry in `cli.rs`'s `Cmd::Describe` operations literal (alongside `emit-material-name-classification-sweep`):

```rust
                    "emit-doublesided-spec-test": {
                        "summary": "Emit the doubleSided back-face-culling spec-test pair (doublesided_quad_false/true): an open quad viewed from behind so culling is observable. The false variant's plan carries a cross_variant SSIM assertion requiring the two renders to diverge."
                    },
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vrm-asset-generator doublesided_spec_test_pair_emits_two_triplets`
Expected: PASS.

- [ ] **Step 5: Smoke the CLI end-to-end**

Run:
```bash
cargo run -p vrm-asset-generator -- emit-doublesided-spec-test --output-dir /tmp/ds-spec --json
ls /tmp/ds-spec
```
Expected: JSON summary with `"count": 2`; directory contains `doublesided_quad_false.{vrm,meta.json,test.yaml}` and `doublesided_quad_true.{...}`.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/src/emit.rs crates/vrm-asset-generator/src/cli.rs
git commit -m "feat(generator): emit-doublesided-spec-test CLI arm (false/true quad pair)"
```

---

### Task 7: Workspace gates (fmt, clippy, full test)

**Files:** none (verification only)

- [ ] **Step 1: Format**

Run: `cargo fmt --all`
Then: `cargo fmt --all -- --check`
Expected: clean (CI gate).

- [ ] **Step 2: Clippy (hard merge gate — zero warnings)**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings. Fix any introduced (likely none; the new code follows existing idioms).

- [ ] **Step 3: Full workspace test**

Run: `cargo test --workspace`
Expected: all pass, including the new emit, schema, diff-engine, and runner tests.

- [ ] **Step 4: Commit any fmt/clippy fixups**

```bash
git add -A
git commit -m "chore: fmt + clippy clean for doubleSided cross-variant spec test"
```

(Skip this commit if Steps 1–3 produced no changes.)

---

## Notes for the implementer

- **TDD order matters:** Tasks 2 → 3 and 4 → 5 have a dependency (schema before its builder/consumer; diff fn before its subcommand). Follow task order.
- **`serde(default)` + `skip_serializing_if`** on `cross_variant` is what keeps the existing 80-test corpus byte-identical — do not drop either attribute.
- **The mock renderer is intentionally not wired in.** This spec test is exercised through real adapters (UniVRM golden first). Do not add it to `scripts/smoke.sh`.
- **Material-name corollary is out of scope** (deferred per the spec doc) — do not re-point the `material_name_classification_sweep` here.

## Self-review notes (author)

- Spec coverage: §Components 1 (Task 1) · 2 camera (Task 3) · 3 schema (Task 2) · 4 diff engine (Task 4) · 5 runner (Task 5) · 6 CLI arm (Task 6); testing section (each task's tests) + fmt/clippy gate (Task 7). All covered.
- Type consistency: `CrossVariantAssertion { sibling_id, max_ssim }`, `CrossVariantResult { ssim, max_ssim, passed }`, `cross_variant_diff(false_png, true_png, max_ssim)`, `emit_vrm_doublesided_quad`, `build_doublesided_quad_test_plan`, `emit_with_sidecars_doublesided_quad`, `emit_doublesided_spec_test_pair` — names identical across all tasks.
- No placeholders: every code step shows full code; commands have expected output.
