# RFC 0004: `render_sequence` op — multi-frame capture for temporal conformance

- **Status:** Draft
- **Author(s):** Paul Flynn
- **Date:** 2026-05-18

## Summary

Add a `render_sequence` operation to the cross-adapter op contract. The op captures N frames at a fixed Hz while the adapter advances physics (and optionally a `.vrma` clip or root-transform animation) between samples. Diff is extended with a `temporal_diff` mode that runs SSIM per-frame and aggregates (mean, p95, worst-frame). Manifest schema gains a `sequence` entry kind alongside the existing `image` kind. This closes the "single frame loses most of the signal" gap for spring-bone swing tests, VRMA pose retarget, and any other test whose conformance is fundamentally temporal.

## Motivation

The current op surface captures one PNG per test. That choice was correct for Phase 1 — MToon material math, lighting math, and rest-pose geometry are all single-frame concerns. But the conformance corpus has grown two clusters where single-frame capture actively hides the conformance question:

1. **Spring-bone swing-sweep** (18 tests). Each variant changes one of `{stiffness, drag, gravityScale, gravityDir, hitRadius, ...}` and animates the root transform to excite the chain. The current convention captures one post-animation frame. Most of the *informative* signal — how the chain *moves through time* — is gone. Two adapters can produce identical post-settle frames via wildly different trajectories. The pass-rate numbers in `docs/findings.md` for swing tests are correspondingly low-information.
2. **VRMA pose retarget** (planned, `vrma-phase4` onwards). The "arms twist inside-out during walking" failure mode reported upstream (VMK#165) is *only* visible across a sequence. A single sampled frame of a multi-bone walk can look correct while the underlying retarget normalization is broken.

Adding a frame-sequence capture closes both. The same machinery also unlocks visual diff for `.vrma` playback fidelity, expression timing, and any future locomotion-style tests donors may ask for.

There is a secondary motivation: **site usability**. A scrubber timeline that lets a reviewer step through frames and see the SSIM heatmap is materially more useful than a single golden PNG for any test where the question is "where did the renderers diverge." This is consistent with how `glTF-Render-Fidelity` presents temporal results and will help with future donation.

## Detailed design

### Op signature

New op in `crates/vrm-ops/src/tools.rs`. Mirrors the existing `RenderParams` shape with sequence-specific additions.

```rust
/// Capture N frames at a fixed display Hz while advancing physics (and
/// optionally a .vrma clip or root-transform animation) between samples.
/// The adapter steps physics by `physics_dt_seconds` between each captured
/// frame (decoupled from `frame_hz` so simulation determinism and display
/// timing are configurable independently — see methodology hazard).
///
/// Frames land at `output_dir/<frame_index:04>.png` for PNG sequences;
/// a single muxed file at `output_dir/sequence.mp4` (or `.mov`) for the
/// muxed formats. The diff engine consumes PNG sequences canonically;
/// muxed formats are convenience for site display.
///
/// Adapters that cannot produce sequences MUST return -32000 Unimplemented
/// with `data: { phase: "v1.x-sequence" }`.
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
    /// Optional animation source. Exactly one of (or neither, for
    /// pure-physics-settle sequences):
    ///   - `animate_root_transform`: linear root translation
    ///     interpolated across `frame_count` frames
    ///   - `apply_vrma`: sample the loaded .vrma at t = i/frame_hz for
    ///     each captured frame i ∈ [0, frame_count)
    /// If both are present, the adapter MUST return -32602 invalid params.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate_root_transform: Option<RootTransformAnimation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_vrma: Option<VrmaPlaybackSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SequenceFormat {
    /// Per-frame PNG, `<frame_index:04>.png`. Canonical for diff.
    PngSequence,
    /// PNG sequence + muxed MP4 (h.264 yuv420p, lossless qp=0). MP4 is
    /// convenience for site display; diff still consumes the PNGs.
    Mp4,
    /// PNG sequence + muxed Apple ProRes 4444 .mov. Same diff semantics
    /// as Mp4. Useful for VMK-side review since QuickTime is native.
    Mov,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootTransformAnimation {
    pub translation_start: [f32; 3],
    pub translation_end: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VrmaPlaybackSpec {
    pub vrma_handle: u32,
    pub start_seconds: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderSequenceResult {
    pub frames: Vec<SequenceFrame>,
    pub duration_seconds: f32,
    pub actual_color_space: ColorSpace,
    pub frame_hz_achieved: f32,
    /// Path to the muxed file when `output_format != PngSequence`. None
    /// when format is PngSequence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muxed_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SequenceFrame {
    pub index: u32,
    pub timestamp_seconds: f32,
    pub path: String,
    /// BLAKE3 hex of the PNG file contents. Diff engine uses this to
    /// short-circuit when both renders produced byte-identical frames.
    pub blake3: String,
}
```

Three things worth flagging:

- **`frame_hz` is decoupled from `physics_dt_seconds`**. A test can request 30 captured frames at 30 Hz (1.0 s of footage) while running spring-bone physics at 60 Hz internally — the adapter steps physics twice per captured frame. This avoids forcing every test to render at full 60 Hz, while preserving the spring-bone determinism methodology pin (60 Hz fixed step). Sequences MUST respect `physics_dt_seconds <= 1/60` or trip a methodology violation.
- **Animation sources are mutually exclusive.** A sequence can be pure-physics (gravity-only swing), or root-animated, or VRMA-driven, but not two at once. This keeps the conformance question well-defined ("what excited the chain?") and matches the existing `animate_root_transform` semantics.
- **BLAKE3 per frame** for content addressing and identity short-circuiting. Same content-addressing rule as the rest of the contract.

### Diff: `temporal_diff` mode

`crates/vrm-diff-engine` gains a `temporal_diff` entry point. Per-frame SSIM is the existing single-frame metric; aggregation is the new part.

```rust
pub struct TemporalDiffResult {
    pub frame_count: u32,
    pub frame_count_compared: u32,    // when sequences differ in length, use min
    pub per_frame: Vec<FrameDiff>,
    pub mean_ssim: f64,
    pub p95_ssim: f64,
    pub min_ssim: f64,
    pub worst_frame_index: u32,
    pub frame_count_match: bool,      // false ⇒ length divergence flagged
    pub passed: bool,                 // mean ≥ threshold AND min ≥ threshold - 0.05
}

pub struct FrameDiff {
    pub index: u32,
    pub ssim: f64,
    pub identity_match: bool,         // true when BLAKE3 hashes matched
}
```

Pass criteria (default; per-test overrides via `.test.yaml`):
- `mean_ssim >= test.temporal_ssim_threshold` (default 0.90)
- `min_ssim >= test.temporal_ssim_threshold - 0.05` (no single frame collapses)
- `frame_count_match == true`

The 0.05 single-frame relaxation acknowledges that a one-frame transient (e.g., a settle-tick offset by a single physics step) shouldn't fail an otherwise-conforming sequence. This mirrors the spirit of the per-test threshold system in [vrm-conformance#2](https://github.com/arkavo-org/vrm-conformance/issues/2).

### Test plan schema extension

`crates/vrm-test-plan` adds an optional `render_sequence` block. When present, the runner's `execute-test-plan` calls `render_sequence` instead of (or in addition to) `render`. Tests that today drive `animate_root_transform` followed by `render` migrate cleanly: the `animate_root_transform` parameters fold into the new `render_sequence.animate_root_transform` field.

```yaml
# example: swing_springbone_stiffness_0.1.test.yaml (post-migration)
id: swing_springbone_stiffness_0_1
asset: swing_springbone_stiffness_0_1.vrm
camera: {...}
lighting: {...}
render_sequence:
  frame_count: 60
  frame_hz: 30.0
  physics_dt_seconds: 0.01666  # 60 Hz physics, 2 steps per captured frame
  output_format: png_sequence
  animate_root_transform:
    translation_start: [0.0, 0.0, 0.0]
    translation_end:   [1.0, 0.0, 0.0]
diff:
  temporal_ssim_threshold: 0.92
  exclude_frames: []   # optional; useful if frame 0 is rest-pose noise
```

The existing single-frame `render:` block stays valid for tests where temporal capture isn't useful (MToon material math, lighting math, etc.). Both blocks present at once is invalid (runner rejects with a clear error); a test plan is single-frame OR sequence.

### Manifest schema extension

`goldens/manifest.json` entries gain an optional `kind` discriminator (defaulting to `image` for back-compat):

```json
{
  "test_id": "swing_springbone_stiffness_0_1",
  "renderer_name": "vrm-metal-kit",
  "renderer_version": "0.15.2",
  "kind": "sequence",
  "frame_count": 60,
  "frame_hz": 30.0,
  "frames": [
    { "index": 0, "image_url": "s3://.../000.png", "blake3": "..." },
    { "index": 1, "image_url": "s3://.../001.png", "blake3": "..." },
    ...
  ],
  "muxed_url": "s3://.../sequence.mp4",
  "muxed_blake3": "...",
  "host": {...},
  "git_hash": "..."
}
```

`crates/vrm-s3`'s `validate-manifest` binary learns both kinds. Existing single-frame entries keep their flat shape — the `kind: "image"` default means no migration of historical entries is needed.

### Methodology pins (sequence-specific)

Append to `docs/methodology.md`:

- Sequence captures MUST use `physics_dt_seconds <= 1/60`. Anything coarser violates the spring-bone determinism pin.
- Sequence captures with `apply_vrma` set MUST sample the .vrma at `t = i / frame_hz` (not `t = i * physics_dt_seconds`). The display clock drives sampling; the physics clock is internal to the adapter.
- Per-frame SSIM compares same-index frames only — the runner does NOT attempt temporal alignment (no DTW, no frame-offset search). If two adapters produce equivalent trajectories at different timings, the test's `physics_dt_seconds` or `frame_hz` is wrong, not the diff.
- Worst-frame index is reported to the site for hover-scrubber highlight. A single bad frame in a 60-frame sequence is fine if mean SSIM holds; the threshold relaxation handles this.

### Adapter implementation guidance (non-normative)

Each adapter wraps its existing render loop. Sketch per engine:

- **vrm-metal-kit**: existing offscreen Metal pipeline already renders to texture. Wrap in a frame-count loop, call `springBoneSystem.update(dt)` between captures, write PNG-per-frame. Optional `ffmpeg` subprocess for mux (acceptable shell-out since the muxed file is convenience, not diff input).
- **three-vrm**: Playwright already runs a render loop. Add a `for (let i = 0; i < frame_count; i++) { springBoneManager.update(dt); renderer.render(); await page.screenshot(...) }` block. Mux via Node `ffmpeg-static`.
- **godot-vrm**: VRMSecondary is already manually stepped at L4. Wrap in the frame-count loop. Godot's `ImageTexture.get_image().save_png()` for per-frame; `ffmpeg` shell-out for mux.
- **UniVRM** (PlayMode): the deferred L4-PlayMode follow-up already needs PlayMode for FastSpringBone to function. `render_sequence` lands as part of that work — `PhysicsDriver.Process(dt)` already exists, `Capture.cs` already captures one frame, so the loop is the only new code.

There is no requirement that adapters mux MP4/MOV themselves. PNG sequence is the contract minimum; the bootstrap script can mux post-hoc with `ffmpeg` if any adapter only emits PNGs.

### Failure modes

| Failure | Handling |
|---|---|
| Adapter doesn't support sequences | Return `-32000 Unimplemented` with `phase: "v1.x-sequence"`. Runner records the test as `Unimplemented` in the manifest (not a hard failure). |
| Both `animate_root_transform` and `apply_vrma` set | Return `-32602 invalid params`. Runner records as malformed; CI fails. |
| `physics_dt_seconds > 1/60` | Adapter SHOULD reject with `-32602`. Runner SHOULD pre-validate before dispatch. |
| Frame-count mismatch in diff | Mark `frame_count_match: false`, fail the test, report both lengths in the diff JSON. |
| Single bad frame (SSIM < threshold - 0.05) | Fail the test; surface `worst_frame_index` to the site so the reviewer lands on the divergent frame. |
| Identity short-circuit (BLAKE3 match) | Mark `identity_match: true`, skip SSIM compute. |
| Disk pressure (60 frames × 76 tests × 4 renderers = ~18,240 PNGs per bootstrap) | Mitigated by S3 push: PNGs go to S3 with content-addressed paths; `goldens-cache/` is gitignored anyway. Site fetches per-frame URLs lazily during scrub. |

### Migration path

- Phase 1 (op surface) is non-breaking: existing tests keep working, new tests can opt into `render_sequence`.
- Phase 2 (diff + manifest) extends schemas with defaults that preserve back-compat for single-frame entries.
- Phase 3 (mock renderer) gives the diff engine a deterministic reference for testing.
- Phase 4 (asset generator) introduces sequence-capable test plans behind a flag; existing plans untouched.
- Phase 5 (first real adapter) migrates the 18 swing-sweep tests from single-frame to sequence. This is the moment swing-sweep numbers become informative.
- Phase 6–7 (remaining adapters) closes the gap; UniVRM bundles with the existing L4-PlayMode follow-up.
- Phase 8 (site) ships the scrubber UI; previously-rendered single-frame goldens remain unchanged.

At no phase does an existing renderer break or a manifest need rewriting.

## Alternatives considered

### Render only the worst frame instead of a sequence

Heuristically identify the frame most likely to differ (e.g. midpoint of swing-sweep animation) and render only that. Cheap, no schema change.

Rejected: defeats the purpose. The conformance question for swing tests is "do these renderers integrate spring physics the same way over time?" — there is no single-frame proxy for that. The "arms twist during walking" failure mode is precisely the case a worst-frame heuristic would miss because the heuristic doesn't know which frame to pick.

### Use video-only output (MP4) and run perceptual video diff (VMAF, etc.)

Skip PNG sequences; have adapters emit MP4 directly; diff using VMAF or similar perceptual video metrics.

Rejected for three reasons. (1) VMAF and friends are tuned for natural video, not non-PBR toon shading; they would import the same domain-mismatch problems we already wrestle with for tone-mapping. (2) Codec-level differences between renderers' encoders (e.g. h.264 quantization between Apple VideoToolbox and ffmpeg libx264) introduce a confound that has nothing to do with renderer conformance. (3) BLAKE3 short-circuit only works on byte-identical content, which lossless PNG sequences preserve and lossy video does not. PNG-sequence-as-canonical with optional convenience mux is the right shape.

### Animate root transform across the same N "render" calls instead of a new op

Loop `animate_root_transform(t_i)` then `render(out_i)` in the runner, no adapter changes.

Rejected: works for root-transform animation but not for VRMA playback (which is per-clip-time, not per-translation-step) and forces N×op-call latency where one op-call suffices. More importantly, the existing `render` op writes one file at one resolution per call — having the runner stitch N independent renders into a "sequence" doesn't actually capture the sequence-vs-frame distinction in the contract. The diff engine, the manifest, and the site all need the sequence concept regardless; might as well let adapters internalize the loop.

### Skip MOV format; ship only PNG sequence + MP4

Three formats is one more than minimum.

Reluctantly kept MOV. VMK is the macOS-native adapter; ProRes .mov files preview natively in QuickTime/Finder during review without re-encode round-trips. The marginal cost is small (an extra `ffmpeg -c:v prores_ks` invocation) and the reviewer ergonomics are non-trivial. If MOV proves unused after Phase 5 we'll drop it.

## Open questions

1. **Storage cost at scale**: 60 frames × ~80 tests × 4 renderers × ~50KB/PNG ≈ 1 GB per bootstrap snapshot. With S3 lifecycle policies this is fine, but it's worth a back-of-envelope check before Phase 5. If it becomes painful we can drop frame_count for non-swing tests, or downsample to 320×320 for sequence captures (whole-frame SSIM is robust to resolution).
2. **Per-frame timing jitter on UniVRM PlayMode**: Unity's coroutine-driven `WaitForEndOfFrame` may not deliver exact 1/60 s timing on a `-batchmode` invocation. We may need to drive the loop via `EditorApplication.update` or a tighter physics callback. Will surface in Phase 7 testing.
3. **Site scrubber UX**: should the timeline default to "play through at 30 Hz" or "freeze on worst-frame"? Lean toward worst-frame freeze with optional play — that matches the "what diverged" question. To be settled in Phase 8.
4. **MP4 muxing for VRMA-driven sequences with audio**: VRMA does not carry audio. Settled — no audio track.

## References

- [`docs/operation-contract.md`](../docs/operation-contract.md) — the cross-adapter contract this op extends.
- [`docs/methodology.md`](../docs/methodology.md) — methodology pins this RFC adds to.
- [`docs/findings.md`](../docs/findings.md) — current swing-sweep low-information cluster motivating this work.
- [RFC-0003](./0003-engine-idiom-divergence.md) — adapter-shape autonomy precedent; per-adapter sequence implementations follow this principle.
- [vrm-conformance#2](https://github.com/arkavo-org/vrm-conformance/issues/2) — per-test SSIM threshold system this extends to temporal aggregation.
- [vrm-conformance#10](https://github.com/arkavo-org/vrm-conformance/issues/10) — multi-bone VRMA retarget; first concrete consumer of `apply_vrma` mode beyond swing-sweep.
- [VMK#165](https://github.com/arkavo-org/VRMMetalKit/issues/165) — "arms twist inside-out during walking"; failure mode that is only visible across a sequence.
- [Khronos glTF-Render-Fidelity](https://github.com/KhronosGroup/glTF-Render-Fidelity) — methodology compatibility target; sequence display patterns inform Phase 8 site UX.
