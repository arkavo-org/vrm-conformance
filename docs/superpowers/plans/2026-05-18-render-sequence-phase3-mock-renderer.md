# `render_sequence` Phase 3 — Mock Renderer Reference Implementation

> **For agentic workers:** Use superpowers:subagent-driven-development to execute this plan task-by-task. Phase 1 + 2 are complete (commits `7b1f1cf` and `a480f26` respectively). The Rust types, runner dispatch, diff aggregator, and manifest schema are all in place — Phase 3 makes the mock renderer the first **real** implementer of `render_sequence`.

**Goal:** Make `vrm-mock-renderer` deterministically produce per-frame PNG sequences. Same inputs ⇒ byte-identical PNGs ⇒ identity_match across runs and self-diff at SSIM 1.0 by construction. This gives Phase 2's runner integration a deterministic E2E target without GPU, and gives the diff engine a regression-friendly reference for the temporal_diff machinery.

**Architecture:** The mock renderer's existing `synthesize_png(params, w, h)` (in `render.rs`) is the single-frame fingerprint. Phase 3 generalizes it to `synthesize_frame(params, frame_state, w, h)` where `frame_state` encodes the per-frame variation (root-transform shift, VRMA clip-time, frame index). A new `render_sequence` handler iterates frames, calls the synthesis function, writes per-frame PNGs, optionally invokes `ffmpeg` to mux MP4/MOV (skipped with a log warning if `ffmpeg` is absent), and returns `RenderSequenceResult` with BLAKE3 hashes.

**Tech stack:** Pure Rust; optional `ffmpeg` subprocess for mux.

**Spec:** [`rfcs/0004-render-sequence-op.md`](../../../rfcs/0004-render-sequence-op.md) — output_format + frame_state semantics. The mock renderer is the canonical reference for what an adapter is *expected* to produce.

---

## Determinism contract

The whole point of the mock is determinism. Phase 3 must preserve this:

- `synthesize_frame(params, frame_state, w, h)` MUST return the same `RgbImage` bit-for-bit for identical inputs across runs / hosts / Rust toolchain versions.
- The `image` crate's PNG encoder is deterministic for solid-color and small structural patterns (Phase 2 Task 3 confirmed this for `[u8; 3]` solid colors). Avoid timestamp metadata in PNG headers; the `image` crate's default encoder does NOT embed time.
- `RenderSequenceResult.frames[i].blake3` MUST therefore be stable across runs.

The Phase 3 determinism test (Task 4) cross-validates this: run `render_sequence` twice with identical params, assert byte-identical PNGs file-by-file and identical BLAKE3 hashes in the result.

---

## File structure

**Modify:**
- `crates/vrm-mock-renderer/src/render.rs` — add `synthesize_frame` (generalization of `synthesize_png`)
- `crates/vrm-mock-renderer/src/handlers.rs` — add `render_sequence` handler
- `crates/vrm-mock-renderer/src/main.rs` — replace `render_sequence => Err(unimplemented(...))` with `json_result(handlers::render_sequence(...))`
- `crates/vrm-mock-renderer/Cargo.toml` — add `blake3.workspace = true`
- `scripts/smoke.sh` — `--sequence` mode

**Create:**
- `crates/vrm-mock-renderer/tests/render_sequence_determinism.rs` — cross-run determinism

---

## Task 1: `synthesize_frame` + `render_sequence` handler (PNG sequence)

This is the core landing — frame-aware synthesis + the handler that drives the loop and writes PNGs. Mux/MP4 is deferred to Task 3, animation encoding details are refined in Task 2.

**Files:**
- Modify: `crates/vrm-mock-renderer/src/render.rs`
- Modify: `crates/vrm-mock-renderer/src/handlers.rs`
- Modify: `crates/vrm-mock-renderer/src/main.rs`
- Modify: `crates/vrm-mock-renderer/Cargo.toml`

- [ ] **Step 1.1: Add `blake3` to mock-renderer deps**

Edit `crates/vrm-mock-renderer/Cargo.toml`. Add `blake3.workspace = true` to `[dependencies]`.

- [ ] **Step 1.2: Add `FrameState` + `synthesize_frame` to render.rs**

In `render.rs`, alongside the existing `synthesize_png`:

```rust
/// Per-frame state that drives deterministic variation in
/// `synthesize_frame`. Encodes everything an animation can change about
/// the rendered pixel content. Values must be deterministic in the
/// inputs — no clock time, no hash randomness.
#[derive(Debug, Clone, Copy)]
pub struct FrameState {
    /// Frame index in [0, frame_count).
    pub index: u32,
    pub frame_count: u32,
    /// Linear root-transform offset (metres) applied at this frame's
    /// `index`. Computed by the handler as a lerp between
    /// `animate_root_transform.translation_start` and `translation_end`.
    /// `[0, 0, 0]` when no animation is in play.
    pub root_translation: [f32; 3],
    /// VRMA sample time at this frame: `start_seconds + index / frame_hz`.
    /// `None` when no `apply_vrma` block was set.
    pub vrma_time_seconds: Option<f32>,
}

/// Frame-aware synthesis. Mirrors `synthesize_png` but adds:
///   - 4-pixel "frame index marker" stripe in the bottom-left corner
///     (encodes index/frame_count as a horizontal bar that grows across
///     the sequence). Lets a reviewer scrub a sequence and see
///     frame-index visually.
///   - When `root_translation.x != 0`, the avatar bbox shifts
///     horizontally by `(root_translation.x * width).round()` pixels
///     (clamped so the bbox doesn't leave the frame).
///   - When `vrma_time_seconds.is_some()`, a 4×4 "vrma marker" square
///     in the top-right corner whose color encodes time-mod-1.0 as
///     `[r, g, 0]` where r and g are 8-bit fractions of (t * 256).
///
/// Same `(params, state, w, h)` → byte-identical RgbImage.
pub fn synthesize_frame(
    params: &MToonParams,
    state: FrameState,
    width: u32,
    height: u32,
) -> RgbImage {
    // Reuse synthesize_png as the base — same fill/stripes/outline logic.
    let mut img = synthesize_png(params, width, height);

    // Horizontal shift via row-by-row copy. Skip when shift is 0.
    let shift_x_px = (state.root_translation[0] * width as f32).round() as i32;
    if shift_x_px != 0 {
        img = shift_horizontally(&img, shift_x_px);
    }

    // Frame index marker — bottom-left bar.
    draw_frame_marker(&mut img, state.index, state.frame_count);

    // VRMA time marker — top-right square.
    if let Some(t) = state.vrma_time_seconds {
        draw_vrma_marker(&mut img, t);
    }

    img
}

fn shift_horizontally(img: &RgbImage, shift_x: i32) -> RgbImage {
    let (w, h) = img.dimensions();
    let mut out = RgbImage::from_pixel(w, h, Rgb(MAGENTA));
    for y in 0..h {
        for x in 0..w {
            let src_x = x as i32 - shift_x;
            if src_x >= 0 && src_x < w as i32 {
                let src = img.get_pixel(src_x as u32, y);
                out.put_pixel(x, y, *src);
            }
        }
    }
    out
}

fn draw_frame_marker(img: &mut RgbImage, index: u32, frame_count: u32) {
    let (w, h) = img.dimensions();
    if h < 6 || w < 6 || frame_count == 0 {
        return;
    }
    let bar_y_start = h - 4;
    let frac = (index as f32 + 1.0) / frame_count as f32;
    let bar_x_end = ((w as f32) * frac).round() as u32;
    let bar_x_end = bar_x_end.min(w);
    for y in bar_y_start..h {
        for x in 0..bar_x_end {
            img.put_pixel(x, y, Rgb([0, 255, 0]));
        }
    }
}

fn draw_vrma_marker(img: &mut RgbImage, time_seconds: f32) {
    let (w, _h) = img.dimensions();
    if w < 6 {
        return;
    }
    let frac = time_seconds.rem_euclid(1.0);
    let r = (frac * 256.0).floor().clamp(0.0, 255.0) as u8;
    let g = ((frac * 65536.0).floor() as u32 & 0xFF) as u8;
    let color = [r, g, 0];
    let x_start = w - 4;
    for y in 0..4_u32 {
        for x in x_start..w {
            img.put_pixel(x, y, Rgb(color));
        }
    }
}
```

`MAGENTA` is already defined at the top of the file — reuse it. The helpers are pure / deterministic. Place them after `synthesize_png` so the file's existing ordering is preserved.

- [ ] **Step 1.3: Add the `render_sequence` handler**

In `handlers.rs`, append:

```rust
use crate::render::{synthesize_frame, FrameState};
use std::path::PathBuf;

pub fn render_sequence(
    registry: &mut SessionRegistry,
    params: ops::RenderSequenceParams,
) -> Result<ops::RenderSequenceResult, RpcError> {
    // Mutual-exclusion validation (per RFC-0004 + docs/operation-contract.md).
    if params.animate_root_transform.is_some() && params.apply_vrma.is_some() {
        return Err(RpcError {
            code: -32602,
            message: "render_sequence: animate_root_transform and apply_vrma are mutually exclusive".into(),
            data: None,
        });
    }
    // 60 Hz physics floor (methodology pin).
    if params.physics_dt_seconds > 1.0 / 60.0 + 1e-9 {
        return Err(RpcError {
            code: -32602,
            message: format!(
                "render_sequence: physics_dt_seconds {} exceeds 60 Hz floor (1/60 ≈ 0.01667)",
                params.physics_dt_seconds
            ),
            data: None,
        });
    }

    let session = registry
        .get(&params.session_id)
        .ok_or_else(|| invalid_session(&params.session_id))?;

    // Resolve animation source.
    let root_anim = params.animate_root_transform.clone();
    let vrma_spec = params.apply_vrma.clone();

    let session_params = session.params.clone();

    // Prepare output dir.
    let output_dir = camino::Utf8PathBuf::from(&params.output_dir);
    std::fs::create_dir_all(output_dir.as_std_path())
        .map_err(|e| RpcError::render_failed(format!("create output_dir: {e}")))?;

    let mut frames = Vec::with_capacity(params.frame_count as usize);
    for i in 0..params.frame_count {
        let t = if params.frame_count <= 1 {
            0.0
        } else {
            i as f32 / (params.frame_count - 1) as f32
        };

        let root_translation = if let Some(anim) = &root_anim {
            lerp3(anim.translation_start, anim.translation_end, t)
        } else {
            [0.0, 0.0, 0.0]
        };

        let vrma_time = vrma_spec.as_ref().map(|v| {
            v.start_seconds + (i as f32) / params.frame_hz
        });

        let state = FrameState {
            index: i,
            frame_count: params.frame_count,
            root_translation,
            vrma_time_seconds: vrma_time,
        };

        let img = synthesize_frame(&session_params, state, params.width, params.height);
        let frame_path = output_dir.join(format!("{:04}.png", i));
        img.save(frame_path.as_std_path())
            .map_err(|e| RpcError::render_failed(format!("save frame {i}: {e}")))?;

        // Compute BLAKE3 of PNG bytes.
        let bytes = std::fs::read(frame_path.as_std_path())
            .map_err(|e| RpcError::render_failed(format!("read frame {i} for blake3: {e}")))?;
        let hash = blake3::hash(&bytes);
        let blake3_hex = format!("blake3:{}", hash.to_hex());

        frames.push(ops::SequenceFrame {
            index: i,
            timestamp_seconds: (i as f32) / params.frame_hz,
            path: frame_path.to_string(),
            blake3: blake3_hex,
        });
    }

    let duration_seconds = if params.frame_hz > 0.0 {
        params.frame_count as f32 / params.frame_hz
    } else {
        0.0
    };

    Ok(ops::RenderSequenceResult {
        frames,
        duration_seconds,
        actual_color_space: params.color_space,
        frame_hz_achieved: params.frame_hz,
        muxed_path: None,  // Mux lands in Task 3
    })
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}
```

The `invalid_session` helper already exists in `handlers.rs` (it's used by `set_camera` etc.). Reuse it.

- [ ] **Step 1.4: Wire it into the dispatcher**

In `main.rs`, replace:
```rust
"render_sequence" => Err(handlers::unimplemented(method, "v1.x-sequence")),
```
with:
```rust
"render_sequence" => json_result(handlers::render_sequence(registry, deser(params)?)),
```

Place this alongside the other Phase-1+ real handlers (between `render` and `dispose` makes sense for the dispatch table flow).

- [ ] **Step 1.5: Build + clippy**

```
cargo build -p vrm-mock-renderer
cargo clippy --workspace --all-targets -- -D warnings
```

If clippy flags `cast_possible_truncation` on `(state.root_translation[0] * width as f32).round() as i32`, suppress narrowly with `#[allow(clippy::cast_possible_truncation)]` on the function — the precision risk is bounded (width ≤ ~4K pixels, translation ≤ ~10 m for any reasonable test).

- [ ] **Step 1.6: Sanity smoke**

```bash
# Generate a test asset
mkdir -p /tmp/mock-seq-smoke
cargo run --release -p vrm-asset-generator -- emit-default --id mock_seq --output-dir /tmp/mock-seq-smoke/

# Drive the mock through render_sequence directly via JSON-RPC stdin
cargo run --release -p vrm-mock-renderer 2>/dev/null <<'EOF' | head -c 500
Content-Length: 105

{"jsonrpc":"2.0","id":1,"method":"load_vrm","params":{"path":"/tmp/mock-seq-smoke/mock_seq.vrm"}}
EOF
```

This is informal smoke — the integration test in Task 4 is the authoritative check. The frame loop should emit `0000.png` through `00<N-1>.png` in `/tmp/mock-seq-out/` and the JSON-RPC result should be parseable.

- [ ] **Step 1.7: Commit**

```bash
git add crates/vrm-mock-renderer/
git commit -m "$(cat <<'EOF'
feat(vrm-mock-renderer): implement render_sequence (PNG sequence only)

synthesize_frame is a frame-aware generalization of synthesize_png. It
reuses the base parametric synthesis and adds three frame-encoded
elements: a horizontal shift driven by animate_root_transform, a
bottom-left progress bar encoding (index, frame_count), and a top-right
RGB marker encoding vrma_time_seconds.

render_sequence handler iterates frame_count times, writes per-frame
PNGs to <output_dir>/<i:04>.png, computes BLAKE3 of each PNG, and
returns SequenceFrame entries. Mutual-exclusion validation matches
RFC-0004 (animate_root_transform + apply_vrma both set ⇒ -32602; physics_
dt_seconds > 1/60 ⇒ -32602). MP4/MOV mux is deferred to Task 3.

Determinism guarantee: same (params, FrameState, width, height) ⇒
byte-identical RgbImage ⇒ stable BLAKE3 across runs.
EOF
)"
```

---

## Task 2: Animation encoding refinement (optional polish)

Task 1 already encodes both animation sources. This task adds **anchor tests** that pin the encoding semantics so future refactors can't silently change pixel content.

**Files:**
- Create: `crates/vrm-mock-renderer/tests/render_sequence_animation.rs`

- [ ] **Step 2.1: Add anchor tests for animation encoding**

```rust
//! Anchor tests pinning the mock renderer's per-frame variation semantics.
//! These guard against silent changes to how animate_root_transform and
//! apply_vrma encode into pixel content.

use camino::Utf8PathBuf;
use vrm_mock_renderer::handlers;
use vrm_mock_renderer::session::{Session, SessionRegistry};
use vrm_ops::tools as ops;

fn load_mock_session(registry: &mut SessionRegistry) -> String {
    // Use the asset generator to emit a known-shape asset, then load it.
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args(&[
            "emit-default",
            "--id", "anchor",
            "--output-dir", out.to_str().unwrap(),
        ])
        .status()
        .expect("asset-generator run");
    assert!(status.success());

    let asset_path = Utf8PathBuf::try_from(out.join("anchor.vrm")).unwrap();
    let session = Session::load(asset_path.as_path()).unwrap();
    let id = registry.insert(session);
    // Keep tempdir alive across the test
    Box::leak(Box::new(dir));
    id
}

fn base_params(session_id: &str, output_dir: &str) -> ops::RenderSequenceParams {
    ops::RenderSequenceParams {
        session_id: session_id.into(),
        width: 64, height: 64,
        output_dir: output_dir.into(),
        frame_count: 3,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        color_space: ops::ColorSpace::Linear,
        msaa: 1,
        output_type: ops::OutputType::Color,
        output_format: ops::SequenceFormat::PngSequence,
        animate_root_transform: None,
        apply_vrma: None,
    }
}

#[test]
fn frames_without_animation_differ_only_in_frame_marker() {
    let mut reg = SessionRegistry::new();
    let session_id = load_mock_session(&mut reg);
    let out = tempfile::tempdir().unwrap();

    let mut params = base_params(&session_id, out.path().to_str().unwrap());
    params.frame_count = 3;

    let result = handlers::render_sequence(&mut reg, params).unwrap();
    assert_eq!(result.frames.len(), 3);
    // Frames differ (frame-index marker grows). Their BLAKE3 hashes
    // must be distinct.
    assert_ne!(result.frames[0].blake3, result.frames[1].blake3);
    assert_ne!(result.frames[1].blake3, result.frames[2].blake3);
}

#[test]
fn frames_with_root_animation_shift_per_frame() {
    let mut reg = SessionRegistry::new();
    let session_id = load_mock_session(&mut reg);
    let out = tempfile::tempdir().unwrap();

    let mut params = base_params(&session_id, out.path().to_str().unwrap());
    params.frame_count = 3;
    params.animate_root_transform = Some(ops::RootTransformAnimation {
        translation_start: [0.0, 0.0, 0.0],
        translation_end: [0.5, 0.0, 0.0],  // 50% of width shift
    });

    let result = handlers::render_sequence(&mut reg, params).unwrap();
    // All frames must be distinct (shift differs each frame even with
    // the frame_marker accounted for).
    let hashes: std::collections::HashSet<_> =
        result.frames.iter().map(|f| f.blake3.clone()).collect();
    assert_eq!(hashes.len(), 3, "all 3 frames should have distinct hashes");
}

#[test]
fn frames_with_vrma_encode_time_in_marker() {
    let mut reg = SessionRegistry::new();
    let session_id = load_mock_session(&mut reg);
    let out = tempfile::tempdir().unwrap();

    let mut params = base_params(&session_id, out.path().to_str().unwrap());
    params.frame_count = 3;
    params.apply_vrma = Some(ops::VrmaPlaybackSpec {
        vrma_handle: 1,
        start_seconds: 0.0,
    });

    let result = handlers::render_sequence(&mut reg, params).unwrap();
    assert_eq!(result.frames.len(), 3);
    // VRMA marker color advances; frames are distinct.
    assert_ne!(result.frames[0].blake3, result.frames[1].blake3);
}

#[test]
fn mutual_exclusion_rejected() {
    let mut reg = SessionRegistry::new();
    let session_id = load_mock_session(&mut reg);
    let out = tempfile::tempdir().unwrap();

    let mut params = base_params(&session_id, out.path().to_str().unwrap());
    params.animate_root_transform = Some(ops::RootTransformAnimation {
        translation_start: [0.0; 3],
        translation_end: [1.0, 0.0, 0.0],
    });
    params.apply_vrma = Some(ops::VrmaPlaybackSpec {
        vrma_handle: 1,
        start_seconds: 0.0,
    });

    let err = handlers::render_sequence(&mut reg, params).unwrap_err();
    assert_eq!(err.code, -32602);
}

#[test]
fn physics_dt_above_60hz_rejected() {
    let mut reg = SessionRegistry::new();
    let session_id = load_mock_session(&mut reg);
    let out = tempfile::tempdir().unwrap();

    let mut params = base_params(&session_id, out.path().to_str().unwrap());
    params.physics_dt_seconds = 0.1;  // way over 1/60

    let err = handlers::render_sequence(&mut reg, params).unwrap_err();
    assert_eq!(err.code, -32602);
}
```

If `Session::load` requires a `meta.json` sidecar (it does — the existing implementation reads it), this is fine because `emit-default` writes both `.vrm` and `.meta.json`.

If the test setup is awkward (e.g. session re-use across tests), refactor with a helper module. Each test currently builds an isolated registry and session.

- [ ] **Step 2.2: Run + commit**

```
cargo test -p vrm-mock-renderer --test render_sequence_animation
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
git add crates/vrm-mock-renderer/tests/render_sequence_animation.rs
git commit -m "$(cat <<'EOF'
test(vrm-mock-renderer): anchor tests for render_sequence animation

Pins the semantics of the frame-index marker, root-transform shift, and
VRMA time encoding so future refactors can't silently change pixel
content. Also pins the two validation rules (mutual exclusion +
physics_dt floor).
EOF
)"
```

---

## Task 3: ffmpeg mux for MP4/MOV (with fallback)

**Files:**
- Modify: `crates/vrm-mock-renderer/src/handlers.rs` — extend `render_sequence` to invoke ffmpeg when `output_format != PngSequence`

- [ ] **Step 3.1: Add ffmpeg invocation**

After the frame loop in `render_sequence`, before constructing the result:

```rust
let muxed_path = match params.output_format {
    ops::SequenceFormat::PngSequence => None,
    ops::SequenceFormat::Mp4 => mux_via_ffmpeg(
        &output_dir,
        "sequence.mp4",
        &["-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "0"],
        params.frame_hz,
    )?,
    ops::SequenceFormat::Mov => mux_via_ffmpeg(
        &output_dir,
        "sequence.mov",
        &["-c:v", "prores_ks", "-profile:v", "4444"],
        params.frame_hz,
    )?,
};

// ... then:
muxed_path,  // in the result struct
```

The helper:

```rust
fn mux_via_ffmpeg(
    output_dir: &camino::Utf8Path,
    out_file: &str,
    codec_args: &[&str],
    frame_hz: f32,
) -> Result<Option<String>, RpcError> {
    // Soft-skip when ffmpeg isn't installed.
    let probe = std::process::Command::new("ffmpeg")
        .arg("-version")
        .output();
    if probe.is_err() {
        tracing::warn!(
            "ffmpeg not found on PATH; muxed output {} skipped \
            (PNG sequence is still written)",
            out_file
        );
        return Ok(None);
    }

    let mux_path = output_dir.join(out_file);
    let frame_pattern = output_dir.join("%04d.png");
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.arg("-y")  // overwrite
        .arg("-framerate").arg(format!("{frame_hz}"))
        .arg("-i").arg(frame_pattern.as_std_path())
        .args(codec_args)
        .arg(mux_path.as_std_path());

    let out = cmd.output().map_err(|e| {
        RpcError::render_failed(format!("ffmpeg invocation failed: {e}"))
    })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(RpcError::render_failed(format!(
            "ffmpeg muxing {} failed (exit {}): {}",
            out_file,
            out.status.code().unwrap_or(-1),
            stderr.lines().rev().take(5).collect::<Vec<_>>().join(" / ")
        )));
    }

    Ok(Some(mux_path.to_string()))
}
```

- [ ] **Step 3.2: Add a test that PngSequence skips mux entirely**

Append to `render_sequence_animation.rs`:

```rust
#[test]
fn png_sequence_format_produces_no_muxed_file() {
    let mut reg = SessionRegistry::new();
    let session_id = load_mock_session(&mut reg);
    let out = tempfile::tempdir().unwrap();

    let params = base_params(&session_id, out.path().to_str().unwrap());
    let result = handlers::render_sequence(&mut reg, params).unwrap();
    assert!(result.muxed_path.is_none());
}

#[test]
#[ignore = "requires ffmpeg on PATH; if absent, runs and asserts soft-skip"]
fn mp4_format_invokes_ffmpeg_or_soft_skips() {
    let mut reg = SessionRegistry::new();
    let session_id = load_mock_session(&mut reg);
    let out = tempfile::tempdir().unwrap();

    let mut params = base_params(&session_id, out.path().to_str().unwrap());
    params.output_format = ops::SequenceFormat::Mp4;
    params.frame_count = 5;

    let result = handlers::render_sequence(&mut reg, params).unwrap();
    // Either ffmpeg ran and produced sequence.mp4, OR it wasn't on PATH
    // and muxed_path is None (soft-skip path). Both are acceptable.
    if let Some(path) = &result.muxed_path {
        assert!(std::path::Path::new(path).exists());
    }
}
```

The MP4 test is `#[ignore]` because CI may or may not have ffmpeg. It's still runnable locally via `cargo test --ignored`. The soft-skip path is asserted by the absence of a hard failure.

- [ ] **Step 3.3: Commit**

```bash
git add crates/vrm-mock-renderer/
git commit -m "$(cat <<'EOF'
feat(vrm-mock-renderer): ffmpeg mux for MP4 / MOV output formats

Shell-out to ffmpeg when output_format is Mp4 (libx264 yuv420p crf=0) or
Mov (prores_ks 4444). PNG sequence is always written first; mux is in
addition, not instead. Soft-skip when ffmpeg is absent — log a warning
and return muxed_path: None. Hard-fail only when ffmpeg is present but
muxing fails.

Mp4-format test is #[ignore]-gated because CI may not have ffmpeg.
EOF
)"
```

---

## Task 4: Cross-run determinism integration test

**Files:**
- Create: `crates/vrm-mock-renderer/tests/render_sequence_determinism.rs`

- [ ] **Step 4.1: Add the test**

```rust
//! Cross-run determinism: render the same sequence twice into two
//! distinct temp dirs, assert byte-identical PNGs file-by-file and
//! identical BLAKE3 hashes in the result.

use camino::Utf8PathBuf;
use vrm_mock_renderer::handlers;
use vrm_mock_renderer::session::{Session, SessionRegistry};
use vrm_ops::tools as ops;

fn fresh_session() -> (SessionRegistry, String, tempfile::TempDir) {
    let mut reg = SessionRegistry::new();
    let asset_dir = tempfile::tempdir().unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args(&[
            "emit-default",
            "--id", "determinism",
            "--output-dir", asset_dir.path().to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let asset_path = Utf8PathBuf::try_from(asset_dir.path().join("determinism.vrm")).unwrap();
    let session = Session::load(asset_path.as_path()).unwrap();
    let id = reg.insert(session);
    (reg, id, asset_dir)
}

fn params(session_id: &str, output_dir: &str) -> ops::RenderSequenceParams {
    ops::RenderSequenceParams {
        session_id: session_id.into(),
        width: 64, height: 64,
        output_dir: output_dir.into(),
        frame_count: 5,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        color_space: ops::ColorSpace::Linear,
        msaa: 1,
        output_type: ops::OutputType::Color,
        output_format: ops::SequenceFormat::PngSequence,
        animate_root_transform: Some(ops::RootTransformAnimation {
            translation_start: [0.0, 0.0, 0.0],
            translation_end: [0.25, 0.0, 0.0],
        }),
        apply_vrma: None,
    }
}

#[test]
fn two_runs_produce_byte_identical_pngs() {
    let (mut reg, session_id, _asset_dir) = fresh_session();

    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();

    let result_a = handlers::render_sequence(
        &mut reg,
        params(&session_id, dir_a.path().to_str().unwrap()),
    ).unwrap();
    let result_b = handlers::render_sequence(
        &mut reg,
        params(&session_id, dir_b.path().to_str().unwrap()),
    ).unwrap();

    assert_eq!(result_a.frames.len(), result_b.frames.len());

    for (fa, fb) in result_a.frames.iter().zip(result_b.frames.iter()) {
        // BLAKE3 hashes must match across runs
        assert_eq!(fa.blake3, fb.blake3, "frame {}: blake3 differs", fa.index);

        // PNG bytes must be byte-identical
        let bytes_a = std::fs::read(&fa.path).unwrap();
        let bytes_b = std::fs::read(&fb.path).unwrap();
        assert_eq!(bytes_a, bytes_b, "frame {} bytes differ", fa.index);
    }
}

#[test]
fn self_diff_via_temporal_diff_returns_ssim_1_for_all_frames() {
    use vrm_diff_engine::temporal::temporal_diff;

    let (mut reg, session_id, _asset_dir) = fresh_session();

    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();

    let result_a = handlers::render_sequence(
        &mut reg,
        params(&session_id, dir_a.path().to_str().unwrap()),
    ).unwrap();
    let result_b = handlers::render_sequence(
        &mut reg,
        params(&session_id, dir_b.path().to_str().unwrap()),
    ).unwrap();

    let paths_a: Vec<camino::Utf8PathBuf> = result_a
        .frames
        .iter()
        .map(|f| camino::Utf8PathBuf::from(&f.path))
        .collect();
    let paths_b: Vec<camino::Utf8PathBuf> = result_b
        .frames
        .iter()
        .map(|f| camino::Utf8PathBuf::from(&f.path))
        .collect();

    let refs_a: Vec<&camino::Utf8Path> = paths_a.iter().map(|p| p.as_path()).collect();
    let refs_b: Vec<&camino::Utf8Path> = paths_b.iter().map(|p| p.as_path()).collect();

    let diff = temporal_diff(&refs_a, &refs_b, 0.95).unwrap();

    assert!(diff.frame_count_match);
    assert_eq!(diff.mean_ssim, 1.0, "self-diff mean SSIM should be exactly 1.0");
    assert_eq!(diff.min_ssim, 1.0, "self-diff min SSIM should be exactly 1.0");
    assert!(
        diff.per_frame.iter().all(|f| f.identity_match),
        "all frames should be identity_match (BLAKE3 short-circuit)"
    );
    assert!(diff.passed);
}
```

The second test requires adding `vrm-diff-engine = { path = "../vrm-diff-engine" }` to the mock renderer's `[dev-dependencies]`. Verify; if absent, add it.

- [ ] **Step 4.2: Run + commit**

```
cargo test -p vrm-mock-renderer --test render_sequence_determinism
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
git add crates/vrm-mock-renderer/
git commit -m "$(cat <<'EOF'
test(vrm-mock-renderer): cross-run determinism + self-diff invariant

Two render_sequence runs with identical params produce byte-identical
PNG files frame-by-frame and identical BLAKE3 hashes in the result.
Self-diff via vrm_diff_engine::temporal::temporal_diff returns
mean_ssim=1.0, min_ssim=1.0, identity_match=true for every frame —
the BLAKE3 short-circuit catches every pair.

This is the Phase 3 done-when condition.
EOF
)"
```

---

## Task 5: Runner end-to-end against mock (sequence smoke)

**Files:**
- Modify: `scripts/smoke.sh` — add `--sequence` mode
- OR: Create: `crates/vrm-runner/tests/render_sequence_e2e_mock.rs`

The runner integration test from Phase 2 Task 10 (`crates/vrm-runner/tests/render_sequence_unimplemented.rs`) asserts the mock returns Unimplemented. Phase 3 obsoletes that — the mock now implements `render_sequence` for real. We have two options:

1. **Update the existing test** to assert the success path instead of Unimplemented.
2. **Add a new test** that asserts the real path, leaving the Unimplemented test broken (FAIL: blocked path).

Choose option 1.

- [ ] **Step 5.1: Rewrite `render_sequence_unimplemented.rs` to assert success**

The test should now construct a plan with `render_sequence`, run it through the runner against the mock, and assert `result.sequence.status == SequenceStatus::Ok` with non-empty `result.frames`.

The test currently expects `Unimplemented`. Update the assertions. Rename the file too: `render_sequence_unimplemented.rs` → `render_sequence_e2e_mock.rs`.

- [ ] **Step 5.2: Extend `scripts/smoke.sh`**

Add a `--sequence` flag (or `SEQUENCE=1` env var) that, after the existing single-frame smoke, runs the mock against a sequence plan and asserts the result PNGs exist + are non-trivial.

Sketch:

```bash
if [ "${SEQUENCE:-0}" = "1" ]; then
    echo "==> sequence smoke (mock renderer + render_sequence)"
    # Emit a sequence test plan inline
    SEQ_PLAN="$SMOKE_DIR/plans/seq.test.yaml"
    cat > "$SEQ_PLAN" <<'EOF_PLAN'
id: smoke_seq
spec_section: x
asset: smoke.vrm
camera: {position: [0,1.2,1.5], target: [0,1,0], up: [0,1,0], fov_degrees: 30.0}
lighting:
  directional: {dir: [0,-1,0], color: [1,1,1], intensity: 1.0}
  ambient: {color: [1,1,1], intensity: 0.3}
output: {width: 64, height: 64, color_space: linear, msaa: 1}
diff: {mode: ssim, threshold: 0.5, reference_renderer: mock}
render_sequence:
  frame_count: 5
  frame_hz: 30.0
  physics_dt_seconds: 0.01666
  output_format: png_sequence
EOF_PLAN

    cargo run --release -p vrm-runner -- execute-test-plan \
        --plan "$SEQ_PLAN" \
        --adapter-bin target/release/vrm-mock-renderer \
        --asset-dir "$SMOKE_DIR/plans" \
        --output-dir "$SMOKE_DIR/seq-out" \
        --renderer-name mock \
        --json | tee "$SMOKE_DIR/seq-summary.json"

    # Assert per-frame files exist
    for i in 0000 0001 0002 0003 0004; do
        if [ ! -f "$SMOKE_DIR/seq-out/smoke_seq_mock_frames/$i.png" ]; then
            echo "FAIL: missing frame $i"
            exit 4
        fi
    done
    echo "OK: 5 sequence frames written"
fi
```

Verify the exact output directory the runner uses for sequence frames matches what Phase 2 Task 10 chose. The pattern was `<output_dir>/<id>_<renderer>_frames/`.

- [ ] **Step 5.3: Build + verify + commit**

```
cargo test -p vrm-runner --test render_sequence_e2e_mock
SEQUENCE=1 bash scripts/smoke.sh   # informal verification
```

```bash
git add crates/vrm-runner/tests/render_sequence_e2e_mock.rs scripts/smoke.sh
# also remove the old file if you renamed
git rm crates/vrm-runner/tests/render_sequence_unimplemented.rs 2>/dev/null || true
git commit -m "$(cat <<'EOF'
test(vrm-runner): render_sequence end-to-end against mock renderer

Replaces the Phase 2 Task 10 Unimplemented-assertion test now that the
mock is a real implementer. The mock is the canonical reference
renderer; this test exercises the full pipeline: TestPlan → plan_to_ops
→ execute_plan → render_sequence dispatch → SequenceExecuteResult → 5
PNG frames on disk with BLAKE3 hashes.

scripts/smoke.sh gains a SEQUENCE=1 mode for informal local verification.
EOF
)"
```

---

## Task 6: Workspace cleanup

- [ ] **Step 6.1: fmt + clippy + workspace test + npm test**

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd adapters/three-vrm && npm test && cd -
```

- [ ] **Step 6.2: Commit any fmt fixes (if any)**

---

## Phase 3 completion checklist

- [ ] `crates/vrm-mock-renderer/src/render.rs` has `synthesize_frame` + `FrameState`
- [ ] `crates/vrm-mock-renderer/src/handlers.rs` has `render_sequence` handler with mutual-exclusion + physics_dt validation
- [ ] `crates/vrm-mock-renderer/src/main.rs` dispatches `render_sequence` to the real handler (no longer Unimplemented)
- [ ] PNG sequence output works; BLAKE3 hashes populated in result
- [ ] MP4/MOV mux invokes ffmpeg when present; soft-skips when absent
- [ ] Animation encoding anchored by tests (root shift, vrma marker, frame marker)
- [ ] Cross-run determinism test: 2 runs → byte-identical PNGs + identical BLAKE3
- [ ] Self-diff via temporal_diff: mean_ssim=min_ssim=1.0, every frame identity_match=true
- [ ] Runner end-to-end against mock succeeds (TestPlan → frames on disk)
- [ ] scripts/smoke.sh `SEQUENCE=1` mode produces 5 frames
- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo test --workspace` green
- [ ] three-vrm npm test green

After Phase 3, the temporal_diff machinery, runner integration, and mock renderer compose into a fully exercised pipeline. Phase 4 (asset generator + sequence-capable test plans) is the next blocker on real adapter implementations (Phase 5+).
