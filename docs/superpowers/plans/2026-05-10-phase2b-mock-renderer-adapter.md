# Phase 2B — Mock Renderer Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a Rust-based mock renderer adapter that satisfies the full Phase 1 op contract over stdio JSON-RPC, producing a deterministic synthetic PNG per asset. This unblocks runner-driven E2E testing, diff-loop regression coverage, and the comparison site's "real golden submission" flow without depending on the still-deferred L3 Swift VRMMetalKit integration or any future external renderer.

**Architecture:** A new `crates/vrm-mock-renderer/` binary that imports `vrm-ops` for JSON-RPC framing + types, parses the loaded `.vrm`'s sibling `.meta.json` sidecar to recover the deterministic `MToonParams`, and synthesizes a small RGB PNG whose pixel values are a stable function of those parameters (so identical params → identical bytes → SSIM 1.0). The mock never executes real lighting math; it's a deterministic fingerprint of the asset's material parameters. Two-tier output: a magenta sentinel background (matches the diff-engine bbox convention), plus a colored rectangle whose RGB encodes shade/base colors and whose intensity encodes shading-shift/toony.

**Tech Stack:** Rust 2021 (1.88), existing workspace crates (`vrm-ops`, `vrm-asset-generator` for the `MToonParams` schema). New deps: `image` (already in workspace), `serde_json` (already), `tokio` for the stdio-server loop (already in workspace). No graphics or GL dependencies — mock means deterministic CPU math.

**Why a mock now:**
- L3 Swift VRMMetalKit integration is deferred; three-vrm and others come later. Today nothing actually answers `load_vrm` other than `Unimplemented`.
- The diff loop, S3 push, site, and runner CLI all need *some* adapter to drive them end-to-end.
- A deterministic mock means SSIM-1.0 self-diff regressions are catchable in CI; a real renderer would have GPU-driver noise.
- Future real adapters can be wired in alongside; the mock stays as the canary for the JSON-RPC contract.

**YAGNI scope guards:**
- The mock does NOT implement Phase 2+ reserved ops (`set_environment`, `step_physics`, etc.) — same Unimplemented stubs as the Swift adapter scaffold.
- No real lighting math, no real shading. The PNG encodes parameters, not geometry-aware rendering.
- No alpha channel; output is RGB only.
- Color-space handling: the mock declares whatever the request asked for (`Linear` or `Srgb`) but pixel bytes are the same — it's a deterministic mock, not a color-managed one.

---

## File Layout

| File | Status | Responsibility |
|---|---|---|
| `crates/vrm-mock-renderer/Cargo.toml` | Create | Workspace deps + bin target. |
| `crates/vrm-mock-renderer/src/main.rs` | Create | Stdio JSON-RPC dispatch loop. Reads framed requests, routes to op handlers, writes framed responses. |
| `crates/vrm-mock-renderer/src/session.rs` | Create | `Session` struct holding loaded `MToonParams`, current camera/lighting/post state. `SessionRegistry` maps session_id to Session. |
| `crates/vrm-mock-renderer/src/render.rs` | Create | `synthesize_png(session, width, height) -> RgbImage`: deterministic param-encoded image. |
| `crates/vrm-mock-renderer/src/handlers.rs` | Create | One function per op (load_vrm, set_camera, etc.) producing a `JsonRpcResponse` from a `JsonRpcRequest`. |
| `crates/vrm-mock-renderer/tests/contract.rs` | Create | Integration test: spawn the mock as a subprocess, feed framed JSON-RPC requests, assert framed responses (echoes the Swift adapter's JsonRpcServerTests). |
| `crates/vrm-mock-renderer/tests/synthesis.rs` | Create | Unit-style test on `synthesize_png`: identical params → identical bytes; different params → different bytes. |
| `Cargo.toml` (workspace) | Modify | Register the new crate in `[workspace] members`. |
| `scripts/smoke.sh` | Modify | Replace the "skip render" L3 placeholder with the mock as the default adapter; the runner now actually completes load → camera → light → post → render → dispose. |

---

## Section A — Crate scaffold + workspace registration

### Task A1: Create the `vrm-mock-renderer` crate skeleton

**Files:**
- Create: `crates/vrm-mock-renderer/Cargo.toml`
- Create: `crates/vrm-mock-renderer/src/main.rs`
- Modify: `Cargo.toml` (workspace `members`)

- [ ] **Step 1: Add member to workspace**

In `Cargo.toml` at the repo root, find the `[workspace] members = [...]` block and add `crates/vrm-mock-renderer` to it (keep alphabetical order).

- [ ] **Step 2: Crate manifest**

`crates/vrm-mock-renderer/Cargo.toml`:

```toml
[package]
name = "vrm-mock-renderer"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
anyhow.workspace = true
serde.workspace = true
serde_json.workspace = true
camino.workspace = true
image.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true

vrm-ops = { path = "../vrm-ops" }

[dev-dependencies]
tempfile.workspace = true

[[bin]]
name = "vrm-mock-renderer"
path = "src/main.rs"
```

- [ ] **Step 3: Stub `main.rs`**

`crates/vrm-mock-renderer/src/main.rs`:

```rust
//! vrm-mock-renderer: a deterministic mock renderer adapter that satisfies
//! the Phase 1 JSON-RPC stdio contract for testing the runner + diff +
//! S3 + site pipeline without a real renderer.

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    tracing::info!("vrm-mock-renderer starting");
    Ok(())
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p vrm-mock-renderer`

Expected: clean build; binary at `target/debug/vrm-mock-renderer`.

- [ ] **Step 5: Whole-workspace verification**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

All green.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/vrm-mock-renderer/
git commit -m "chore(mock-renderer): scaffold crate + register in workspace"
```

---

## Section B — Deterministic synthesis (the core)

### Task B1: `synthesize_png` from `MToonParams` (TDD)

**Files:**
- Create: `crates/vrm-mock-renderer/src/render.rs`
- Create: `crates/vrm-mock-renderer/src/lib.rs`
- Create: `crates/vrm-mock-renderer/tests/synthesis.rs`

The mock's signature trick: **identical params produce byte-identical PNGs**. That makes the diff loop's SSIM-1.0 self-test trivially exercisable. The encoding scheme:

- Background: magenta `[255, 0, 255]` everywhere (matches diff-engine sentinel).
- Avatar bbox: a centered rectangle covering 50% width × 50% height.
- Avatar fill RGB: `base_color_factor` clamped to `[0, 255]` u8, modulated by `(shading_shift_factor + 1.0) / 2.0` brightness gain.
- Avatar top stripe (8 pixels): `shade_color_factor` (modulated by `shading_toony_factor`).
- Avatar right stripe (8 pixels): `parametric_rim_color_factor` (modulated by `rim_lighting_mix_factor`).
- Avatar outline: 1-pixel ring around the bbox painted `outline_color_factor`, drawn only when `outline_width_mode != None`.

This is a parameter fingerprint, not a rendering. Different params → different pixels → different SSIM.

For the schema: we read `MToonParams` from the asset's sidecar `<asset>.meta.json` file (which `vrm-asset-generator` emits alongside every `.vrm`). The mock therefore depends on the meta sidecar being present; if it's missing, load_vrm fails with `LoadFailed`.

- [ ] **Step 1: Add `vrm-asset-generator` as a dep**

We need `MToonParams` and `OutlineWidthMode`. Modify `crates/vrm-mock-renderer/Cargo.toml`:

```toml
[dependencies]
anyhow.workspace = true
serde.workspace = true
serde_json.workspace = true
camino.workspace = true
image.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true

vrm-ops = { path = "../vrm-ops" }
vrm-asset-generator = { path = "../vrm-asset-generator" }
```

- [ ] **Step 2: Failing test**

`crates/vrm-mock-renderer/tests/synthesis.rs`:

```rust
use vrm_asset_generator::params::{MToonParams, OutlineWidthMode};
use vrm_mock_renderer::render::synthesize_png;

#[test]
fn identical_params_produce_identical_pixels() {
    let p = MToonParams::defaults("a");
    let a = synthesize_png(&p, 64, 64);
    let b = synthesize_png(&p, 64, 64);
    assert_eq!(
        a.as_raw(),
        b.as_raw(),
        "identical params must yield byte-identical pixels"
    );
}

#[test]
fn different_base_color_changes_avatar_pixels() {
    let mut a = MToonParams::defaults("a");
    a.base_color_factor = [1.0, 0.0, 0.0, 1.0];
    let mut b = MToonParams::defaults("b");
    b.base_color_factor = [0.0, 1.0, 0.0, 1.0];

    let img_a = synthesize_png(&a, 64, 64);
    let img_b = synthesize_png(&b, 64, 64);
    assert_ne!(
        img_a.as_raw(),
        img_b.as_raw(),
        "different base_color should produce different pixels"
    );
}

#[test]
fn background_is_magenta_sentinel() {
    let p = MToonParams::defaults("sentinel");
    let img = synthesize_png(&p, 64, 64);
    // Top-left corner is outside the centered avatar bbox.
    let corner = img.get_pixel(0, 0);
    assert_eq!(
        corner.0,
        [255, 0, 255],
        "background must be magenta sentinel for diff-engine bbox detection"
    );
}

#[test]
fn avatar_bbox_is_centered_50_percent() {
    let p = MToonParams::defaults("bbox");
    let img = synthesize_png(&p, 64, 64);
    // Image center should be inside the avatar (not magenta).
    let center = img.get_pixel(32, 32);
    assert_ne!(
        center.0,
        [255, 0, 255],
        "image center should be inside the avatar (non-magenta)"
    );
    // Corner should be background.
    let corner = img.get_pixel(2, 2);
    assert_eq!(corner.0, [255, 0, 255]);
}

#[test]
fn outline_mode_none_omits_ring() {
    let mut p = MToonParams::defaults("no_outline");
    p.outline_width_mode = OutlineWidthMode::None;
    p.outline_color_factor = [1.0, 1.0, 1.0]; // would be white if drawn
    let img = synthesize_png(&p, 64, 64);
    // The pixel just inside the bbox edge should NOT be white outline.
    // Avatar bbox runs [16..48] × [16..48] for 64×64 image (50% centered).
    let edge = img.get_pixel(16, 32);
    assert_ne!(
        edge.0,
        [255, 255, 255],
        "outline_width_mode::None must not draw a white ring"
    );
}

#[test]
fn outline_mode_world_draws_ring() {
    let mut p = MToonParams::defaults("with_outline");
    p.outline_width_mode = OutlineWidthMode::WorldCoordinates;
    p.outline_color_factor = [1.0, 1.0, 1.0];
    let img = synthesize_png(&p, 64, 64);
    let edge = img.get_pixel(16, 32);
    assert_eq!(
        edge.0,
        [255, 255, 255],
        "outline_width_mode::WorldCoordinates must draw a white ring"
    );
}
```

- [ ] **Step 3: Create `lib.rs` exposing the module**

`crates/vrm-mock-renderer/src/lib.rs`:

```rust
//! vrm-mock-renderer library entry; the binary in `src/main.rs` wires the
//! stdio JSON-RPC loop on top of this.

pub mod render;
```

- [ ] **Step 4: Wire crate as both lib + bin**

Modify `crates/vrm-mock-renderer/Cargo.toml` to declare both targets:

```toml
[lib]
path = "src/lib.rs"

[[bin]]
name = "vrm-mock-renderer"
path = "src/main.rs"
```

- [ ] **Step 5: Run failing test**

Run: `cargo test -p vrm-mock-renderer --test synthesis`

Expected: compile error — `render::synthesize_png` doesn't exist.

- [ ] **Step 6: Implement `synthesize_png`**

`crates/vrm-mock-renderer/src/render.rs`:

```rust
//! Deterministic parameter-encoded PNG synthesis. Not a renderer in any
//! meaningful sense — pixel values are a stable, idempotent function of
//! `MToonParams`. Identical params → byte-identical RgbImage.

use image::{Rgb, RgbImage};
use vrm_asset_generator::params::{MToonParams, OutlineWidthMode};

const MAGENTA: [u8; 3] = [255, 0, 255];

/// Synthesize a deterministic mock-rendered avatar fingerprint at the
/// given resolution. The image always has:
///   * Magenta sentinel background outside the avatar bbox.
///   * A centered avatar bbox covering 50% of width × 50% of height.
///   * Fill RGB encoded from `base_color_factor` × shading-shift.
///   * Top stripe encoded from `shade_color_factor` × shading-toony.
///   * Right stripe encoded from `parametric_rim_color_factor` × rim-mix.
///   * 1-pixel outline ring colored by `outline_color_factor` when
///     `outline_width_mode != None`.
pub fn synthesize_png(params: &MToonParams, width: u32, height: u32) -> RgbImage {
    let mut img = RgbImage::from_pixel(width, height, Rgb(MAGENTA));

    let (x0, y0, x1, y1) = avatar_bbox(width, height);

    // Brightness multiplier from shading_shift_factor:
    // shift in [-1.0, 1.0] → brightness in [0.0, 1.0], centered at 0.5.
    let shift_brightness = ((params.shading_shift_factor + 1.0) / 2.0).clamp(0.0, 1.0);
    let fill = scale_color3(rgba_to_rgb(params.base_color_factor), shift_brightness);

    // Shade stripe: shade_color modulated by toony factor.
    let shade = scale_color3(params.shade_color_factor, params.shading_toony_factor.clamp(0.0, 1.0));

    // Rim stripe: rim color modulated by mix factor.
    let rim = scale_color3(
        params.parametric_rim_color_factor,
        params.rim_lighting_mix_factor.clamp(0.0, 1.0),
    );

    // Body fill
    for y in y0..y1 {
        for x in x0..x1 {
            img.put_pixel(x, y, Rgb(fill));
        }
    }

    // Top stripe — 8 pixels high inside the bbox top.
    let top_stripe_end = (y0 + 8).min(y1);
    for y in y0..top_stripe_end {
        for x in x0..x1 {
            img.put_pixel(x, y, Rgb(shade));
        }
    }

    // Right stripe — 8 pixels wide inside the bbox right edge.
    let right_stripe_start = (x1.saturating_sub(8)).max(x0);
    for y in y0..y1 {
        for x in right_stripe_start..x1 {
            img.put_pixel(x, y, Rgb(rim));
        }
    }

    // Outline ring (when enabled).
    if !matches!(params.outline_width_mode, OutlineWidthMode::None) {
        let outline = float3_to_u8(params.outline_color_factor);
        for x in x0..x1 {
            img.put_pixel(x, y0, Rgb(outline));
            img.put_pixel(x, y1.saturating_sub(1), Rgb(outline));
        }
        for y in y0..y1 {
            img.put_pixel(x0, y, Rgb(outline));
            img.put_pixel(x1.saturating_sub(1), y, Rgb(outline));
        }
    }

    img
}

fn avatar_bbox(width: u32, height: u32) -> (u32, u32, u32, u32) {
    let x0 = width / 4;
    let y0 = height / 4;
    let x1 = (width * 3) / 4;
    let y1 = (height * 3) / 4;
    (x0, y0, x1, y1)
}

fn rgba_to_rgb(rgba: [f32; 4]) -> [f32; 3] {
    [rgba[0], rgba[1], rgba[2]]
}

fn scale_color3(rgb: [f32; 3], scale: f32) -> [u8; 3] {
    let s = scale.clamp(0.0, 1.0);
    [
        f_to_u8(rgb[0] * s),
        f_to_u8(rgb[1] * s),
        f_to_u8(rgb[2] * s),
    ]
}

fn float3_to_u8(rgb: [f32; 3]) -> [u8; 3] {
    [f_to_u8(rgb[0]), f_to_u8(rgb[1]), f_to_u8(rgb[2])]
}

fn f_to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}
```

- [ ] **Step 7: Run tests**

Run: `cargo test -p vrm-mock-renderer --test synthesis`

Expected: all 6 tests pass.

If `outline_mode_none_omits_ring` fails because the default `MToonParams::defaults()` returns something other than `OutlineWidthMode::None`, check `crates/vrm-asset-generator/src/params.rs::defaults` and adjust the test or the default. The test assumes default = `None`, which is what F1 specified.

- [ ] **Step 8: Workspace clean**

```bash
cargo clippy -p vrm-mock-renderer --all-targets -- -D warnings
cargo fmt --all -- --check
```

Both clean.

- [ ] **Step 9: Commit**

```bash
git add crates/vrm-mock-renderer/Cargo.toml crates/vrm-mock-renderer/src/lib.rs crates/vrm-mock-renderer/src/render.rs crates/vrm-mock-renderer/tests/synthesis.rs Cargo.lock
git commit -m "feat(mock-renderer): deterministic synthesize_png from MToonParams"
```

---

## Section C — Stdio JSON-RPC dispatch

### Task C1: `Session` + `SessionRegistry` types

**Files:**
- Create: `crates/vrm-mock-renderer/src/session.rs`
- Modify: `crates/vrm-mock-renderer/src/lib.rs`

Holds the deserialized `MToonParams` (loaded from the sidecar meta.json), plus mock state for camera/lighting/post that handlers can write through. A `SessionRegistry` maps session_id → Session for the lifetime of the adapter process.

- [ ] **Step 1: Implement**

`crates/vrm-mock-renderer/src/session.rs`:

```rust
//! Per-load_vrm session state. `Session` holds the parameters extracted
//! from the asset's `.meta.json` sidecar plus the most-recent camera /
//! lighting / post values the runner has set on it. `SessionRegistry`
//! owns sessions for the lifetime of the adapter process.

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::HashMap;
use vrm_asset_generator::params::MToonParams;

#[derive(Debug, Clone)]
pub struct Session {
    pub asset_path: Utf8PathBuf,
    pub params: MToonParams,
    pub camera: Option<vrm_ops::tools::SetCameraParams>,
    pub lighting: Option<vrm_ops::tools::SetLightingParams>,
    pub post_processing: Option<vrm_ops::tools::SetPostProcessingParams>,
}

impl Session {
    pub fn load(asset_path: &Utf8Path) -> Result<Self> {
        let meta_path = asset_path.with_extension("meta.json");
        let meta_bytes = std::fs::read(meta_path.as_std_path())
            .with_context(|| format!("read meta sidecar: {meta_path}"))?;
        // The sidecar shape is `{"id":..., "params": MToonParams, ...}`.
        // We only need params; pull it out with a small envelope.
        #[derive(serde::Deserialize)]
        struct Sidecar {
            params: MToonParams,
        }
        let sidecar: Sidecar = serde_json::from_slice(&meta_bytes)
            .with_context(|| format!("parse meta sidecar: {meta_path}"))?;
        Ok(Self {
            asset_path: asset_path.to_owned(),
            params: sidecar.params,
            camera: None,
            lighting: None,
            post_processing: None,
        })
    }
}

#[derive(Debug, Default)]
pub struct SessionRegistry {
    sessions: HashMap<String, Session>,
    next_id: u64,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, session: Session) -> String {
        self.next_id += 1;
        let id = format!("mock-{}", self.next_id);
        self.sessions.insert(id.clone(), session);
        id
    }

    pub fn get(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Session> {
        self.sessions.get_mut(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<Session> {
        self.sessions.remove(id)
    }
}
```

- [ ] **Step 2: Wire into `lib.rs`**

Modify `crates/vrm-mock-renderer/src/lib.rs`:

```rust
//! vrm-mock-renderer library entry; the binary in `src/main.rs` wires the
//! stdio JSON-RPC loop on top of this.

pub mod render;
pub mod session;
```

- [ ] **Step 3: Compile**

Run: `cargo build -p vrm-mock-renderer`

Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/vrm-mock-renderer/src/lib.rs crates/vrm-mock-renderer/src/session.rs
git commit -m "feat(mock-renderer): Session + SessionRegistry types"
```

---

### Task C2: Op handlers (load_vrm, set_*, render, dispose, reserved stubs)

**Files:**
- Create: `crates/vrm-mock-renderer/src/handlers.rs`
- Modify: `crates/vrm-mock-renderer/src/lib.rs`

Each handler takes a `&mut SessionRegistry` and the deserialized op-specific params, returns either a typed result or an `RpcError`. The dispatch table lives in `main.rs` (Task C3).

The `render` handler is where it all comes together: look up the session, synthesize the PNG from the session's stored params, write to `output_path`, return `RenderResult`.

- [ ] **Step 1: Implement**

`crates/vrm-mock-renderer/src/handlers.rs`:

```rust
//! Per-op handlers. Each function returns either a typed result that the
//! dispatch loop will wrap in a successful JsonRpcResponse, or an RpcError
//! that becomes the response's `error` field.

use crate::render::synthesize_png;
use crate::session::{Session, SessionRegistry};
use camino::Utf8Path;
use vrm_ops::tools as ops;
use vrm_ops::RpcError;

pub fn load_vrm(
    registry: &mut SessionRegistry,
    params: ops::LoadVrmParams,
) -> Result<ops::LoadVrmResult, RpcError> {
    let path = Utf8Path::new(&params.path);
    let session = Session::load(path).map_err(|e| {
        RpcError::load_failed(format!("{path}: {e}"))
    })?;
    let session_id = registry.insert(session);
    Ok(ops::LoadVrmResult { session_id })
}

pub fn set_camera(
    registry: &mut SessionRegistry,
    params: ops::SetCameraParams,
) -> Result<ops::UnitResult, RpcError> {
    let session = registry
        .get_mut(&params.session_id)
        .ok_or_else(|| invalid_session(&params.session_id))?;
    session.camera = Some(params);
    Ok(ops::UnitResult {})
}

pub fn set_lighting(
    registry: &mut SessionRegistry,
    params: ops::SetLightingParams,
) -> Result<ops::UnitResult, RpcError> {
    let session = registry
        .get_mut(&params.session_id)
        .ok_or_else(|| invalid_session(&params.session_id))?;
    session.lighting = Some(params);
    Ok(ops::UnitResult {})
}

pub fn set_post_processing(
    registry: &mut SessionRegistry,
    params: ops::SetPostProcessingParams,
) -> Result<ops::UnitResult, RpcError> {
    let session = registry
        .get_mut(&params.session_id)
        .ok_or_else(|| invalid_session(&params.session_id))?;
    session.post_processing = Some(params);
    Ok(ops::UnitResult {})
}

pub fn render(
    registry: &mut SessionRegistry,
    params: ops::RenderParams,
) -> Result<ops::RenderResult, RpcError> {
    let session = registry
        .get(&params.session_id)
        .ok_or_else(|| invalid_session(&params.session_id))?;
    let img = synthesize_png(&session.params, params.width, params.height);
    if let Some(parent) = std::path::Path::new(&params.output_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                RpcError::render_failed(format!("create output dir: {e}"))
            })?;
        }
    }
    img.save(&params.output_path).map_err(|e| {
        RpcError::render_failed(format!("save png {}: {e}", params.output_path))
    })?;
    // Mock declares whatever the caller asked for; pixel bytes are the same.
    Ok(ops::RenderResult {
        output_path: params.output_path,
        actual_color_space: params.color_space,
    })
}

pub fn dispose(
    registry: &mut SessionRegistry,
    params: ops::DisposeParams,
) -> Result<ops::UnitResult, RpcError> {
    registry.remove(&params.session_id);
    Ok(ops::UnitResult {})
}

/// Reserved ops all return Unimplemented at the dispatch site. This helper
/// produces the canonical envelope so phase labels stay consistent.
pub fn unimplemented(method: &str, phase: &str) -> RpcError {
    RpcError::unimplemented(method, phase)
}

fn invalid_session(id: &str) -> RpcError {
    RpcError {
        code: -32602,
        message: format!("invalid session_id: {id}"),
        data: None,
    }
}
```

- [ ] **Step 2: Wire into `lib.rs`**

Modify `crates/vrm-mock-renderer/src/lib.rs`:

```rust
//! vrm-mock-renderer library entry; the binary in `src/main.rs` wires the
//! stdio JSON-RPC loop on top of this.

pub mod handlers;
pub mod render;
pub mod session;
```

- [ ] **Step 3: Compile**

Run: `cargo build -p vrm-mock-renderer`

Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/vrm-mock-renderer/src/lib.rs crates/vrm-mock-renderer/src/handlers.rs
git commit -m "feat(mock-renderer): per-op handlers (load_vrm, set_*, render, dispose)"
```

---

### Task C3: Stdio dispatch loop in `main.rs`

**Files:**
- Modify: `crates/vrm-mock-renderer/src/main.rs`

Read framed JSON-RPC requests on stdin (using `vrm_ops::stdio::read_message`), inspect the `method` field to choose the right handler, deserialize the typed params, call the handler, frame and write the response. Loop until stdin closes.

Method-to-phase mapping for reserved ops (matches the Swift adapter):
- `set_environment` → phase `v1.x`
- `set_humanoid_pose`, `set_root_transform`, `animate_root_transform`, `step_physics`, `reset_physics` → phase `Phase 2`
- `set_expression` → phase `Phase 3`

Unknown methods → `-32601`. Phase 1 ops route to real handlers. Reserved-but-declared ops return `Unimplemented` with the right phase label.

- [ ] **Step 1: Implement the dispatch loop**

Replace `crates/vrm-mock-renderer/src/main.rs`:

```rust
//! vrm-mock-renderer entrypoint: a stdio JSON-RPC server that satisfies the
//! Phase 1 op contract using `vrm_mock_renderer::handlers`. Reserved Phase
//! 2+ ops return `Unimplemented` with the appropriate phase label.

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::io::{stdin, stdout, BufReader, Write};
use vrm_mock_renderer::{handlers, session::SessionRegistry};
use vrm_ops::stdio::{read_message, write_message};
use vrm_ops::{JsonRpcResponse, RpcError};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    tracing::info!("vrm-mock-renderer starting");

    let mut registry = SessionRegistry::new();
    let stdin = stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = stdout();
    let mut writer = stdout.lock();

    loop {
        let body = match read_message(&mut reader) {
            Ok(b) => b,
            Err(e) => {
                // EOF or framing error → exit cleanly.
                tracing::info!("stdin closed: {e}");
                return Ok(());
            }
        };

        let req: Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("malformed request: {e}");
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": {
                        "code": -32700,
                        "message": format!("parse error: {e}"),
                    }
                });
                write_framed(&mut writer, &resp)?;
                continue;
            }
        };

        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        tracing::debug!(method, id = ?id, "request");

        let result = dispatch(&mut registry, &method, params);

        let resp = match result {
            Ok(v) => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": v,
            }),
            Err(e) => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": e,
            }),
        };
        write_framed(&mut writer, &resp)?;
    }
}

fn dispatch(
    registry: &mut SessionRegistry,
    method: &str,
    params: Value,
) -> Result<Value, RpcError> {
    match method {
        "load_vrm" => json_result(handlers::load_vrm(registry, deser(params)?)),
        "set_camera" => json_result(handlers::set_camera(registry, deser(params)?)),
        "set_lighting" => json_result(handlers::set_lighting(registry, deser(params)?)),
        "set_post_processing" => {
            json_result(handlers::set_post_processing(registry, deser(params)?))
        }
        "render" => json_result(handlers::render(registry, deser(params)?)),
        "dispose" => json_result(handlers::dispose(registry, deser(params)?)),

        // Reserved-but-declared ops: return Unimplemented with the phase
        // the operation belongs to. Matches the Swift adapter's labels.
        "set_environment" => Err(handlers::unimplemented(method, "v1.x")),
        "set_expression" => Err(handlers::unimplemented(method, "Phase 3")),
        "set_humanoid_pose"
        | "set_root_transform"
        | "animate_root_transform"
        | "step_physics"
        | "reset_physics" => Err(handlers::unimplemented(method, "Phase 2")),

        _ => Err(RpcError {
            code: -32601,
            message: format!("method not found: {method}"),
            data: None,
        }),
    }
}

fn deser<P: DeserializeOwned>(v: Value) -> Result<P, RpcError> {
    serde_json::from_value(v).map_err(|e| RpcError {
        code: -32602,
        message: format!("invalid params: {e}"),
        data: None,
    })
}

fn json_result<R: serde::Serialize>(r: Result<R, RpcError>) -> Result<Value, RpcError> {
    r.and_then(|v| {
        serde_json::to_value(v).map_err(|e| RpcError {
            code: -32603,
            message: format!("serialize result: {e}"),
            data: None,
        })
    })
}

fn write_framed<W: Write>(w: &mut W, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    write_message(w, &body)?;
    Ok(())
}
```

> **Note:** `_ = JsonRpcResponse::<()>::ok(0, ());` (suppressing the unused `JsonRpcResponse` import) — but we don't need to import it since we hand-roll the JSON envelope. If `cargo clippy` complains about unused imports, remove `JsonRpcResponse` from the `use vrm_ops::{...}` line.

- [ ] **Step 2: Build + clippy**

```bash
cargo build -p vrm-mock-renderer
cargo clippy -p vrm-mock-renderer --all-targets -- -D warnings
```

If clippy flags an unused `JsonRpcResponse` import, remove it from the `use vrm_ops::` line. Re-run.

- [ ] **Step 3: Commit**

```bash
git add crates/vrm-mock-renderer/src/main.rs
git commit -m "feat(mock-renderer): stdio JSON-RPC dispatch loop with full op routing"
```

---

## Section D — Contract integration test

### Task D1: Subprocess-based contract test

**Files:**
- Create: `crates/vrm-mock-renderer/tests/contract.rs`

Spawn `target/debug/vrm-mock-renderer` as a subprocess, write framed JSON-RPC requests to its stdin, read framed responses from stdout, assert the right responses come back. This is the runner's integration POV: if this test passes, the runner can drive the mock.

We can borrow the `Adapter` client from `vrm-runner::adapter` so we're not re-implementing framing on the test side — but that creates a dev-dep on vrm-runner. Cleaner: use `vrm_ops::stdio::{read_message, write_message}` directly. The test mirrors the Swift adapter's `JsonRpcServerTests` in spirit.

- [ ] **Step 1: Implement test**

`crates/vrm-mock-renderer/tests/contract.rs`:

```rust
//! Subprocess-based contract test: spawn the mock binary, feed framed
//! JSON-RPC requests, assert framed responses. Mirrors the runner's
//! production usage of the adapter.

use std::io::{BufReader, Write};
use std::process::{Command, Stdio};
use vrm_ops::stdio::{read_message, write_message};

fn spawn_mock() -> std::process::Child {
    let bin = env!("CARGO_BIN_EXE_vrm-mock-renderer");
    Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mock")
}

fn rpc(
    stdin: &mut std::process::ChildStdin,
    stdout: &mut BufReader<std::process::ChildStdout>,
    id: u64,
    method: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    let body = serde_json::to_vec(&req).unwrap();
    write_message(stdin, &body).unwrap();
    stdin.flush().unwrap();
    let resp_bytes = read_message(stdout).unwrap();
    serde_json::from_slice(&resp_bytes).unwrap()
}

#[test]
fn full_session_load_render_dispose() {
    use vrm_asset_generator::params::MToonParams;

    // Build a tiny synthetic VRM + sidecar in a tempdir. The mock only
    // needs the meta.json (it ignores the .vrm body), so a stub is fine.
    let dir = tempfile::tempdir().unwrap();
    let vrm_path = dir.path().join("synthetic.vrm");
    let meta_path = dir.path().join("synthetic.meta.json");
    let out_path = dir.path().join("out.png");

    std::fs::write(&vrm_path, b"glTF\x02\x00\x00\x00\x0c\x00\x00\x00").unwrap();
    let meta = serde_json::json!({
        "id": "synthetic",
        "license": "CC0-1.0",
        "params": MToonParams::defaults("synthetic"),
    });
    std::fs::write(&meta_path, serde_json::to_vec(&meta).unwrap()).unwrap();

    let mut child = spawn_mock();
    let mut stdin = child.stdin.take().expect("stdin piped");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout piped"));

    // load_vrm
    let resp = rpc(
        &mut stdin,
        &mut stdout,
        1,
        "load_vrm",
        serde_json::json!({ "path": vrm_path.to_string_lossy() }),
    );
    let session_id = resp["result"]["session_id"]
        .as_str()
        .expect("load_vrm returns session_id")
        .to_string();
    assert!(session_id.starts_with("mock-"), "got: {session_id}");

    // set_camera (any-shape; mock just stores it)
    let resp = rpc(
        &mut stdin,
        &mut stdout,
        2,
        "set_camera",
        serde_json::json!({
            "session_id": session_id,
            "position": [0.0, 1.4, 1.5],
            "target": [0.0, 1.4, 0.0],
            "up": [0.0, 1.0, 0.0],
            "fov_degrees": 30.0,
        }),
    );
    assert!(resp.get("result").is_some(), "set_camera ok: {resp}");

    // render
    let resp = rpc(
        &mut stdin,
        &mut stdout,
        3,
        "render",
        serde_json::json!({
            "session_id": session_id,
            "width": 64,
            "height": 64,
            "output_path": out_path.to_string_lossy(),
            "color_space": "Linear",
            "msaa": 4,
            "output_type": "Color",
        }),
    );
    assert!(resp.get("result").is_some(), "render ok: {resp:#?}");
    assert!(out_path.exists(), "render must produce a PNG file");

    // The synthesized PNG should be 64×64 RGB.
    let img = image::open(&out_path).unwrap().to_rgb8();
    assert_eq!(img.dimensions(), (64, 64));

    // dispose
    let resp = rpc(
        &mut stdin,
        &mut stdout,
        4,
        "dispose",
        serde_json::json!({ "session_id": session_id }),
    );
    assert!(resp.get("result").is_some());

    // Close stdin so the loop exits cleanly.
    drop(stdin);
    let _ = child.wait();
}

#[test]
fn unknown_method_returns_minus_32601() {
    let mut child = spawn_mock();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let resp = rpc(
        &mut stdin,
        &mut stdout,
        99,
        "nonexistent_op",
        serde_json::json!({}),
    );
    assert_eq!(resp["error"]["code"], -32601);

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn reserved_phase_2_op_returns_unimplemented_phase_2() {
    let mut child = spawn_mock();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let resp = rpc(
        &mut stdin,
        &mut stdout,
        1,
        "step_physics",
        serde_json::json!({ "dt_seconds": 0.016, "count": 1 }),
    );
    assert_eq!(resp["error"]["code"], -32000);
    assert_eq!(resp["error"]["data"]["phase"], "Phase 2");

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn load_vrm_missing_meta_returns_load_failed() {
    let dir = tempfile::tempdir().unwrap();
    let vrm_path = dir.path().join("nomerge.vrm");
    std::fs::write(&vrm_path, b"glTF").unwrap();
    // intentionally no .meta.json sidecar

    let mut child = spawn_mock();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let resp = rpc(
        &mut stdin,
        &mut stdout,
        1,
        "load_vrm",
        serde_json::json!({ "path": vrm_path.to_string_lossy() }),
    );
    assert_eq!(resp["error"]["code"], -32001, "expect LoadFailed: {resp:#?}");

    drop(stdin);
    let _ = child.wait();
}
```

- [ ] **Step 2: Add vrm-asset-generator to dev-deps too**

The contract test uses `MToonParams` to build the meta sidecar. It's already a normal dep, which makes it available to tests too — no change needed unless cargo complains. If it does:

```toml
[dev-dependencies]
tempfile.workspace = true
```

(Already in place; vrm-asset-generator's path dep flows through.)

- [ ] **Step 3: Run tests**

Run: `cargo test -p vrm-mock-renderer`

Expected: 6 synthesis tests + 4 contract tests = 10 pass.

- [ ] **Step 4: Workspace clean**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

All green.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-mock-renderer/tests/contract.rs
git commit -m "test(mock-renderer): subprocess contract test exercises all Phase 1 ops"
```

---

## Section E — Smoke integration + docs

### Task E1: Smoke script uses mock as the default adapter

**Files:**
- Modify: `scripts/smoke.sh`

Today the smoke script tries to build the Swift adapter, expects it to fail at `load_vrm` (L3 blocked), and falls through to a placeholder PNG. With the mock landed, the smoke can actually exercise the runner end-to-end: build mock → asset-generator emits → runner drives mock → render produced → runner diff against the same render → SSIM 1.0.

- [ ] **Step 1: Replace the renderer step**

Find the "step 2: build Swift adapter" and "step 3: runner (known-blocked on L3)" blocks. Replace them with:

```bash
# ---- step 2: build mock renderer ------------------------------------------
echo "==> Building mock renderer (cargo build --release -p vrm-mock-renderer)"
cargo build --release -p vrm-mock-renderer

ADAPTER=$ROOT/target/release/vrm-mock-renderer

# ---- step 3: runner drives mock adapter -----------------------------------
if [ "$SKIP_RENDER" = "1" ]; then
    echo "==> Skipping runner step (--skip-render)"
else
    echo "==> Running test plan against mock adapter"
    cargo run --release -p vrm-runner -- execute-test-plan \
        --plan "$ASSETS/smoke_default.test.yaml" \
        --adapter-bin "$ADAPTER" \
        --asset-dir "$ASSETS" \
        --output-dir "$OUTPUTS" \
        --renderer-name vrm-mock \
        --json
    PNG="$OUTPUTS/smoke_default_vrm-mock.png"
fi
```

Update the header-comment prereq list — remove the Swift mention from "Required" and add a note that the Swift adapter is a stretch target for L3. Specifically, change:

```
#   - VRMMetalKit adapter built (adapters/vrm-metal-kit/.build/release/vrm-metal-kit-adapter)
#       NOTE: as of v0.1 the adapter returns Unimplemented for every op (L3 deferred);
#             the runner step is therefore expected to fail at load_vrm. Pass --skip-render
#             or `SMOKE_SKIP_RENDER=1` to bypass.
```

to:

```
#   - vrm-mock-renderer binary (cargo will build it in step 2 below)
#       The mock is a deterministic CPU adapter that satisfies the Phase 1
#       op contract without GPU or VRMMetalKit dependencies. The real Swift
#       adapter remains an L3 stretch target; the smoke uses the mock so
#       the full runner → diff → S3 → site loop runs green by default.
```

And update the `cargo, swift, node, python3` preflight check to drop `swift`:

```bash
for tool in cargo node python3; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "smoke: missing required tool: $tool" >&2
        exit 1
    }
done
```

- [ ] **Step 2: Verify smoke runs end-to-end**

```bash
./scripts/smoke.sh 2>&1 | tail -15
```

Expected: every step green, including the runner step that produces `smoke_default_vrm-mock.png`. The "Running runner diff loop" step (added in Phase 2A) still self-diffs to SSIM 1.0.

- [ ] **Step 3: Commit**

```bash
git add scripts/smoke.sh
git commit -m "chore(smoke): use vrm-mock-renderer as default adapter (Swift now stretch)"
```

---

### Task E2: Document the mock + update README

**Files:**
- Modify: `README.md`
- Modify: `docs/operation-contract.md`

- [ ] **Step 1: Update README repo layout table**

In `README.md`, add a row to the layout table for `adapters/` and a row for the mock. Find this line:

```
| `adapters/vrm-metal-kit/` | Swift package: VRMMetalKit MCP adapter (macOS / Metal). |
```

Add immediately above it:

```
| `crates/vrm-mock-renderer/` | Rust binary. Deterministic CPU mock adapter that satisfies the Phase 1 op contract — used for E2E CI testing without GPU / VRMMetalKit dependencies. |
```

- [ ] **Step 2: Update operation contract**

In `docs/operation-contract.md`, find the "Reserved operations (Phase 2+)" section. Just before it, add a brief paragraph:

```markdown
## Reference implementations

- **`vrm-mock-renderer`** (in-tree, Rust). A deterministic CPU adapter that satisfies the Phase 1 op contract. Renders are a stable function of `MToonParams` — identical params produce byte-identical PNGs, so self-diff is SSIM 1.0 by construction. Used as the default smoke-test adapter; not a real renderer.
- **`adapters/vrm-metal-kit/`** (in-tree, Swift). Real macOS/Metal renderer scaffold. JSON-RPC framing is implemented; the actual VRMMetalKit integration (L3) is deferred.
```

- [ ] **Step 3: Commit**

```bash
git add README.md docs/operation-contract.md
git commit -m "docs: introduce vrm-mock-renderer in README + operation contract"
```

---

## Self-Review

**Spec coverage:**

| Phase 2B goal | Task |
|---|---|
| Crate scaffold + workspace registration | A1 |
| Deterministic param-encoded PNG synthesis | B1 |
| Session + SessionRegistry state | C1 |
| Per-op handlers (Phase 1 + reserved stubs) | C2 |
| Stdio JSON-RPC dispatch | C3 |
| Subprocess contract test (E2E from runner POV) | D1 |
| Smoke script wires mock as default | E1 |
| README + contract docs | E2 |

**Placeholder scan:** none. All code blocks are complete. Magic constants are explained (50% bbox, 8-pixel stripes, magenta sentinel).

**Type consistency:**

- `Session` fields `camera`, `lighting`, `post_processing` use the exact `vrm_ops::tools::*` param types (not local copies). Handlers in C2 take those same types directly.
- `SessionRegistry::insert/get/get_mut/remove` signatures used identically in main.rs dispatch + handlers + contract test.
- `synthesize_png(params: &MToonParams, w: u32, h: u32) -> RgbImage` consistent across B1, C2, D1.
- `RpcError::load_failed` / `RpcError::render_failed` / `RpcError::unimplemented` — the convenience constructors already exist in `vrm-ops`.

**YAGNI guards:**

- ✅ No GPU code, no GL, no graphics stack.
- ✅ No alpha channel.
- ✅ No real color-space conversion.
- ✅ No Phase 2+ ops implemented — just Unimplemented stubs with the right phase labels.
- ✅ The mock does NOT parse the .vrm GLB; it reads the sidecar instead. Future renderers will of course parse the .vrm; the mock is allowed to take this shortcut.

**Risk register:**

- **Sidecar coupling.** The mock depends on `.meta.json` being adjacent to the `.vrm`. The asset-generator currently emits both via `emit_with_sidecars`, so production assets always have the sidecar. Hand-curated test plans pointing at `.vrm`s without sidecars will fail with `LoadFailed`; this is acceptable for v0.1 since the corpus is generated.
- **`MToonParams::defaults` default for `outline_width_mode`.** Test `outline_mode_none_omits_ring` assumes default is `None`. If F1's defaults ever change, the test will fail loudly with a clear assertion — good.
- **PNG round-trip determinism.** Saving via `image::save` re-encodes as PNG, which is deflate-compressed. Determinism is byte-identical *raw pixel data*, not byte-identical *PNG file bytes*. The synthesis test compares `as_raw()` (raw pixels), so this is correct. The contract test only checks dimensions on the decoded PNG, not bytes.

---

## Execution Handoff

Plan saved to `docs/superpowers/plans/2026-05-10-phase2b-mock-renderer-adapter.md`. Two execution options:

1. **Subagent-Driven (recommended)** — fresh subagent per task, two-stage review per task. 8 tasks; comfortably parallel for B1+C1 (independent files); A1 must land first; D1/E1/E2 sequential after C3.
2. **Inline Execution** — execute tasks in this session via `superpowers:executing-plans`. Critical path is A1 → B1 → C1 → C2 → C3 → D1 → E1 → E2; mostly sequential.

For inline execution, expect ~25 minutes to fully ship Phase 2B given the size.
