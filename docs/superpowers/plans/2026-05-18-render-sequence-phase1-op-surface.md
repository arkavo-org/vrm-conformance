# `render_sequence` Phase 1 — Op Surface + Unimplemented Stubs

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the `render_sequence` op type plus 4 supporting types in `crates/vrm-ops/` and have every adapter return `-32000 Unimplemented` (with `phase: "v1.x-sequence"` in the error envelope) for the new method name. After this phase, the operation contract published by `describe` declares `render_sequence` support, and the runner-side wiring in Phase 2 can be developed against a stable surface. Methodology pins for sequences also land in this phase.

**Architecture:** Six new `*Params` / `*Result` / supporting types in `crates/vrm-ops/src/tools.rs` mirroring the existing op-type pattern, with full serde round-trip tests. Each adapter (VMK / three-vrm / godot-vrm / UniVRM) dispatches the new method name through its existing Unimplemented escape route — no real implementation lands in this phase. The describe catalog reads the same types via the same JSON-Schema-via-`schemars` path the existing ops use; a single sanity test verifies catalog exposure. `docs/methodology.md` gains the sequence-specific pins so adapter implementations in Phase 5–7 have an authoritative reference.

**Tech Stack:** Rust workspace (vrm-ops), Swift (VMK adapter), TypeScript (three-vrm adapter), Rust + GDScript (godot-vrm adapter via vrm-godot-shim), C# (UniVRM adapter).

**Spec:** [`rfcs/0004-render-sequence-op.md`](../../../rfcs/0004-render-sequence-op.md) — full RFC. Phase-zero gate: RFC MUST be Accepted before this plan starts execution.

---

## File structure

**Modify:**
- `crates/vrm-ops/src/tools.rs` — add 6 new types (~180 LOC additions, all in the same file because that's where every other op type lives in this crate)
- `crates/vrm-ops/tests/serde.rs` — add serde round-trip tests for the new types
- `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift` — add `render_sequence` to the Unimplemented deferral set
- `adapters/three-vrm/src/operations.ts` — add `render_sequence` handler returning Unimplemented
- `crates/vrm-godot-shim/src/bridge.rs` — add `render_sequence` to the shim's Unimplemented set
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs` — add `render_sequence` to the batch dispatcher's Unimplemented path
- `docs/operation-contract.md` — document `render_sequence` alongside `render`
- `docs/methodology.md` — add "Sequence captures" section with the 60 Hz physics floor, no-temporal-alignment rule, and worst-frame-index reporting convention

**Create:** none (this phase is pure additions to existing files).

---

## Task 1: Supporting types — `SequenceFormat`, `RootTransformAnimation`, `VrmaPlaybackSpec`, `SequenceFrame`

These four types are referenced by `RenderSequenceParams` and `RenderSequenceResult` (Tasks 2 and 3) but stand on their own and have independent serde semantics worth testing in isolation.

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs`
- Test: `crates/vrm-ops/tests/serde.rs`

- [ ] **Step 1.1: Write the failing serde round-trip tests**

Append to `crates/vrm-ops/tests/serde.rs`:

```rust
#[test]
fn sequence_format_serializes_snake_case() {
    let png = SequenceFormat::PngSequence;
    let mp4 = SequenceFormat::Mp4;
    let mov = SequenceFormat::Mov;
    assert_eq!(serde_json::to_string(&png).unwrap(), r#""png_sequence""#);
    assert_eq!(serde_json::to_string(&mp4).unwrap(), r#""mp4""#);
    assert_eq!(serde_json::to_string(&mov).unwrap(), r#""mov""#);

    // Round-trip
    for fmt in [SequenceFormat::PngSequence, SequenceFormat::Mp4, SequenceFormat::Mov] {
        let s = serde_json::to_string(&fmt).unwrap();
        let back: SequenceFormat = serde_json::from_str(&s).unwrap();
        assert_eq!(back, fmt);
    }
}

#[test]
fn root_transform_animation_roundtrip() {
    let r = RootTransformAnimation {
        translation_start: [0.0, 0.0, 0.0],
        translation_end:   [1.0, 0.0, 0.0],
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: RootTransformAnimation = serde_json::from_str(&s).unwrap();
    assert_eq!(back.translation_start[0], 0.0);
    assert_eq!(back.translation_end[0], 1.0);
}

#[test]
fn vrma_playback_spec_roundtrip() {
    let v = VrmaPlaybackSpec {
        vrma_handle: 7,
        start_seconds: 0.25,
    };
    let s = serde_json::to_string(&v).unwrap();
    let back: VrmaPlaybackSpec = serde_json::from_str(&s).unwrap();
    assert_eq!(back.vrma_handle, 7);
    assert_eq!(back.start_seconds, 0.25);
}

#[test]
fn sequence_frame_roundtrip() {
    let f = SequenceFrame {
        index: 12,
        timestamp_seconds: 0.4,
        path: "/tmp/seq/0012.png".into(),
        blake3: "blake3:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
    };
    let s = serde_json::to_string(&f).unwrap();
    let back: SequenceFrame = serde_json::from_str(&s).unwrap();
    assert_eq!(back.index, 12);
    assert_eq!(back.path, "/tmp/seq/0012.png");
    assert!(back.blake3.starts_with("blake3:"));
}
```

- [ ] **Step 1.2: Run tests to verify they fail**

Run: `cargo test -p vrm-ops --test serde sequence_format root_transform_animation vrma_playback_spec sequence_frame`
Expected: FAIL with `error[E0422]: cannot find struct, variant or union type 'SequenceFormat'` (and similar for the other three).

- [ ] **Step 1.3: Add the types in tools.rs**

Append to `crates/vrm-ops/src/tools.rs` after `DumpBonePositionsResult` (or wherever the file's tail group of "future ops" types sits — match the file's existing convention):

```rust
/// Output format for `render_sequence`. PNG sequence is canonical for diff;
/// MP4 and MOV are convenience formats for site display + reviewer ergonomics.
/// When MP4 or MOV is requested, the adapter MUST still emit the per-frame
/// PNGs (diff consumes them); the muxed file is in addition, not instead.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SequenceFormat {
    /// Per-frame PNG at `<output_dir>/<frame_index:04>.png`. Canonical for
    /// diff; lossless; BLAKE3-identifiable for identity short-circuit.
    PngSequence,
    /// PNG sequence + muxed `<output_dir>/sequence.mp4` (h.264 yuv420p,
    /// lossless qp=0).
    Mp4,
    /// PNG sequence + muxed `<output_dir>/sequence.mov` (Apple ProRes 4444).
    Mov,
}

/// Linear root-transform animation interpolated across `frame_count` frames
/// of a `render_sequence` call. The adapter samples translation at
/// `t = i / (frame_count - 1)` for each captured frame i ∈ [0, frame_count).
/// Duration is implicit: `frame_count / frame_hz` seconds.
///
/// Distinct from the v0.1 `AnimateRootTransformParams` because that op
/// carries its own `duration_seconds` + `fps` (single-shot animation, then
/// one render). For sequence-driven animation, those fields are redundant
/// with the sequence's own `frame_count` + `frame_hz`, so they're omitted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootTransformAnimation {
    pub translation_start: [f32; 3],
    pub translation_end: [f32; 3],
}

/// VRMA playback spec for `render_sequence`. Samples the loaded `.vrma` at
/// `t = start_seconds + (i / frame_hz)` for each captured frame i.
/// Display clock drives sampling; physics clock is internal to the adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VrmaPlaybackSpec {
    pub vrma_handle: u32,
    /// Offset into the .vrma clip where capture begins. Use 0.0 to start
    /// at clip beginning.
    pub start_seconds: f32,
}

/// One captured frame of a `render_sequence` result. The BLAKE3 hash lets
/// the diff engine short-circuit when both renderers produced byte-identical
/// frames (common for rest-pose lead-in frames).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SequenceFrame {
    pub index: u32,
    pub timestamp_seconds: f32,
    pub path: String,
    /// BLAKE3 hex of the PNG contents, prefixed `blake3:` per the content-
    /// addressing convention in `docs/operation-contract.md`.
    pub blake3: String,
}
```

- [ ] **Step 1.4: Run tests to verify they pass**

Run: `cargo test -p vrm-ops --test serde sequence_format root_transform_animation vrma_playback_spec sequence_frame`
Expected: PASS — all four tests pass.

- [ ] **Step 1.5: Commit**

```bash
git add crates/vrm-ops/src/tools.rs crates/vrm-ops/tests/serde.rs
git commit -m "$(cat <<'EOF'
feat(vrm-ops): add render_sequence supporting types

SequenceFormat (png_sequence | mp4 | mov, snake_case-serialized),
RootTransformAnimation (sequence-flavor, no duration/fps because the
sequence carries those), VrmaPlaybackSpec, SequenceFrame with BLAKE3
content addressing per the operation contract.

Per rfcs/0004-render-sequence-op.md.
EOF
)"
```

---

## Task 2: `RenderSequenceParams`

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs`
- Test: `crates/vrm-ops/tests/serde.rs`

- [ ] **Step 2.1: Write the failing serde round-trip tests**

Append to `crates/vrm-ops/tests/serde.rs`:

```rust
#[test]
fn render_sequence_params_minimal_roundtrip() {
    let p = RenderSequenceParams {
        session_id: "sess-seq".into(),
        width: 512,
        height: 512,
        output_dir: "/tmp/seq".into(),
        frame_count: 60,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        color_space: ColorSpace::Linear,
        msaa: 4,
        output_type: OutputType::Color,
        output_format: SequenceFormat::PngSequence,
        animate_root_transform: None,
        apply_vrma: None,
    };
    let s = serde_json::to_string(&p).unwrap();
    let back: RenderSequenceParams = serde_json::from_str(&s).unwrap();
    assert_eq!(back.frame_count, 60);
    assert_eq!(back.frame_hz, 30.0);
    assert!(back.animate_root_transform.is_none());
    assert!(back.apply_vrma.is_none());
    // None variants must skip serialization to keep the wire format tight.
    assert!(!s.contains("animate_root_transform"));
    assert!(!s.contains("apply_vrma"));
}

#[test]
fn render_sequence_params_with_root_animation_roundtrip() {
    let p = RenderSequenceParams {
        session_id: "sess-seq".into(),
        width: 256, height: 256,
        output_dir: "/tmp/seq".into(),
        frame_count: 30,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        color_space: ColorSpace::Srgb,
        msaa: 4,
        output_type: OutputType::Color,
        output_format: SequenceFormat::Mp4,
        animate_root_transform: Some(RootTransformAnimation {
            translation_start: [0.0, 0.0, 0.0],
            translation_end:   [1.0, 0.0, 0.0],
        }),
        apply_vrma: None,
    };
    let s = serde_json::to_string(&p).unwrap();
    let back: RenderSequenceParams = serde_json::from_str(&s).unwrap();
    let anim = back.animate_root_transform.unwrap();
    assert_eq!(anim.translation_end[0], 1.0);
    assert!(s.contains(r#""output_format":"mp4""#));
}

#[test]
fn render_sequence_params_with_vrma_roundtrip() {
    let p = RenderSequenceParams {
        session_id: "sess-seq".into(),
        width: 256, height: 256,
        output_dir: "/tmp/seq".into(),
        frame_count: 60,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        color_space: ColorSpace::Linear,
        msaa: 1,
        output_type: OutputType::Color,
        output_format: SequenceFormat::PngSequence,
        animate_root_transform: None,
        apply_vrma: Some(VrmaPlaybackSpec {
            vrma_handle: 3,
            start_seconds: 0.5,
        }),
    };
    let s = serde_json::to_string(&p).unwrap();
    let back: RenderSequenceParams = serde_json::from_str(&s).unwrap();
    let v = back.apply_vrma.unwrap();
    assert_eq!(v.vrma_handle, 3);
    assert_eq!(v.start_seconds, 0.5);
}
```

- [ ] **Step 2.2: Run tests to verify they fail**

Run: `cargo test -p vrm-ops --test serde render_sequence_params`
Expected: FAIL with `cannot find struct RenderSequenceParams`.

- [ ] **Step 2.3: Add the type in tools.rs**

Append to `crates/vrm-ops/src/tools.rs` after `SequenceFrame`:

```rust
/// Capture N frames at a fixed display Hz while advancing physics (and
/// optionally a .vrma clip or root-transform animation) between samples.
/// The adapter steps physics by `physics_dt_seconds` between each captured
/// frame (decoupled from `frame_hz` so simulation determinism and display
/// timing are configurable independently — see `docs/methodology.md`,
/// "Sequence captures").
///
/// Frames land at `output_dir/<frame_index:04>.png` for PNG sequences;
/// a single muxed file at `output_dir/sequence.mp4` (or `.mov`) for the
/// muxed formats. The diff engine consumes PNG sequences canonically;
/// muxed formats are convenience for site display.
///
/// Adapters that cannot produce sequences MUST return `-32000 Unimplemented`
/// with `data: { phase: "v1.x-sequence" }`.
///
/// Validation:
///   - If both `animate_root_transform` and `apply_vrma` are `Some`, the
///     adapter MUST return `-32602 invalid params`.
///   - If `physics_dt_seconds > 1.0 / 60.0`, the adapter SHOULD return
///     `-32602 invalid params` (methodology pin: 60 Hz physics floor).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderSequenceParams {
    pub session_id: String,
    pub width: u32,
    pub height: u32,
    pub output_dir: String,
    pub frame_count: u32,
    pub frame_hz: f32,
    pub physics_dt_seconds: f32,
    pub color_space: ColorSpace,
    pub msaa: u8,
    pub output_type: OutputType,
    pub output_format: SequenceFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate_root_transform: Option<RootTransformAnimation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_vrma: Option<VrmaPlaybackSpec>,
}
```

- [ ] **Step 2.4: Run tests to verify they pass**

Run: `cargo test -p vrm-ops --test serde render_sequence_params`
Expected: PASS (all three tests).

- [ ] **Step 2.5: Commit**

```bash
git add crates/vrm-ops/src/tools.rs crates/vrm-ops/tests/serde.rs
git commit -m "$(cat <<'EOF'
feat(vrm-ops): add RenderSequenceParams

frame_hz decoupled from physics_dt_seconds so the 60 Hz spring-bone
determinism pin holds independent of capture rate. animate_root_transform
and apply_vrma are mutually exclusive (validated at dispatch).
EOF
)"
```

---

## Task 3: `RenderSequenceResult`

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs`
- Test: `crates/vrm-ops/tests/serde.rs`

- [ ] **Step 3.1: Write the failing serde round-trip tests**

Append to `crates/vrm-ops/tests/serde.rs`:

```rust
#[test]
fn render_sequence_result_png_only_roundtrip() {
    let r = RenderSequenceResult {
        frames: vec![
            SequenceFrame {
                index: 0,
                timestamp_seconds: 0.0,
                path: "/tmp/seq/0000.png".into(),
                blake3: "blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            },
            SequenceFrame {
                index: 1,
                timestamp_seconds: 0.0333,
                path: "/tmp/seq/0001.png".into(),
                blake3: "blake3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            },
        ],
        duration_seconds: 2.0,
        actual_color_space: ColorSpace::Linear,
        frame_hz_achieved: 30.0,
        muxed_path: None,
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: RenderSequenceResult = serde_json::from_str(&s).unwrap();
    assert_eq!(back.frames.len(), 2);
    assert_eq!(back.frames[1].timestamp_seconds, 0.0333);
    assert_eq!(back.duration_seconds, 2.0);
    assert!(back.muxed_path.is_none());
    // None muxed_path must skip serialization
    assert!(!s.contains("muxed_path"));
}

#[test]
fn render_sequence_result_with_mux_roundtrip() {
    let r = RenderSequenceResult {
        frames: vec![],  // empty intentional — checks Vec serialization on the wire
        duration_seconds: 2.0,
        actual_color_space: ColorSpace::Srgb,
        frame_hz_achieved: 29.97,
        muxed_path: Some("/tmp/seq/sequence.mp4".into()),
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: RenderSequenceResult = serde_json::from_str(&s).unwrap();
    assert_eq!(back.muxed_path.as_deref(), Some("/tmp/seq/sequence.mp4"));
    assert_eq!(back.frame_hz_achieved, 29.97);
    assert!(s.contains(r#""muxed_path":"/tmp/seq/sequence.mp4""#));
}
```

- [ ] **Step 3.2: Run tests to verify they fail**

Run: `cargo test -p vrm-ops --test serde render_sequence_result`
Expected: FAIL with `cannot find struct RenderSequenceResult`.

- [ ] **Step 3.3: Add the type in tools.rs**

Append to `crates/vrm-ops/src/tools.rs` after `RenderSequenceParams`:

```rust
/// Result of `render_sequence`. `frames` lists every captured frame in
/// order; `frame_hz_achieved` may differ slightly from the requested
/// `frame_hz` if the adapter had to quantize (e.g. UniVRM coroutine timing
/// under -batchmode). `muxed_path` is `Some` when `output_format` was
/// `Mp4` or `Mov`; the diff engine ignores it — it consumes the per-frame
/// PNGs canonically.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderSequenceResult {
    pub frames: Vec<SequenceFrame>,
    pub duration_seconds: f32,
    pub actual_color_space: ColorSpace,
    pub frame_hz_achieved: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muxed_path: Option<String>,
}
```

- [ ] **Step 3.4: Run tests to verify they pass**

Run: `cargo test -p vrm-ops --test serde render_sequence_result`
Expected: PASS (both tests).

- [ ] **Step 3.5: Commit**

```bash
git add crates/vrm-ops/src/tools.rs crates/vrm-ops/tests/serde.rs
git commit -m "$(cat <<'EOF'
feat(vrm-ops): add RenderSequenceResult

frame_hz_achieved separate from requested frame_hz so adapters can report
quantization (e.g. coroutine timing jitter). muxed_path is optional;
diff engine consumes per-frame PNGs canonically.
EOF
)"
```

---

## Task 4: Document `render_sequence` in `operation-contract.md`

**Files:**
- Modify: `docs/operation-contract.md`

- [ ] **Step 4.1: Locate the `render` op section**

Run: `grep -n "^### .render.\|^## Render\|^## Capture" docs/operation-contract.md`
Expected: shows the heading for the existing single-frame `render` op section.

- [ ] **Step 4.2: Insert the `render_sequence` subsection immediately after `render`**

Append a new `### render_sequence` subsection. The exact placement is: directly after the `render` op's "Result:" block and before the next sibling section. Use this content:

```markdown
### `render_sequence`

Capture N frames at a fixed display Hz while advancing physics (and optionally a `.vrma` clip or root-transform animation) between samples. The adapter steps physics by `physics_dt_seconds` between each captured frame — display Hz and physics Hz are decoupled so the spring-bone determinism pin (60 Hz fixed step) holds independent of capture rate. See [`rfcs/0004-render-sequence-op.md`](../rfcs/0004-render-sequence-op.md) for the full design.

Adapters that have not implemented sequences return the standard Unimplemented envelope:

```
-32000 Unimplemented   data: { "phase": "v1.x-sequence" }
```

- Params: `RenderSequenceParams { session_id, width, height, output_dir, frame_count, frame_hz, physics_dt_seconds, color_space, msaa, output_type, output_format, animate_root_transform?, apply_vrma? }`
- Result: `RenderSequenceResult { frames: [{ index, timestamp_seconds, path, blake3 }], duration_seconds, actual_color_space, frame_hz_achieved, muxed_path? }`

**Validation rules adapters MUST enforce:**

- `animate_root_transform` and `apply_vrma` are mutually exclusive. Both set ⇒ `-32602 invalid params`.
- `physics_dt_seconds > 1/60` violates the spring-bone determinism methodology pin ⇒ `-32602 invalid params`.

**Output layout:**

- `output_format: png_sequence` → `<output_dir>/<frame_index:04>.png`, no muxed file.
- `output_format: mp4` → per-frame PNGs **plus** `<output_dir>/sequence.mp4` (h.264 yuv420p, lossless qp=0). `muxed_path` is set in the result.
- `output_format: mov` → per-frame PNGs **plus** `<output_dir>/sequence.mov` (Apple ProRes 4444). `muxed_path` is set in the result.

The diff engine consumes per-frame PNGs canonically regardless of `output_format`. Muxed files are convenience for site display and reviewer ergonomics.
```

- [ ] **Step 4.3: Verify the doc renders coherently**

Run: `head -400 docs/operation-contract.md`
Expected: `### render_sequence` subsection appears immediately after `### render` (or its closest analogue); existing sections intact.

- [ ] **Step 4.4: Commit**

```bash
git add docs/operation-contract.md
git commit -m "$(cat <<'EOF'
docs(operation-contract): document render_sequence op

Companion to the single-frame render op. Standard Unimplemented envelope
for adapters that haven't shipped sequences (phase: v1.x-sequence).
Validation rules (mutual exclusion + physics_dt floor) called out so
adapter implementations have one authoritative reference.
EOF
)"
```

---

## Task 5: Add methodology pins for sequences in `docs/methodology.md`

**Files:**
- Modify: `docs/methodology.md`

- [ ] **Step 5.1: Locate the existing "Spring bone" or "Determinism" section**

Run: `grep -n "^## \|^### " docs/methodology.md | head -30`
Expected: shows the file's section structure; pick the section closest to "Spring bone" / "Physics" / "Determinism".

- [ ] **Step 5.2: Append a new "Sequence captures" section**

Add at the bottom of `docs/methodology.md` (or just after the spring-bone determinism section if structure permits):

```markdown
## Sequence captures (multi-frame `render_sequence`)

Adopted by [RFC-0004](../rfcs/0004-render-sequence-op.md). These pins apply to any test plan with a `render_sequence:` block; single-frame `render:` tests are unaffected.

**Physics floor.** `physics_dt_seconds <= 1.0 / 60.0`. Anything coarser violates the spring-bone determinism pin. Adapters SHOULD reject coarser values with `-32602 invalid params`; the runner SHOULD pre-validate before dispatch.

**Sampling clock.** Sequence captures with `apply_vrma` set MUST sample the `.vrma` at `t = start_seconds + (i / frame_hz)` — display clock drives sampling; the physics clock is internal to the adapter. This decoupling exists so a test can capture at 30 Hz while running spring-bone physics at 60 Hz (two physics steps per captured frame).

**No temporal alignment.** Per-frame SSIM compares same-index frames only. The runner does NOT attempt temporal alignment (no DTW, no frame-offset search). If two adapters produce equivalent trajectories at different timings, the test's `physics_dt_seconds` or `frame_hz` is wrong, not the diff.

**Pass criteria.** Default: `mean_ssim >= temporal_ssim_threshold AND min_ssim >= temporal_ssim_threshold - 0.05`. The 0.05 single-frame relaxation acknowledges that a one-frame transient (e.g. a settle-tick offset by a single physics step) shouldn't fail an otherwise-conforming sequence. Per-test thresholds via the existing `vrm-conformance#2` mechanism.

**Worst-frame reporting.** Every sequence diff result MUST surface `worst_frame_index` so site reviewers can land on the divergent frame directly. A single bad frame in a 60-frame sequence is fine if mean SSIM holds; the threshold relaxation handles this.

**Output format.** PNG sequence is the canonical contract format. MP4/MOV are convenience formats for site display and reviewer ergonomics — the diff engine consumes the per-frame PNGs regardless. Adapters that emit only PNG sequences are spec-compliant; the bootstrap script can mux post-hoc via `ffmpeg`.
```

- [ ] **Step 5.3: Verify the doc renders coherently**

Run: `tail -80 docs/methodology.md`
Expected: new "Sequence captures" section is the last (or near-last) section; six bolded sub-rules visible.

- [ ] **Step 5.4: Commit**

```bash
git add docs/methodology.md
git commit -m "$(cat <<'EOF'
docs(methodology): pin sequence-capture conventions

60 Hz physics floor, display-clock-driven VRMA sampling, no temporal
alignment, mean+min SSIM pass criteria with 0.05 single-frame relaxation,
worst-frame reporting, PNG-sequence-canonical output. Adopted by RFC-0004.
EOF
)"
```

---

## Task 6: VMK adapter Unimplemented stub

**Files:**
- Modify: `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`

- [ ] **Step 6.1: Read the current reservedPhases dictionary**

Run: `grep -n "reservedPhases\|render_sequence\|v1.x" adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`
Expected: shows the existing `reservedPhases` static dictionary mapping method names → phase labels. Note: the VRMA phase 1 plan added `load_vrma` etc. to this dict; `render_sequence` is the next addition.

- [ ] **Step 6.2: Add `render_sequence` to the reservedPhases dict**

Edit the `reservedPhases` static. Add the entry — preserve existing entries:

```swift
    static let reservedPhases: [String: String] = [
        "set_environment":         "v1.x",
        "set_expression":          "Phase 3",
        "set_humanoid_pose":       "Phase 2",
        "set_root_transform":      "Phase 2",
        "load_vrma":               "vrma-v1",
        "apply_vrma_at_time":      "vrma-v1",
        "dump_humanoid_pose":      "vrma-v1",
        "dump_expression_weights": "vrma-v1",
        "dump_look_at_state":      "vrma-v1",
        "render_sequence":         "v1.x-sequence",
    ]
```

If the existing dict diverges from the snippet above (e.g. VRMA phase 1 hasn't landed yet or VMK has already promoted some of the VRMA ops out of reserved), preserve the actual state and only add the new `render_sequence` line.

- [ ] **Step 6.3: Build the adapter and verify it compiles**

Run: `cd adapters/vrm-metal-kit && swift build`
Expected: `Build complete!` with no errors.

- [ ] **Step 6.4: Test Unimplemented dispatch via JSON-RPC**

Run:

```bash
cd adapters/vrm-metal-kit
swift run vrm-metal-kit-adapter <<< '{"jsonrpc":"2.0","id":1,"method":"render_sequence","params":{"session_id":"x","width":256,"height":256,"output_dir":"/tmp/x","frame_count":1,"frame_hz":30.0,"physics_dt_seconds":0.01666,"color_space":"Linear","msaa":1,"output_type":"Color","output_format":"png_sequence"}}'
```

Expected: stdout contains `"error":{"code":-32000` and `"phase":"v1.x-sequence"`.

- [ ] **Step 6.5: Commit**

```bash
git add adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift
git commit -m "$(cat <<'EOF'
feat(adapters/vrm-metal-kit): add render_sequence stub returning Unimplemented

Returns -32000 with phase: "v1.x-sequence". Real implementation lands in
render-sequence phase 5; vrm-metal-kit is the first real adapter per the
rollout plan.
EOF
)"
```

---

## Task 7: three-vrm adapter Unimplemented stub

**Files:**
- Modify: `adapters/three-vrm/src/operations.ts`

- [ ] **Step 7.1: Identify the existing dispatcher pattern**

Run: `grep -n "method\|case \"\|switch\|Unimplemented\|VRMA_V1_OPS" adapters/three-vrm/src/operations.ts | head -30`
Expected: shows the existing method-name dispatcher and (post-VRMA-phase-1) the `VRMA_V1_OPS` set pattern.

- [ ] **Step 7.2: Add the `render_sequence` case to the Unimplemented branch**

Edit `adapters/three-vrm/src/operations.ts`. The simplest pattern is a dedicated branch that matches the existing reserved-op shape used by VRMA phase 1:

```typescript
// Before the unknown-method fallthrough, add:
if (method === "render_sequence") {
  return {
    error: {
      code: -32000,
      message: "Unimplemented",
      data: { phase: "v1.x-sequence" },
    },
  };
}
```

If the file uses a `Set` or a `Map` for reserved phases, add `render_sequence: "v1.x-sequence"` to that data structure instead. Match the file's existing convention.

- [ ] **Step 7.3: Build and test**

Run: `cd adapters/three-vrm && npm run build && npm test`
Expected: build succeeds, existing tests pass.

- [ ] **Step 7.4: Add an Unimplemented test**

Find the test file that exercises Unimplemented elsewhere (`grep -rn "Unimplemented\|render_sequence" adapters/three-vrm/test/`). Append:

```typescript
it("returns Unimplemented for render_sequence with phase v1.x-sequence", async () => {
  const resp = await dispatch({
    jsonrpc: "2.0",
    id: 1,
    method: "render_sequence",
    params: {
      session_id: "x",
      width: 256, height: 256,
      output_dir: "/tmp/x",
      frame_count: 1,
      frame_hz: 30.0,
      physics_dt_seconds: 1.0 / 60.0,
      color_space: "Linear",
      msaa: 1,
      output_type: "Color",
      output_format: "png_sequence",
    },
  });
  assert.equal(resp.error.code, -32000);
  assert.equal(resp.error.data.phase, "v1.x-sequence");
});
```

- [ ] **Step 7.5: Run the new test**

Run: `cd adapters/three-vrm && npm test`
Expected: new test passes; existing tests still pass.

- [ ] **Step 7.6: Commit**

```bash
git add adapters/three-vrm/src/operations.ts adapters/three-vrm/test/
git commit -m "$(cat <<'EOF'
feat(adapters/three-vrm): add render_sequence stub returning Unimplemented

Returns -32000 with phase: "v1.x-sequence". Real implementation will land
in render-sequence phase 6 by wrapping the existing render/screenshot
loop with per-frame springBoneManager.update(dt).
EOF
)"
```

---

## Task 8: godot-vrm adapter Unimplemented stub (via vrm-godot-shim)

**Files:**
- Modify: `crates/vrm-godot-shim/src/bridge.rs`

- [ ] **Step 8.1: Identify the shim's method dispatch**

Run: `grep -n "method\|Unimplemented\|-32000\|vrma-v1" crates/vrm-godot-shim/src/bridge.rs | head -20`
Expected: shows the shim's JSON-RPC dispatch table or match arms, including (post-VRMA-phase-1) the VRMA ops' Unimplemented branch.

- [ ] **Step 8.2: Add `render_sequence` to the Unimplemented branch**

Edit `crates/vrm-godot-shim/src/bridge.rs`. Locate the dispatch (likely a `match method.as_str() { ... }`). Add a branch alongside (or extending) the existing VRMA Unimplemented branch:

```rust
"render_sequence" => {
    Ok(JsonRpcError::unimplemented_with_phase("v1.x-sequence"))
}
```

If `unimplemented_with_phase` doesn't exist on the existing error helper, follow whatever helper the shim already uses for `-32000` + `phase` envelope.

- [ ] **Step 8.3: Build the shim**

Run: `cargo build -p vrm-godot-shim`
Expected: build succeeds.

- [ ] **Step 8.4: Add a shim-level test**

Append to `crates/vrm-godot-shim/src/bridge.rs` (or a sibling tests file the crate uses):

```rust
#[test]
fn render_sequence_returns_unimplemented_with_phase_v1_x_sequence() {
    let req = r#"{"jsonrpc":"2.0","id":1,"method":"render_sequence","params":{"session_id":"x","width":256,"height":256,"output_dir":"/tmp/x","frame_count":1,"frame_hz":30.0,"physics_dt_seconds":0.01666,"color_space":"Linear","msaa":1,"output_type":"Color","output_format":"png_sequence"}}"#;
    let resp = dispatch(req).unwrap();
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["error"]["code"], -32000);
    assert_eq!(v["error"]["data"]["phase"], "v1.x-sequence");
}
```

Adjust the `dispatch` call to match the actual entry point in `bridge.rs`.

- [ ] **Step 8.5: Run the test**

Run: `cargo test -p vrm-godot-shim render_sequence_returns_unimplemented`
Expected: PASS.

- [ ] **Step 8.6: Commit**

```bash
git add crates/vrm-godot-shim/src/bridge.rs
git commit -m "$(cat <<'EOF'
feat(vrm-godot-shim): add render_sequence stub returning Unimplemented

Returns -32000 with phase: "v1.x-sequence" at the shim level without
round-tripping through Godot. Real implementation lands in render-sequence
phase 6 by extending the GDScript conformance script's render loop.
EOF
)"
```

---

## Task 9: UniVRM adapter Unimplemented stub

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs`

UniVRM's real `render_sequence` implementation lands in render-sequence Phase 7, bundled with the deferred L4-PlayMode work (FastSpringBone gates on `Application.isPlaying`; the per-frame loop needs PlayMode anyway). For Phase 1 we just ensure UniVRM's batch dispatcher reports Unimplemented for the op name, mirroring the other adapters.

- [ ] **Step 9.1: Identify the existing op dispatcher**

Run: `grep -n "method\|case \|Unimplemented\|-32000\|RunBatch" adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs | head -30`
Expected: shows the dispatch — UniVRM batches via filesystem (RFC-0003); each test case method-dispatches inside the batch.

- [ ] **Step 9.2: Add `render_sequence` to the Unimplemented branch**

In `Conformance.cs`, find the method-name dispatch (likely a switch statement or method-name lookup in the batch loop). Add a branch:

```csharp
case "render_sequence":
    return UnimplementedError("v1.x-sequence");
```

If `UnimplementedError` doesn't exist, follow whatever helper Conformance.cs already uses for `-32000` + `phase` envelopes.

- [ ] **Step 9.3: Build via Unity batch (or skip if Unity isn't available locally)**

Run: `adapters/univrm/launcher.sh --validate-only` (if this flag exists; otherwise rely on CI build-validate step).
Expected: compilation OK. If Unity isn't installed locally, note the omission in the commit body — CI build-validate will catch a syntax break.

- [ ] **Step 9.4: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs
git commit -m "$(cat <<'EOF'
feat(adapters/univrm): add render_sequence stub returning Unimplemented

Returns -32000 with phase: "v1.x-sequence". Real implementation bundled
with the deferred L4-PlayMode follow-up — render-sequence phase 7 — since
FastSpringBone requires Application.isPlaying anyway.
EOF
)"
```

---

## Task 10: Integration sanity test — describe catalog includes `render_sequence`

**Files:**
- Modify or create: `adapters/three-vrm/test/describe.test.ts` (or wherever describe is tested in this adapter)

- [ ] **Step 10.1: Add a describe-catalog assertion**

Find the existing `describe` test (`grep -rn "describe" adapters/three-vrm/test/`). Extend it (or add a new test) so it asserts `render_sequence` appears in the catalog:

```typescript
it("describe catalog exposes render_sequence", async () => {
  const catalog = await runCli(["describe", "--format", "json"]);
  const parsed = JSON.parse(catalog);
  const methods = new Set(parsed.methods?.map((m: any) => m.name) ?? []);
  assert(methods.has("render_sequence"), "describe catalog missing render_sequence");
});
```

If `describe` is wired differently in this adapter, mirror its existing shape — the assertion is "method-name list from `describe --format json` includes `render_sequence`."

- [ ] **Step 10.2: Run the test**

Run: `cd adapters/three-vrm && npm test`
Expected: new describe test passes alongside existing tests.

- [ ] **Step 10.3: Commit**

```bash
git add adapters/three-vrm/test/
git commit -m "$(cat <<'EOF'
test(adapters/three-vrm): verify describe catalog exposes render_sequence

Catches future regressions where the op gets dropped from the published
operation catalog. Mirrors the VRMA phase 1 describe-catalog test pattern.
EOF
)"
```

---

## Task 11: Workspace cleanup — fmt + clippy + full test

**Files:**
- (none touched directly; cleanup pass)

- [ ] **Step 11.1: Run cargo fmt across workspace**

Run: `cargo fmt --all`
Expected: no changes (if everything was already formatted) OR formatter applies fixes.

- [ ] **Step 11.2: Run clippy with -D warnings**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: zero warnings, zero errors. If clippy flags anything in the new types (e.g. `needless_borrow`, `derivable_impls`, `large_enum_variant` if `RenderSequenceParams` is bigger than expected), fix inline.

- [ ] **Step 11.3: Run the full test suite**

Run: `cargo test --workspace`
Expected: all tests pass, including the new serde round-trip tests for `render_sequence` types.

- [ ] **Step 11.4: Run three-vrm tests**

Run: `cd adapters/three-vrm && npm test && cd -`
Expected: all tests pass (including the new describe-catalog assertion from Task 10).

- [ ] **Step 11.5: Commit any fmt/clippy fixes (if needed)**

If fmt or clippy made changes:

```bash
git add -u
git commit -m "$(cat <<'EOF'
chore: cargo fmt + clippy clean-up after render_sequence op surface

Phase 1 of render_sequence rollout. Zero clippy warnings, zero fmt diffs
across the workspace.
EOF
)"
```

If no changes were needed, skip this commit.

---

## Phase 1 completion checklist

- [ ] All 6 new types in `crates/vrm-ops/src/tools.rs` (`SequenceFormat`, `RootTransformAnimation`, `VrmaPlaybackSpec`, `SequenceFrame`, `RenderSequenceParams`, `RenderSequenceResult`) with serde round-trip tests passing
- [ ] `docs/operation-contract.md` documents `render_sequence` alongside `render`
- [ ] `docs/methodology.md` documents the sequence-capture pins (60 Hz physics floor, display-clock VRMA sampling, no temporal alignment, mean+min SSIM pass criteria, worst-frame reporting, PNG-sequence-canonical)
- [ ] VMK adapter declares `render_sequence` in its `reservedPhases` map (Unimplemented with `phase: "v1.x-sequence"`)
- [ ] three-vrm adapter dispatches `render_sequence` to its Unimplemented branch
- [ ] godot-vrm adapter (via vrm-godot-shim) dispatches `render_sequence` to Unimplemented
- [ ] UniVRM adapter dispatches `render_sequence` to Unimplemented (real impl bundled with Phase 7 L4-PlayMode work)
- [ ] `describe --format json` output includes `render_sequence`
- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo test --workspace` green
- [ ] `cd adapters/three-vrm && npm test` green

After this phase, the op contract is published and stable. Phase 2 (diff engine + manifest schema + runner integration) builds on this surface — that plan file is `docs/superpowers/plans/2026-05-18-render-sequence-phase2-diff-and-manifest.md` (not yet written; will be drafted after this phase merges and any RFC-0004 amendments are applied).
