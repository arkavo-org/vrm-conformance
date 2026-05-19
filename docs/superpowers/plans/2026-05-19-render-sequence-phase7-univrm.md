# `render_sequence` Phase 7 — UniVRM PlayMode Implementer

> **For agentic workers:** Use superpowers:subagent-driven-development. Phases 1–6 are landed (latest `b9c77ff`). UniVRM is structurally different from the other adapters (RFC-0003 batched filesystem contract, not per-op JSON-RPC), but the PlayMode batch infrastructure for spring-bone physics is **already in place** — Phase 7 extends it for sequence capture.

**Goal:** Make UniVRM the fourth (and consortium-reference) real implementer of `render_sequence`. The existing `BatchRunner.RunBatchInPlayMode` (`adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/PlayMode/BatchRunner.cs`) already renders single frames with real spring-bone physics in PlayMode. Phase 7 extends it to branch on `t.render_sequence != null` and run the per-frame loop, mirroring `PhysicsDriver.AnimateRootTransform`'s pattern but capturing per-frame PNGs.

**Pre-existing infrastructure (already landed before Phase 7):**

- `BatchRunner.RunBatchInPlayMode` — PlayMode entry point invoked via `launcher.sh` (PlayMode is the default; `L3_EDITMODE=1` opts back to EditMode).
- `PhysicsDriver.Process(dt)` — per-step spring-bone advancement (line 71/115 of PhysicsDriver.cs).
- `PhysicsDriver.AnimateRootTransform` — frame loop with root lerp + Process(dt) (line 82). This is the **direct template** for the sequence loop.
- `Capture.Render(cam, output, outputPath)` — single-frame PNG export.
- `SceneSetup.GltfToUnity` — coordinate transform.

**Out of scope for Phase 7:**

- `apply_vrma` per-frame in UniVRM (reject, same as Phases 5+6).
- MP4/MOV mux in UniVRM (reject non-PNG, same as Phases 5+6).
- `bootstrap-goldens.sh` sequence-path manifest writing — deferred to a Phase 5b-style follow-up that touches shared infra.

**Architecture:**

- **Rust runner side** (`crates/vrm-runner/src/execute_batch.rs`): `BatchTestEntry` gains `render_sequence: Option<RenderSequenceBlock>` so the field crosses the manifest JSON. `ResultEntry` (the per-test entry the adapter writes back via NDJSON) gains optional sequence fields so the runner can recognize sequence-shape results.
- **C# adapter side**:
  - `Manifest.cs` gains `RenderSequenceDto` + `RenderSequenceFrameDto` and adds `render_sequence` to `TestEntryDto`. `EntryDto` gains the sequence fields (or a sibling).
  - `PhysicsDriver.cs` gains a new `RenderSequence` helper that loops `frame_count` times, per frame: lerp root translation, `runtime.SpringBone.Process(physics_dt)`, `yield return null` so Unity renders, `Capture.Render` to `<output_dir>/<test_id>_frames/<NNNN>.png`. (Actually the loop needs to be a coroutine because `yield` is required for Unity render — it lives in `BatchRunner.cs` as a private method, with PhysicsDriver only providing the step call.)
  - `BatchRunner.RunBatchInPlayMode` / `RenderOneCo` branch on `t.render_sequence != null` to dispatch into the new sequence path.

**BLAKE3**: UniVRM populates `blake3: "blake3:" + 64*"0"` per frame; runner re-hashes (Phase 5 Task 2 contract, already wired for sequence entries from per-op adapters — the runner-side rehash code path handles batched-mode results too if we surface frames in `ResultEntry`).

Actually a subtlety: the existing `execute_test_batch` runner code consumes the adapter's `results.ndjson` as `ResultEntry { output_path, blake3, ... }`. For sequence entries, the runner needs to know to look for `frames` and re-hash each one. **In Phase 7 we extend `ResultEntry` and the consumer.**

**Tech stack:** Rust (runner) + C# (Unity 6 / UniVRM v0.131.0 / Built-in RP). No upstream UniVRM revision change. No new Swift/Node/Godot work.

**Spec:** [`rfcs/0004-render-sequence-op.md`](../../../rfcs/0004-render-sequence-op.md). The batched filesystem contract is RFC-0003.

---

## File structure

**Modify:**
- `crates/vrm-runner/src/execute_batch.rs` — `BatchTestEntry.render_sequence` + `ResultEntry.frames` + sequence-aware result handling
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Manifest.cs` — `RenderSequenceDto`, `RenderSequenceFrameDto`, fields on `TestEntryDto` + `EntryDto`
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/PhysicsDriver.cs` — minor: expose a single-step `Process(dt)` wrapper if needed (the existing internal call may already be reachable; verify)
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/PlayMode/BatchRunner.cs` — branch in `RenderOneCo`, new `RenderSequenceCo` helper
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs` — EditMode path detects sequence plans and rejects with a clear error (sequence requires PlayMode)
- Existing tests touching `BatchTestEntry` constructors: add `render_sequence: None`

**Create:**
- `crates/vrm-runner/tests/render_sequence_e2e_univrm.rs` — `#[ignore]`-gated end-to-end (requires Unity 6 + PlayMode)

---

## Task 1: Rust side — `BatchTestEntry.render_sequence` + `ResultEntry.frames`

**Files:**
- Modify: `crates/vrm-runner/src/execute_batch.rs`

- [ ] **Step 1.1: Add field to `BatchTestEntry`**

Add `render_sequence: Option<RenderSequenceBlock>` after `animation`. Import the type from `vrm_test_plan`. Use `#[serde(skip_serializing_if = "Option::is_none")]` so the manifest stays clean for non-sequence tests.

Update `build_manifest` to populate the field from `plan.render_sequence.clone()`.

- [ ] **Step 1.2: Extend `ResultEntry` with sequence fields**

```rust
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
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
    // NEW: sequence-shape results carry these instead of output_path/blake3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<Vec<SequenceFrameEntry>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_hz_achieved: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct SequenceFrameEntry {
    pub index: u32,
    pub timestamp_seconds: f32,
    pub path: String,
    pub blake3: String,
}
```

The `Serialize` derive is needed if the runner ever re-emits results elsewhere; if compilation complains it's not used, drop it.

- [ ] **Step 1.3: Re-hash sequence frames in the batch result handler**

Find where `LocalManifestEntry` is built from `ResultEntry` (around line ~415). When `entry.frames.is_some()`, iterate frames, re-hash each PNG's bytes via `blake3::hash`, overwrite `frame.blake3`. Mirror the Phase 5 Task 2 pattern (centralized in `rehash_frames` in execute.rs) — consider extracting that helper to a shared module if reuse is non-trivial. For Phase 7, an inline re-hash loop is acceptable.

- [ ] **Step 1.4: Update callsites that construct `BatchTestEntry { ... }` literally**

`grep -rn "BatchTestEntry {" crates/vrm-runner/tests/` should find any test fixtures that need `render_sequence: None`.

- [ ] **Step 1.5: Build + test + clippy**

```
cargo build -p vrm-runner
cargo test -p vrm-runner
cargo clippy --workspace --all-targets -- -D warnings
```

All existing tests still pass. Sequence-side semantics are exercised by Task 4's E2E.

- [ ] **Step 1.6: Commit**

```bash
git add crates/vrm-runner/src/execute_batch.rs
git commit -m "$(cat <<'EOF'
feat(vrm-runner): batch manifest + result shape gain render_sequence fields

BatchTestEntry serializes render_sequence (RenderSequenceBlock) when
present so UniVRM's PlayMode batch can branch on it. ResultEntry gains
optional frames / duration_seconds / frame_hz_achieved so sequence
results round-trip through the NDJSON. Runner re-hashes per-frame
BLAKE3 from on-disk PNG bytes the same way single-PNG entries are
handled (Phase 5 Task 2 contract centralizes hashing in Rust).

UniVRM C# side lands in subsequent tasks.
EOF
)"
```

---

## Task 2: C# Manifest DTOs

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Manifest.cs`

- [ ] **Step 2.1: Add `RenderSequenceDto` + `RenderSequenceFrameDto`**

Add after the existing `AnimationDto` (or in a similar slot). JsonUtility constraints: every field public, no generics, no nullable value types, no IDictionary, **no fields named `internal`**.

```csharp
[Serializable]
public class RenderSequenceDto
{
    public int frame_count;
    public float frame_hz;
    public float physics_dt_seconds;
    public string output_format;             // "png_sequence"
    public RenderSequenceAnimateDto animate_root_transform;  // may be null
    public RenderSequenceVrmaDto apply_vrma;                 // may be null
    public float temporal_ssim_threshold;    // 0 ⇒ unset (use RFC default)
}

[Serializable]
public class RenderSequenceAnimateDto
{
    public float[] translation_start;
    public float[] translation_end;
}

[Serializable]
public class RenderSequenceVrmaDto
{
    public int vrma_handle;
    public float start_seconds;
}

[Serializable]
public class RenderSequenceFrameOutputDto
{
    public int index;
    public float timestamp_seconds;
    public string path;
    public string blake3;
}
```

- [ ] **Step 2.2: Add `render_sequence` field to `TestEntryDto`**

```csharp
public RenderSequenceDto render_sequence;  // null when plan is single-frame
```

- [ ] **Step 2.3: Add sequence fields to `EntryDto`**

The result-side `EntryDto` already has `output_path`, `blake3`, `actual_color_space`, `render_seconds`. Add:

```csharp
public RenderSequenceFrameOutputDto[] frames;  // null for single-frame entries
public float duration_seconds;                  // 0 for single-frame
public float frame_hz_achieved;                 // 0 for single-frame
```

JsonUtility will serialize zero/null defaults harmlessly; the Rust side's `#[serde(default)]` accepts them.

- [ ] **Step 2.4: Verify project compiles**

There's no good way to verify C# compilation outside Unity without running the launcher. The Unity build-validate CI step would catch syntax errors. Spot-check by reading the file end-to-end after the edit.

- [ ] **Step 2.5: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Manifest.cs
git commit -m "$(cat <<'EOF'
feat(adapters/univrm): Manifest DTOs for render_sequence

RenderSequenceDto / RenderSequenceAnimateDto / RenderSequenceVrmaDto /
RenderSequenceFrameOutputDto mirror the Rust-side schema. Added to
TestEntryDto (input) and EntryDto (output).

JsonUtility-compatible: all fields public, no generics, no nullable
value types. Zero-value sequence fields are harmless on single-frame
entries; Rust side accepts via #[serde(default)].
EOF
)"
```

---

## Task 3: C# `BatchRunner.RenderSequenceCo` helper

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/PlayMode/BatchRunner.cs`

- [ ] **Step 3.1: Add the sequence branch in `RenderOneCo`**

After the "VRMA — load + apply at time" block (around line 218, before the render phase), add:

```csharp
if (t.render_sequence != null)
{
    // Sequence-mode dispatch — replaces the single-frame render phase.
    yield return RenderSequenceCo(outputDir, rendererName, t, cam, e => result = e);

    // Cleanup is the same as the single-frame path's finally clause —
    // but since we yielded inside the coroutine, we can't use try/finally
    // across yields. Mirror the per-frame cleanup manually.
    if (cameraGo != null) UnityEngine.Object.DestroyImmediate(cameraGo);
    if (lightGo != null) UnityEngine.Object.DestroyImmediate(lightGo);
    if (vrmGo != null) UnityEngine.Object.DestroyImmediate(vrmGo);
    setEntry(result);
    yield break;
}
```

- [ ] **Step 3.2: Add `RenderSequenceCo` helper method**

```csharp
private IEnumerator RenderSequenceCo(
    string outputDir,
    string rendererName,
    Manifest.TestEntryDto t,
    Camera cam,
    Action<Manifest.EntryDto> setEntry)
{
    var rs = t.render_sequence;

    // RFC-0004 validation
    if (rs.physics_dt_seconds > 1.0f / 60.0f + 1e-6f)
    {
        setEntry(ErrorEntry(t.test_id, -32602, "InvalidParams", "L4-sequence",
            $"physics_dt_seconds {rs.physics_dt_seconds} exceeds 60 Hz floor"));
        yield break;
    }
    if (rs.animate_root_transform != null && rs.apply_vrma != null)
    {
        setEntry(ErrorEntry(t.test_id, -32602, "InvalidParams", "L4-sequence",
            "animate_root_transform and apply_vrma are mutually exclusive"));
        yield break;
    }
    if (rs.apply_vrma != null)
    {
        setEntry(ErrorEntry(t.test_id, -32602, "InvalidParams", "L4-sequence",
            "apply_vrma not yet implemented in UniVRM (Phase 7 deferral)"));
        yield break;
    }
    if (!string.IsNullOrEmpty(rs.output_format) && rs.output_format != "png_sequence")
    {
        setEntry(ErrorEntry(t.test_id, -32602, "InvalidParams", "L4-sequence",
            $"output_format \"{rs.output_format}\" is not yet supported by UniVRM; only png_sequence"));
        yield break;
    }
    if (rs.frame_count < 1)
    {
        setEntry(ErrorEntry(t.test_id, -32602, "InvalidParams", "L4-sequence",
            "frame_count must be >= 1"));
        yield break;
    }

    // Output dir + frames sub-dir per the runner's convention:
    // <output_dir>/<test_id>_<renderer>_frames/<NNNN>.png
    var framesDir = Path.Combine(outputDir, $"{t.test_id}_{rendererName}_frames");
    try { Directory.CreateDirectory(framesDir); }
    catch (Exception e)
    {
        setEntry(ErrorEntry(t.test_id, -32002, "RenderFailed", "L4-sequence",
            $"create frames dir: {e}"));
        yield break;
    }

    // Find the VRM instance — t already loaded it in RenderOneCo. We
    // need access to the Vrm10Instance for the root + spring-bone.
    // The simplest path: walk the scene root for Vrm10Instance.
    var vrm = UnityEngine.Object.FindFirstObjectByType<UniVRM10.Vrm10Instance>();
    if (vrm == null)
    {
        setEntry(ErrorEntry(t.test_id, -32002, "RenderFailed", "L4-sequence",
            "no Vrm10Instance found in scene"));
        yield break;
    }

    var origPosition = vrm.transform.position;

    Vector3 startV = Vector3.zero, endV = Vector3.zero;
    if (rs.animate_root_transform != null
        && rs.animate_root_transform.translation_start != null
        && rs.animate_root_transform.translation_end != null)
    {
        startV = SceneSetup.GltfToUnity(rs.animate_root_transform.translation_start);
        endV = SceneSetup.GltfToUnity(rs.animate_root_transform.translation_end);
    }

    var runtime = vrm.Runtime;
    var zeroHash = "blake3:" + new string('0', 64);
    var framesOut = new List<Manifest.RenderSequenceFrameOutputDto>();

    for (int i = 0; i < rs.frame_count; i++)
    {
        // Interpolate root translation. For frame_count==1, t=0.
        float ti = rs.frame_count > 1 ? (float)i / (rs.frame_count - 1) : 0f;
        vrm.transform.position = origPosition + Vector3.Lerp(startV, endV, ti);

        // Step spring-bone physics one tick.
        if (runtime != null && runtime.SpringBone != null)
        {
            runtime.SpringBone.Process(rs.physics_dt_seconds);
        }

        // Yield so Unity renders the scene this frame.
        yield return null;

        // Capture to <i:04>.png.
        var framePath = Path.Combine(framesDir, $"{i:D4}.png");
        Capture.Result captureResult;
        try
        {
            captureResult = Capture.Render(cam, t.output, framePath);
        }
        catch (Exception e)
        {
            vrm.transform.position = origPosition;
            setEntry(ErrorEntry(t.test_id, -32002, "RenderFailed", "L4-sequence",
                $"frame {i}: {e}"));
            yield break;
        }

        framesOut.Add(new Manifest.RenderSequenceFrameOutputDto
        {
            index = i,
            timestamp_seconds = (float)i / rs.frame_hz,
            path = captureResult.outputPath,
            blake3 = zeroHash,
        });
    }

    // Restore root.
    vrm.transform.position = origPosition;

    setEntry(new Manifest.EntryDto
    {
        test_id = t.test_id,
        status = "ok",
        frames = framesOut.ToArray(),
        duration_seconds = (float)rs.frame_count / rs.frame_hz,
        frame_hz_achieved = rs.frame_hz,
        actual_color_space = "Srgb",
    });
}
```

The `using System.Collections.Generic;` import may be needed at the top of the file — check.

`Vrm10Instance.Runtime.SpringBone.Process(dt)` matches PhysicsDriver's existing call (line 71/115 of PhysicsDriver.cs). If `runtime` is null (non-spring-bone model), the loop still runs — the avatar just doesn't have physics to step.

- [ ] **Step 3.3: Capture single-frame baseline behavior for empty-frame test plans**

If the test plan declares `render_sequence` with `frame_count: 0`, current code rejects. If `frame_count: 1`, the loop runs once with t=0 (no animation). Good.

- [ ] **Step 3.4: Update EditMode `Conformance.cs` to reject sequence plans loudly**

In `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs`, find the VRMA-rejection block (around line 128, "VRMA tests require PlayMode batch"). Add a sibling check:

```csharp
if (t.render_sequence != null)
{
    return new Manifest.EntryDto
    {
        test_id = t.test_id,
        status = "error",
        error = new Manifest.ErrorDto
        {
            code = -32000,
            message = "render_sequence tests require PlayMode batch",
            data = new Manifest.ErrorDataDto
            {
                feature = "render_sequence",
                value = $"frame_count={t.render_sequence.frame_count}",
                supported = new[] { "default PlayMode launcher (omit L3_EDITMODE)" },
            },
        },
    };
}
```

Place after the VRMA rejection so the EditMode path explicitly tells callers PlayMode is needed.

- [ ] **Step 3.5: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/
git commit -m "$(cat <<'EOF'
feat(adapters/univrm): RenderSequenceCo in PlayMode BatchRunner

PlayMode batch's RenderOneCo branches on t.render_sequence and dispatches
into a new RenderSequenceCo coroutine: per-frame root lerp +
runtime.SpringBone.Process(physics_dt) + yield return null + Capture.Render
to <output_dir>/<test_id>_<renderer>_frames/<NNNN>.png. Restores original
root translation after the loop.

Validation mirrors VMK/three-vrm/godot-vrm: mutual exclusion +
physics_dt > 1/60 (with 1e-6 f32 tolerance) + apply_vrma + non-PNG
output_format all return -32602.

BLAKE3 populated with 64-zero sentinel; Rust runner re-hashes from PNG
bytes on receipt (Phase 5 Task 2 contract).

Also extends the EditMode entry (Conformance.RunBatch) to reject sequence
plans with a clear "requires PlayMode batch" error, matching the VRMA
precedent in the same file.
EOF
)"
```

---

## Task 4: Runner E2E against UniVRM PlayMode

**Files:**
- Create: `crates/vrm-runner/tests/render_sequence_e2e_univrm.rs`

- [ ] **Step 4.1: Add the `#[ignore]`-gated integration test**

Mirror the VMK/three-vrm/godot-vrm E2E pattern. UniVRM uses `execute-test-batch` (not `execute-test-plan`) because of its filesystem-as-protocol contract — see `crates/vrm-runner/src/execute_batch.rs` for the existing API.

Skeleton:

```rust
//! End-to-end sequence dispatch test against the UniVRM PlayMode adapter.
//!
//! Ignored by default because it requires:
//!   - macOS (UniVRM adapter is macOS-only per the launcher)
//!   - Unity 6 (6000.4.6f1) installed via Unity Hub
//!   - PlayMode is the launcher default; L3_EDITMODE=1 forces EditMode
//!     (which rejects sequence plans loudly)
//!
//! Run locally with:
//!   cargo test -p vrm-runner --test render_sequence_e2e_univrm -- --ignored
//!
//! UniVRM is the consortium reference for VRM 1.0 rendering. This test
//! makes it the fourth real renderer that drives render_sequence through
//! the runner, completing the Phase 7 coverage.

use camino::Utf8PathBuf;
use vrm_asset_generator::emit::emit_vrm;
use vrm_asset_generator::params::MToonParams;
use vrm_asset_generator::sidecar::build_default_test_plan;
use vrm_runner::execute_batch::{build_manifest, run_batch, RunOptions};  // adapt to actual API
use vrm_test_plan::{RenderSequenceBlock, SequenceFormat, SequenceRootTransformAnimation};

fn univrm_launcher() -> Utf8PathBuf {
    let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap().parent().unwrap();
    let launcher = workspace_root.join("adapters/univrm/launcher.sh");
    assert!(launcher.exists(), "univrm launcher.sh missing");
    // Probe Unity availability — if absent, the test reports cleanly.
    let unity_bin = std::env::var("UNITY_BIN")
        .unwrap_or_else(|_| "/Applications/Unity/Hub/Editor/6000.4.6f1/Unity.app/Contents/MacOS/Unity".into());
    assert!(
        std::path::Path::new(&unity_bin).exists(),
        "Unity 6000.4.6f1 must be installed (set UNITY_BIN or install via Hub)"
    );
    launcher
}

#[test]
#[ignore = "requires Unity 6 + macOS + UniVRM PlayMode launcher"]
fn univrm_render_sequence_with_animate_root_transform_produces_frames() {
    // ... build plan + asset, run via execute_test_batch, assert frames on disk
    // with non-zero rehashed BLAKE3
}
```

The exact harness depends on the existing `execute-test-batch` Rust API surface. Read `crates/vrm-runner/src/execute_batch.rs` and `crates/vrm-runner/src/cli.rs` to find the right entry point — the test may need to invoke the CLI subcommand directly or use a lower-level `run_batch` helper.

- [ ] **Step 4.2: Run with `--ignored`** locally if Unity is installed.

- [ ] **Step 4.3: Commit**

```bash
git add crates/vrm-runner/tests/render_sequence_e2e_univrm.rs
git commit -m "$(cat <<'EOF'
test(vrm-runner): render_sequence end-to-end against UniVRM PlayMode

#[ignore]-gated integration test mirroring the VMK/three-vrm/godot-vrm
E2E pattern. Drives a 2-frame swing-seq plan through the UniVRM
PlayMode batch launcher, asserts real PNG frames land on disk with
runner-rehashed BLAKE3.

UniVRM is the consortium reference for VRM 1.0 rendering; this completes
the four-way real-renderer coverage of render_sequence (mock + VMK +
three-vrm + godot-vrm + UniVRM).
EOF
)"
```

---

## Task 5: Workspace cleanup

- [ ] **Step 5.1: fmt + clippy + workspace test + npm test + swift test**

Standard checks. Add a note in the cleanup commit if any fmt fixes were needed.

---

## Phase 7 completion checklist

- [ ] Rust `BatchTestEntry` carries `render_sequence` through the manifest
- [ ] Rust `ResultEntry` carries `frames` / `duration_seconds` / `frame_hz_achieved`
- [ ] Runner re-hashes per-frame BLAKE3 from PNG bytes
- [ ] C# `Manifest.cs` has RenderSequenceDto + frame-output DTOs + render_sequence on TestEntryDto + sequence fields on EntryDto
- [ ] C# `BatchRunner.RenderOneCo` branches on `t.render_sequence != null`
- [ ] C# `RenderSequenceCo` runs the frame loop with per-frame PNG capture
- [ ] C# `Conformance.RunBatch` (EditMode) rejects sequence plans with clear "requires PlayMode batch" error
- [ ] Runner E2E `#[ignore]`-gated test exists and passes locally
- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo test --workspace` green
- [ ] three-vrm npm test green
- [ ] vmk swift test green

After Phase 7, UniVRM joins the four-way real-renderer coverage. The swing-seq corpus from Phase 4 can drive a five-way consensus diff (mock + VMK + three-vrm + godot-vrm + UniVRM). `docs/findings.md` post-Phase-5/6/7 numbers entry is the natural next step — the headline conformance number will visibly change for swing-sweep tests as the consortium reference (UniVRM PlayMode) joins the comparison.
