# VRMA Phase 4 — UniVRM Adapter (Real)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make UniVRM a real (non-Unimplemented) VRMA adapter. The PlayMode batch path loads `.vrma` files via `VrmAnimationImporter`, applies them to the loaded `Vrm10Instance` at the requested time, and dumps the resulting humanoid pose + expression weights + lookAt state to the `<output_dir>/<id>_<renderer>.pose.json` shape the runner reads in `crates/vrm-runner/src/diff.rs::diff_pose_one`.

UniVRM is the consortium reference for VRMA — phase 4 is the first real implementation. Phases 5 (three-vrm) and 6 (manual humanoid plans + bootstrap + findings) build on the pose.json shape this phase locks in.

**Architecture:**
- **Manifest schema in lockstep (Rust + C#):** `vrm_test_plan::VrmaAnimation` already exists from phase 2; the runner already serializes it into the batch manifest JSON via `BatchTestEntry.animation`. The UniVRM-side `AnimationDto` doesn't have a `vrma` field yet — task 1 adds it.
- **VRMA application in PlayMode only:** `VrmAnimationImporter` returns a `Vrm10AnimationInstance` with humanoid + expression + lookAt curves. Applying the animation requires Unity playable-graph machinery (`AnimationClipPlayable`) which only works in PlayMode. The existing `Conformance.Tests.Play.BatchRunner.RunBatchInPlayMode` is where VRMA processing lands.
- **Pose dump after VRMA apply, before render:** matches the runner's plan_to_ops sequence. The dumps capture the VRMA-applied pose; physics + render run after.
- **pose.json shape:** `{ humanoid: DumpHumanoidPoseResult, expressions: DumpExpressionWeightsResult, look_at: DumpLookAtStateResult }` — same JSON the runner reads via `ReferencePoseFixture` in `crates/vrm-runner/src/diff.rs`.

**Tech Stack:** Rust (`vrm-runner/src/execute_batch.rs`), C# (UniVRM adapter), Unity 6 + UniVRM v0.131.0 PlayMode test framework.

**Spec:** [`docs/superpowers/specs/2026-05-17-vrma-conformance-design.md`](../specs/2026-05-17-vrma-conformance-design.md).

**Builds on:**
- Phase 1 op types (commits `36b663d..fab903c`)
- Phase 2 runner substrate (commits `1e73346..a344436`)
- Phase 3 asset generator (commits `131e877..83a95b5`)

**Verifiability:** Tasks 1-3 land Rust + C# code; the C# compiles via Unity batch (no asset rendering). Tasks 4-9 require a local Unity 6000.4.6f1 install with UniVRM `com.vrmc.vrm@4a17eb92884b` package cache — the implementer subagent CAN compile and run Unity locally on this M4 Max host (existing Conformance.Tests.PlayMode passes today). End-to-end smoke against a phase-3-emitted VRMA plan verifies the pose.json shape.

---

## File structure

**Modify (Rust):**
- `crates/vrm-runner/src/execute_batch.rs` — bump manifest_version (no schema change needed Rust-side since `BatchTestEntry.animation: Option<AnimationConfig>` already carries `vrma`)

**Modify (C#):**
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Manifest.cs` — add `VrmaDto` + `AnimationDto.vrma`; bump `manifest_version`
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/PlayMode/BatchRunner.cs` — add VRMA processing branch
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs` — EditMode path: gracefully skip VRMA-bearing tests with a clear error (VRMA needs PlayMode)
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/ManifestRoundtripTest.cs` — extend round-trip to cover the new VrmaDto field

**Create (C#):**
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/VrmaDriver.cs` — owns VrmAnimationImporter integration, time-sample logic, pose-dump emission

**Modify (scripts):**
- (none — bootstrap already wires UniVRM; the addition is contingent on RUN_UNIVRM=1)

---

## Task 1: Manifest schema — add VRMA fields to AnimationDto

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Manifest.cs`
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/ManifestRoundtripTest.cs`

- [ ] **Step 1.1: Extend Manifest.cs**

Open `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Manifest.cs`. Find the `AnimationDto` class (around line 100):

```csharp
[Serializable]
public class AnimationDto
{
    public RootTransformDto root_transform;
}
```

Replace with:

```csharp
[Serializable]
public class AnimationDto
{
    public RootTransformDto root_transform;
    public VrmaDto vrma;
}

[Serializable]
public class VrmaDto
{
    public string path;
    public float apply_at_time;
}
```

Bump `manifest_version` at the top of the file (search for the existing constant or default — it's likely `manifest_version = 1` in BatchManifest construction; ensure both sides bump in lockstep).

- [ ] **Step 1.2: Extend ManifestRoundtripTest**

Open `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/ManifestRoundtripTest.cs`. Find the existing test that round-trips an `AnimationDto` and extend it to construct one with a `vrma` field, then verify JsonUtility serializes/deserializes it:

```csharp
[Test]
public void AnimationDtoRoundtripsVrma()
{
    var input = new Manifest.AnimationDto
    {
        root_transform = null,
        vrma = new Manifest.VrmaDto { path = "/tmp/x.vrma", apply_at_time = 0.5f },
    };
    var json = JsonUtility.ToJson(input);
    var back = JsonUtility.FromJson<Manifest.AnimationDto>(json);
    Assert.AreEqual("/tmp/x.vrma", back.vrma.path);
    Assert.AreEqual(0.5f, back.vrma.apply_at_time);
    Assert.IsNull(back.root_transform);
    StringAssert.Contains("\"vrma\":", json);
}
```

- [ ] **Step 1.3: Verify EditMode tests pass**

Run from repo root:

```bash
cd adapters/univrm
./launcher.sh --edit-mode-test-only
```

(Or invoke Unity directly with EditMode test args — check `launcher.sh` for the exact invocation. The EditMode test suite includes ManifestRoundtripTest.)

Expected: all EditMode tests pass including the new `AnimationDtoRoundtripsVrma`.

- [ ] **Step 1.4: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Manifest.cs \
        adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/ManifestRoundtripTest.cs
git commit -m "$(cat <<'EOF'
feat(adapters/univrm): add VrmaDto to AnimationDto for batch manifest

Mirrors the Rust-side vrm_test_plan::VrmaAnimation already shipped by
phase 2 (commit 6a45368). AnimationDto now carries an optional vrma
block with path + apply_at_time, JsonUtility-friendly. Roundtrip test
verifies serialize/deserialize symmetry.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Runner-side manifest_version bump (if needed)

**Files:**
- Modify: `crates/vrm-runner/src/execute_batch.rs`

Phase 2 already extended `vrm_test_plan::AnimationConfig` with `vrma`, and `BatchTestEntry` carries `animation: Option<AnimationConfig>` directly via `plan.animation.clone()`. So the JSON serialization already includes `vrma` when the plan has it. The C# side just needs to know which manifest_version it's looking at.

- [ ] **Step 2.1: Bump manifest_version constant**

Open `crates/vrm-runner/src/execute_batch.rs`. Find where `manifest_version` is set (likely a literal `1` or constant). If the previous value was `1`, change to `2`. Update any test fixtures that hard-code the version.

- [ ] **Step 2.2: Verify**

Run: `cargo test -p vrm-runner execute_batch` — all tests pass.

Run: `cargo clippy -p vrm-runner --all-targets -- -D warnings` — clean.

- [ ] **Step 2.3: Commit**

```bash
git add crates/vrm-runner/src/execute_batch.rs
git commit -m "$(cat <<'EOF'
feat(vrm-runner): bump batch manifest_version to 2 for VRMA fields

The wire JSON gains animation.vrma per phase 2; this version bump
signals that to the UniVRM C# side (which now reads VrmaDto from
AnimationDto).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Conformance.cs EditMode path — graceful skip for VRMA tests

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs`

EditMode can't drive AnimationClipPlayable graphs, so VRMA test plans must be routed through PlayMode. If a VRMA plan reaches the EditMode `RunBatch`, produce a clear error rather than silently skipping or rendering rest-pose.

- [ ] **Step 3.1: Branch on `t.animation?.vrma != null`**

In `Conformance.cs::RenderOne`, near the existing pre-flight rejection for unsupported post-processing (around line 86), add:

```csharp
// VRMA tests require PlayMode (AnimationClipPlayable graphs).
// EditMode batch path rejects them explicitly so failures surface as
// "feature: needs_playmode" rather than silent rest-pose renders.
if (t.animation != null && t.animation.vrma != null)
{
    return new Manifest.EntryDto
    {
        test_id = t.test_id,
        status = "error",
        error = new Manifest.ErrorDto
        {
            code = -32000,
            message = "VRMA tests require PlayMode batch (animation.vrma present)",
            data = new Manifest.ErrorDataDto
            {
                feature = "vrma",
                value = t.animation.vrma.path,
                supported = "RUN_UNIVRM_PLAYMODE=1 with Conformance.Tests.Play.BatchRunner",
            },
        },
    };
}
```

- [ ] **Step 3.2: Compile + commit**

Run a Unity batch compile pass (`launcher.sh --validate-only` or equivalent). Expected: clean.

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs
git commit -m "$(cat <<'EOF'
feat(adapters/univrm): EditMode RunBatch rejects VRMA tests cleanly

VRMA application requires AnimationClipPlayable graphs which only work
in PlayMode. The EditMode path now returns code=-32000 with
feature="vrma" instead of silently rendering rest-pose. PlayMode
BatchRunner (next task) implements the real VRMA path.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: VrmaDriver — load + apply VRMA at time t

**Files:**
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/VrmaDriver.cs`

The VrmaDriver wraps UniVRM's `VrmAnimationImporter` and the runtime application machinery. Its public surface:

- `static IEnumerator LoadAndApply(string vrmaPath, Vrm10Instance target, float time, Action<bool, string> onComplete)` — loads the .vrma, retargets to the target Vrm10Instance, samples at `time`, and writes the result onto the avatar.

The UniVRM canonical sample for this is `Assets/VRM10_Samples/RuntimeLoaderSample/VrmAnimation*` (or similar). Inspect those for the official API path — likely:

1. `VrmAnimation.LoadAsync(path, awaitCaller)` → `Vrm10AnimationInstance`
2. Build retarget configuration between source AnimationInstance and target Vrm10Instance using `(INormalizedPoseProvider, ITPoseProvider) ControlRig`
3. Sample animation at time t via `AnimationClip.SampleAnimation(animationGo, time)` or playable-graph evaluate

- [ ] **Step 4.1: Locate UniVRM's canonical VRMA application sample**

```bash
find adapters/univrm/UniVRMConformance/Library/PackageCache -name "*VrmAnimation*Sample*" -o -name "*Retarget*Sample*"
grep -rn "VrmAnimation.LoadAsync\|VrmAnimationImporter\|Vrm10AnimationInstance" \
    adapters/univrm/UniVRMConformance/Library/PackageCache/com.vrmc.vrm@4a17eb92884b/Samples~/ 2>/dev/null | head
```

If samples reference a "RuntimeAnimation" or "CopyPose" sample, READ the relevant `.cs` file to lock in the canonical retarget shape. Adapt our implementation from there.

- [ ] **Step 4.2: Create VrmaDriver.cs**

```csharp
// VRMA application driver. PlayMode-only — uses Unity AnimationClip
// sampling which requires Application.isPlaying. Wraps UniVRM's
// VrmAnimationImporter + control-rig retargeting.
//
// Caller responsibility:
//   1. Load the .vrm via Vrm10.LoadPathAsync; pass the resulting
//      Vrm10Instance as `target`.
//   2. After LoadAndApply completes successfully, the target's
//      transforms / expression weights / lookAt state reflect the
//      .vrma sampled at `time`.
//   3. Read the pose-dump fields (humanoid bones from Animator
//      transforms; expressions from target.Runtime.Expression;
//      lookAt from target.Runtime.LookAt) and serialize via
//      VrmaDriver.WritePoseJson.

using System;
using System.Collections;
using System.IO;
using System.Text;
using UniGLTF;
using UnityEngine;
using UniVRM10;

namespace Conformance
{
    public static class VrmaDriver
    {
        public class ApplyResult
        {
            public bool ok;
            public string error;
        }

        // Loads vrmaPath as a Vrm10AnimationInstance, retargets onto
        // `target`'s control rig, samples at `time`. The caller's
        // coroutine must yield to this method.
        public static IEnumerator LoadAndApply(
            string vrmaPath,
            Vrm10Instance target,
            float time,
            Action<ApplyResult> onComplete)
        {
            ApplyResult result = new ApplyResult { ok = false, error = null };

            Vrm10AnimationInstance animation = null;
            Exception loadException = null;

            // 1. Load .vrma via UniVRM's importer.
            System.Threading.Tasks.Task<Vrm10AnimationInstance> loadTask = null;
            try
            {
                loadTask = VrmAnimation.LoadAsync(
                    vrmaPath,
                    awaitCaller: new ImmediateCaller(),
                    ct: System.Threading.CancellationToken.None);
            }
            catch (Exception e)
            {
                loadException = e;
            }

            if (loadException != null)
            {
                result.error = $"VrmAnimation.LoadAsync threw: {loadException}";
                onComplete?.Invoke(result);
                yield break;
            }

            // Wait for the load to complete (ImmediateCaller resolves synchronously).
            while (!loadTask.IsCompleted) yield return null;
            if (loadTask.IsFaulted)
            {
                result.error = $"VrmAnimation load faulted: {loadTask.Exception}";
                onComplete?.Invoke(result);
                yield break;
            }

            animation = loadTask.Result;
            if (animation == null)
            {
                result.error = "VrmAnimation.LoadAsync returned null Vrm10AnimationInstance";
                onComplete?.Invoke(result);
                yield break;
            }

            try
            {
                // 2. Retarget control rigs.
                //    The source's ControlRig provides INormalizedPoseProvider;
                //    target.Runtime accepts that to drive humanoid bones.
                //    The canonical UniVRM call is something like:
                //      target.Runtime.Process(source: animation, time: time)
                //    BUT the actual API may differ — verify in the spike (step 4.1).
                //    If the API expects a Playable graph evaluation, build one
                //    via animation.gameObject.GetComponent<Animator>() +
                //    AnimationClipPlayable.Create(graph, clip).
                //    Document the exact API used in a comment here.

                // 3. Sample at `time`. For AnimationClip-on-Animator: simulate
                //    by setting the animator's playable time:
                var clip = animation.GetComponent<UnityEngine.Animation>()?.clip
                    ?? animation.GetComponentInChildren<Animator>()?.runtimeAnimatorController as AnimationClip;
                if (clip != null)
                {
                    clip.SampleAnimation(animation.gameObject, time);
                }

                // 4. Push the source's sampled state onto the target via
                //    retargeting. The IVrm10Animation interface exposes
                //    (INormalizedPoseProvider, ITPoseProvider) ControlRig.
                //    target.Runtime.Process(...) is the application entry.
                var (sourcePose, sourceTPose) = animation.ControlRig;
                target.Runtime.Process(sourcePose, sourceTPose);

                result.ok = true;
            }
            catch (Exception e)
            {
                result.error = $"Apply threw: {e}";
            }
            finally
            {
                // Clean up the loaded animation GameObject — its only purpose
                // was to be a sample source.
                if (animation != null && animation.gameObject != null)
                {
                    UnityEngine.Object.DestroyImmediate(animation.gameObject);
                }
            }

            onComplete?.Invoke(result);
        }

        // Read the avatar's current state into the runner's pose.json shape.
        public static string BuildPoseJson(Vrm10Instance target)
        {
            // ... (filled in by tasks 5-7)
            return "{\"humanoid\":null,\"expressions\":null,\"look_at\":null}";
        }

        public static void WritePoseJson(string path, string json)
        {
            Directory.CreateDirectory(Path.GetDirectoryName(path));
            File.WriteAllText(path, json, Encoding.UTF8);
        }
    }
}
```

**Important — verify the API in step 4.1 first.** The `VrmAnimation.LoadAsync`, `target.Runtime.Process`, and `animation.ControlRig` calls above are sketched from the IVrm10Animation interface but the actual canonical caller may use a different shape (e.g. `Vrm10Runtime.Process(...)` requires specific arg shapes). The implementer MUST verify against UniVRM's runtime sample before committing.

- [ ] **Step 4.3: Compile**

Unity batch compile pass. Expected: clean.

- [ ] **Step 4.4: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/VrmaDriver.cs
git commit -m "$(cat <<'EOF'
feat(adapters/univrm): VrmaDriver — load + apply VRMA at time t

PlayMode-only. Wraps VrmAnimation.LoadAsync + control-rig retargeting
onto a target Vrm10Instance, samples at the requested time. Pose-dump
methods stubbed; filled in by tasks 5-7.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: dump_humanoid_pose — read humanoid bone rotations from Vrm10Instance

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/VrmaDriver.cs`

After `Runtime.Process(sourcePose, sourceTPose)` lands, the target Vrm10Instance's humanoid bones reflect the VRMA pose. Read them via:

- `target.Humanoid.BoneMap` returns `Dictionary<HumanBodyBones, Transform>` (or similar — check `Vrm10Instance` API)
- For each humanoid bone: `transform.localRotation` is the spec-relevant `local_rotation_quat`
- For `hips`: also read `transform.localPosition` (the only bone with translation per spec)

VRM humanoid bone names map to the 15 required + 4 optional from `humanoid::Skeleton`. The VRM10 enum or `Vrm10HumanoidBones` enum provides the spec-conformant names.

- [ ] **Step 5.1: Inspect Vrm10Instance humanoid API**

```bash
grep -rn "HumanBodyBones\|HumanoidBoneType\|VrmExtensionAccessor" \
    adapters/univrm/UniVRMConformance/Library/PackageCache/com.vrmc.vrm@4a17eb92884b/Runtime/Components/Vrm10Instance.cs \
    adapters/univrm/UniVRMConformance/Library/PackageCache/com.vrmc.vrm@4a17eb92884b/Runtime/Humanoid/*.cs 2>/dev/null | head -20
```

Identify the exact accessor: likely `target.GetBoneTransform(HumanBodyBones)` or `target.Humanoid.Bones[HumanBodyBones.Head]`.

- [ ] **Step 5.2: Implement `BuildHumanoidPoseSection(Vrm10Instance)` helper**

In `VrmaDriver.cs`, add a private helper that walks the 19 humanoid bones, reads each Transform, and emits a JSON fragment. Replace the stub return in `BuildPoseJson` accordingly.

```csharp
// VRMA spec humanoid bones — 15 required + 4 commonly-supported.
private static readonly HumanBodyBones[] HumanoidBoneEnum = new[]
{
    HumanBodyBones.Hips,
    HumanBodyBones.Spine,
    HumanBodyBones.Chest,
    HumanBodyBones.Neck,
    HumanBodyBones.Head,
    HumanBodyBones.LeftShoulder,
    HumanBodyBones.LeftUpperArm,
    HumanBodyBones.LeftLowerArm,
    HumanBodyBones.LeftHand,
    HumanBodyBones.RightShoulder,
    HumanBodyBones.RightUpperArm,
    HumanBodyBones.RightLowerArm,
    HumanBodyBones.RightHand,
    HumanBodyBones.LeftUpperLeg,
    HumanBodyBones.LeftLowerLeg,
    HumanBodyBones.LeftFoot,
    HumanBodyBones.RightUpperLeg,
    HumanBodyBones.RightLowerLeg,
    HumanBodyBones.RightFoot,
};

private static string ToVrmaBoneName(HumanBodyBones b) => b switch
{
    HumanBodyBones.Hips => "hips",
    HumanBodyBones.Spine => "spine",
    HumanBodyBones.Chest => "chest",
    HumanBodyBones.Neck => "neck",
    HumanBodyBones.Head => "head",
    HumanBodyBones.LeftShoulder => "leftShoulder",
    HumanBodyBones.LeftUpperArm => "leftUpperArm",
    HumanBodyBones.LeftLowerArm => "leftLowerArm",
    HumanBodyBones.LeftHand => "leftHand",
    HumanBodyBones.RightShoulder => "rightShoulder",
    HumanBodyBones.RightUpperArm => "rightUpperArm",
    HumanBodyBones.RightLowerArm => "rightLowerArm",
    HumanBodyBones.RightHand => "rightHand",
    HumanBodyBones.LeftUpperLeg => "leftUpperLeg",
    HumanBodyBones.LeftLowerLeg => "leftLowerLeg",
    HumanBodyBones.LeftFoot => "leftFoot",
    HumanBodyBones.RightUpperLeg => "rightUpperLeg",
    HumanBodyBones.RightLowerLeg => "rightLowerLeg",
    HumanBodyBones.RightFoot => "rightFoot",
    _ => throw new ArgumentOutOfRangeException(nameof(b)),
};

private static StringBuilder AppendHumanoidPose(StringBuilder sb, Vrm10Instance target)
{
    sb.Append("\"humanoid\":{");
    sb.Append("\"bones\":[");

    bool first = true;
    var missing = new System.Collections.Generic.List<string>();

    foreach (var bone in HumanoidBoneEnum)
    {
        var t = target.GetBoneTransform(bone);  // verify exact accessor
        if (t == null)
        {
            missing.Add(ToVrmaBoneName(bone));
            continue;
        }
        if (!first) sb.Append(',');
        first = false;
        var q = t.localRotation;
        sb.AppendFormat(System.Globalization.CultureInfo.InvariantCulture,
            "{{\"name\":\"{0}\",\"local_rotation_quat\":[{1},{2},{3},{4}]}}",
            ToVrmaBoneName(bone), q.x, q.y, q.z, q.w);
    }
    sb.Append(']');

    // Hips translation
    var hips = target.GetBoneTransform(HumanBodyBones.Hips);
    if (hips != null)
    {
        var p = hips.localPosition;
        sb.AppendFormat(System.Globalization.CultureInfo.InvariantCulture,
            ",\"hips_translation\":[{0},{1},{2}]", p.x, p.y, p.z);
    }

    if (missing.Count > 0)
    {
        sb.Append(",\"bones_missing\":[");
        for (int i = 0; i < missing.Count; i++)
        {
            if (i > 0) sb.Append(',');
            sb.Append('"').Append(missing[i]).Append('"');
        }
        sb.Append(']');
    }
    sb.Append('}');
    return sb;
}
```

If `target.GetBoneTransform(HumanBodyBones)` doesn't exist, the spike from step 5.1 will have revealed the right accessor — adapt accordingly.

- [ ] **Step 5.3: Compile + commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/VrmaDriver.cs
git commit -m "$(cat <<'EOF'
feat(adapters/univrm): VrmaDriver dumps humanoid pose section

Reads localRotation for each of the 19 VRM humanoid bones (15 required
+ 4 optional) via target.GetBoneTransform. Hips localPosition becomes
hips_translation per VRMA spec. Bones absent from the loaded avatar
end up in bones_missing for the runner's diff_pose path.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: dump_expression_weights — read from Vrm10ExpressionRuntime

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/VrmaDriver.cs`

The Vrm10Instance exposes `Runtime.Expression.Process` and accessors for current expression weights. The 14 preset names map directly to VRM10's `ExpressionPreset` enum.

- [ ] **Step 6.1: Inspect Expression runtime API**

```bash
grep -rn "ExpressionRuntime\|GetWeight\|expression.*weight" \
    adapters/univrm/UniVRMConformance/Library/PackageCache/com.vrmc.vrm@4a17eb92884b/Runtime/Vrm10Runtime/*.cs 2>/dev/null | head
```

Identify the public read accessor (likely `target.Runtime.Expression.GetWeight(ExpressionKey)` or `target.Vrm.Expression.MergedRuntime.GetWeights()`).

- [ ] **Step 6.2: Implement `AppendExpressions(...)` helper**

Add to `VrmaDriver.cs`:

```csharp
private static readonly ExpressionPreset[] PresetEnum = new[]
{
    ExpressionPreset.happy, ExpressionPreset.angry, ExpressionPreset.sad,
    ExpressionPreset.relaxed, ExpressionPreset.surprised,
    ExpressionPreset.aa, ExpressionPreset.ih, ExpressionPreset.ou,
    ExpressionPreset.ee, ExpressionPreset.oh,
    ExpressionPreset.blink, ExpressionPreset.blinkLeft, ExpressionPreset.blinkRight,
    ExpressionPreset.neutral,
};

private static StringBuilder AppendExpressions(StringBuilder sb, Vrm10Instance target)
{
    sb.Append("\"expressions\":{");
    sb.Append("\"presets\":{");

    bool first = true;
    foreach (var preset in PresetEnum)
    {
        var key = ExpressionKey.CreateFromPreset(preset);
        var weight = target.Runtime.Expression.GetWeight(key); // verify API
        if (!first) sb.Append(',');
        first = false;
        sb.AppendFormat(System.Globalization.CultureInfo.InvariantCulture,
            "\"{0}\":{1}", preset.ToString(), weight);
    }
    sb.Append('}');

    // Custom expressions: iterate target's Vrm.Expression.Custom collection.
    sb.Append(",\"custom\":{");
    // ... iterate custom expressions; map name → weight
    sb.Append('}');

    sb.Append('}');
    return sb;
}
```

- [ ] **Step 6.3: Compile + commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/VrmaDriver.cs
git commit -m "$(cat <<'EOF'
feat(adapters/univrm): VrmaDriver dumps expression weights section

Reads weights via target.Runtime.Expression.GetWeight for each of the
14 VRMA spec presets + iterates custom expressions for the custom
section. Output format matches the runner's DumpExpressionWeightsResult
shape (presets + custom kept structurally separate per spec).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: dump_look_at_state — read from Vrm10LookAt runtime

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/VrmaDriver.cs`

- [ ] **Step 7.1: Inspect LookAt API**

```bash
grep -rn "Vrm10LookAt\|GetYaw\|GetPitch\|lookAt" \
    adapters/univrm/UniVRMConformance/Library/PackageCache/com.vrmc.vrm@4a17eb92884b/Runtime/Vrm10Runtime/*.cs 2>/dev/null | head
```

Find the accessors for current yaw/pitch, applied mode (bone vs expression), and `offsetFromHeadBone`.

- [ ] **Step 7.2: Implement `AppendLookAt(...)`**

```csharp
private static StringBuilder AppendLookAt(StringBuilder sb, Vrm10Instance target)
{
    sb.Append("\"look_at\":{");

    // Raw quaternion from the LookAt component's current rotation.
    // The VRMA spec encodes gaze as a quat on a node; UniVRM applies it
    // either to head/eye bones or expressions. We report the spec-correct
    // gaze quat (NOT the head bone's rotation).
    Quaternion gaze;
    // Method varies — likely target.Runtime.LookAt.GetGazeQuat() or
    // similar. If not directly exposed, derive from yaw/pitch:
    // gaze = Quaternion.Euler(pitch, yaw, 0).
    float yaw = target.Runtime.LookAt.Yaw;     // verify API
    float pitch = target.Runtime.LookAt.Pitch; // verify API
    // Extrinsic ZXY per spec: rotation around Y is yaw, around X is pitch.
    gaze = Quaternion.Euler(pitch, yaw, 0f);

    sb.AppendFormat(System.Globalization.CultureInfo.InvariantCulture,
        "\"gaze_direction_quat\":[{0},{1},{2},{3}]",
        gaze.x, gaze.y, gaze.z, gaze.w);
    sb.AppendFormat(System.Globalization.CultureInfo.InvariantCulture,
        ",\"yaw_deg\":{0},\"pitch_deg\":{1}",
        yaw, pitch);

    // applied_via — read from the loaded avatar's VRMC_vrm.lookAt.type.
    string appliedVia = target.Vrm.LookAt.LookAtType switch
    {
        LookAtType.bone => "bone",
        LookAtType.expression => "expression",
        _ => "off",
    };
    sb.AppendFormat(",\"applied_via\":\"{0}\"", appliedVia);

    // offsetFromHeadBone — read from the VRM extension.
    var offset = target.Vrm.LookAt.OffsetFromHead;
    sb.AppendFormat(System.Globalization.CultureInfo.InvariantCulture,
        ",\"offset_from_head_bone\":[{0},{1},{2}]",
        offset.x, offset.y, offset.z);

    sb.Append('}');
    return sb;
}
```

- [ ] **Step 7.3: Update `BuildPoseJson` to call all three**

```csharp
public static string BuildPoseJson(Vrm10Instance target)
{
    var sb = new StringBuilder(2048);
    sb.Append('{');
    AppendHumanoidPose(sb, target);
    sb.Append(',');
    AppendExpressions(sb, target);
    sb.Append(',');
    AppendLookAt(sb, target);
    sb.Append('}');
    return sb.ToString();
}
```

- [ ] **Step 7.4: Compile + commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/VrmaDriver.cs
git commit -m "$(cat <<'EOF'
feat(adapters/univrm): VrmaDriver dumps lookAt state section

Reads yaw + pitch from target.Runtime.LookAt; derives the spec-defined
gaze quaternion via Extrinsic ZXY (Quaternion.Euler(pitch, yaw, 0)).
applied_via from target.Vrm.LookAt.LookAtType (bone | expression | off).
offsetFromHeadBone from the avatar's VRM extension.

BuildPoseJson now produces the full {humanoid, expressions, look_at}
shape the runner reads via ReferencePoseFixture.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: PlayMode BatchRunner — wire VRMA into the render pipeline

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/PlayMode/BatchRunner.cs`

Insert VRMA processing AFTER physics settle but BEFORE render. Output pose.json alongside the PNG.

- [ ] **Step 8.1: Add VRMA branch in RunBatchInPlayMode**

Find the existing `PhysicsDriver.Settle(vrm, t.physics)` call. Insert immediately after it (still inside the per-test `try`):

```csharp
// VRMA — load + apply at time t, then dump pose state.
if (t.animation != null && t.animation.vrma != null)
{
    bool vrmaCompleted = false;
    VrmaDriver.ApplyResult vrmaResult = null;
    yield return VrmaDriver.LoadAndApply(
        t.animation.vrma.path,
        vrm,
        t.animation.vrma.apply_at_time,
        r => { vrmaResult = r; vrmaCompleted = true; });
    if (!vrmaCompleted || !vrmaResult.ok)
    {
        result = ErrorEntry(t.test_id, -32000, "VrmaApplyFailed", "L4",
            vrmaResult?.error ?? "VrmaDriver did not invoke onComplete");
        setEntry(result);
        // Continue to next test (don't break the batch).
        if (cameraGo != null) UnityEngine.Object.DestroyImmediate(cameraGo);
        if (lightGo != null) UnityEngine.Object.DestroyImmediate(lightGo);
        if (vrmGo != null) UnityEngine.Object.DestroyImmediate(vrmGo);
        continue;
    }

    // Write pose.json alongside the PNG output path.
    var poseJson = VrmaDriver.BuildPoseJson(vrm);
    var posePath = Path.Combine(outputDir, $"{t.test_id}_{manifest.renderer_name}.pose.json");
    VrmaDriver.WritePoseJson(posePath, poseJson);
}
```

The `continue` is critical — VRMA-error tests still need cleanup of camera/light/vrm GameObjects, otherwise the next iteration leaks.

- [ ] **Step 8.2: Compile + commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/PlayMode/BatchRunner.cs
git commit -m "$(cat <<'EOF'
feat(adapters/univrm): BatchRunner applies VRMA + writes pose.json

After PhysicsDriver.Settle and before render, if t.animation.vrma is
set, VrmaDriver.LoadAndApply samples the .vrma at apply_at_time and
writes the result onto the loaded Vrm10Instance. The pose.json sidecar
goes to <output_dir>/<test_id>_<renderer>.pose.json so the Rust runner
picks it up via diff_pose_one.

VRMA load/apply failures produce -32000 entries and don't break the
batch.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: Smoke test against a phase-3 VRMA plan

**Files:** none directly.

- [ ] **Step 9.1: Generate one VRMA test plan**

```bash
cargo run -p vrm-asset-generator -- emit-vrma-humanoid-sweep --output-dir /tmp/vrma-univrm-smoke
ls /tmp/vrma-univrm-smoke/vrma_humanoid_head_yaw_45.*
```

Expected: 3 files (.vrm + .vrma + .test.yaml).

- [ ] **Step 9.2: Build the UniVRM batch manifest pointing at this one plan**

The simplest path: invoke vrm-runner's `execute-test-batch` subcommand with a one-file batch directory.

```bash
mkdir -p /tmp/vrma-univrm-smoke-output
RUN_UNIVRM=1 cargo run -p vrm-runner -- execute-test-batch \
    --plans /tmp/vrma-univrm-smoke \
    --adapter-bin adapters/univrm/launcher.sh \
    --asset-dir /tmp/vrma-univrm-smoke \
    --output-dir /tmp/vrma-univrm-smoke-output \
    --renderer-name univrm \
    --json | tee /tmp/vrma-univrm-smoke.log
```

- [ ] **Step 9.3: Verify pose.json appears with the expected shape**

```bash
ls -la /tmp/vrma-univrm-smoke-output/
cat /tmp/vrma-univrm-smoke-output/vrma_humanoid_head_yaw_45_univrm.pose.json | head -30
```

Expected: file exists, contains `humanoid`, `expressions`, `look_at` top-level keys; `humanoid.bones` includes the 19 humanoid bones; `humanoid.bones[where name=="head"].local_rotation_quat` shows a ~45° Y rotation (`[0, ~0.38, 0, ~0.92]`).

Run the `cargo test --workspace` to ensure nothing regressed:

```bash
cargo test --workspace 2>&1 | tail -10
```

Expected: all tests pass.

- [ ] **Step 9.4: Commit a tracking note (if smoke produced new findings)**

If the smoke surfaced an issue worth recording (e.g. UniVRM applies VRMA differently than the canonical reference), append to `docs/findings.md`. Otherwise no commit needed for this step.

---

## Task 10: Workspace fmt + clippy + tests

**Files:** none directly.

- [ ] **Step 10.1: Run cleanup**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All three clean.

- [ ] **Step 10.2: Commit any fmt/clippy fixes**

```bash
git status -s
```

If modifications:

```bash
git add -u
git commit -m "$(cat <<'EOF'
chore: cargo fmt + clippy clean-up after VRMA phase 4

Final workspace pass after VRMA phase 4 (UniVRM adapter real). Zero
clippy warnings, zero fmt diffs.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Otherwise skip.

---

## Phase 4 completion checklist

- [ ] `Manifest.cs` `AnimationDto` carries `VrmaDto vrma`; `manifest_version` bumped Rust+C# in lockstep
- [ ] `ManifestRoundtripTest.AnimationDtoRoundtripsVrma` passes
- [ ] `Conformance.cs` EditMode path returns `-32000 vrma needs_playmode` for VRMA-bearing tests
- [ ] `VrmaDriver.cs` exposes `LoadAndApply` (load .vrma → retarget → sample at time t) + `BuildPoseJson` + `WritePoseJson`
- [ ] `BuildPoseJson` produces the runner's `ReferencePoseFixture` shape: `{humanoid: {bones, hips_translation, bones_missing}, expressions: {presets, custom}, look_at: {gaze_direction_quat, yaw_deg, pitch_deg, applied_via, offset_from_head_bone}}`
- [ ] `BatchRunner.cs` PlayMode path invokes VrmaDriver when `t.animation.vrma != null`; writes `<output_dir>/<id>_<renderer>.pose.json`
- [ ] One phase-3-emitted VRMA plan (`vrma_humanoid_head_yaw_45`) renders end-to-end through UniVRM batch with pose.json output
- [ ] pose.json includes a `head` bone with localRotation reflecting the +45° Y arc
- [ ] `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace` all clean

After this phase, UniVRM is the first real VRMA adapter. Phase 5 wires three-vrm (also real). Phase 6 lands manual humanoid clips + cross-renderer bootstrap + findings.

## Important caveats for the implementer

1. **UniVRM API names are sketched, not verified.** `target.GetBoneTransform`, `target.Runtime.Expression.GetWeight`, `target.Runtime.LookAt.Yaw`, `target.Vrm.LookAt.LookAtType`, `VrmAnimation.LoadAsync` — these names follow the IVrm10Animation interface but the actual API of `com.vrmc.vrm@4a17eb92884b` may differ slightly. **Spike step 5.1, 6.1, 7.1 each direct you to verify the exact API surface from the package cache before committing.** Adapt the code to what's actually there.

2. **PlayMode test invocation requires Unity 6000.4.6f1 locally.** The implementer needs to verify Unity is on `PATH` (or set `UNITY_BIN`) and that `Conformance.Tests.PlayMode` is configured to discover `Conformance.Tests.Play.BatchRunner.RunBatchInPlayMode` as a UnityTest. The existing PlayMode tests already work end-to-end so the framework is wired — VrmaDriver lands in that framework.

3. **The retarget call is the riskiest piece.** Vrm10AnimationInstance vs Vrm10Instance — UniVRM's runtime exposes the `(INormalizedPoseProvider, ITPoseProvider) ControlRig` tuple but the actual application call (`target.Runtime.Process(...)`) may need to be in a specific Update phase of the playable graph. If the simple `clip.SampleAnimation + Process` approach in task 4 doesn't produce a visible pose change, look for a CopyPose sample under `Library/PackageCache/com.vrmc.vrm@4a17eb92884b/Samples~/` and follow its pattern verbatim.
