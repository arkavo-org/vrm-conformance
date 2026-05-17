// VRMA application driver — PlayMode only.
//
// Canonical UniVRM v0.131.0 API verified against
// Library/PackageCache/com.vrmc.vrm@4a17eb92884b/Samples~/SimpleVrma/SimpleVrma.cs
// and Runtime/Components/VrmAnimationInstance/Vrm10AnimationInstance.cs:
//
//   Loader  : new AutoGltfFileParser(path).Parse()
//             → new VrmAnimationData(data)
//             → new VrmAnimationImporter(vrmaData)
//             → await loader.LoadAsync(new ImmediateCaller())
//             → gltfInstance.GetComponent<Vrm10AnimationInstance>()
//
//   Apply   : target.Runtime.VrmAnimation = vrmaInstance;
//             then target.Runtime.Process() to retarget pose + expressions.
//             Vrm10Runtime.Process() calls Vrm10Retarget.Retarget(ControlRig)
//             + expression weight setters + LookAt.
//
//   Sample  : Vrm10AnimationInstance implements ITimeControl.SetTime(t)
//             which sets AnimationState.time + calls animation.Sample().
//             We replicate that pattern directly: grab the Animation
//             component, disable playback speed, play, seek, sample —
//             then call Runtime.Process() once.
//
// Caller responsibility: load the .vrm via Vrm10.LoadPathAsync first,
// then call VrmaDriver.LoadAndApply with the resulting Vrm10Instance.
// After the coroutine completes with ok==true, the target's transforms,
// expression weights, and lookAt state reflect the .vrma sampled at time.
// Tasks 5-7 of the VRMA phase 4 plan fill in BuildPoseJson.

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

        /// <summary>
        /// Load a .vrma from disk, retarget it onto <paramref name="target"/>,
        /// and sample the animation at <paramref name="time"/> seconds.
        /// Must be called from a PlayMode coroutine — Vrm10Runtime gating on
        /// Application.isPlaying and FastSpringBone require it.
        /// </summary>
        public static IEnumerator LoadAndApply(
            string vrmaPath,
            Vrm10Instance target,
            float time,
            Action<ApplyResult> onComplete)
        {
            var result = new ApplyResult { ok = false, error = null };

            if (target == null)
            {
                result.error = "target Vrm10Instance is null";
                onComplete?.Invoke(result);
                yield break;
            }

            if (string.IsNullOrEmpty(vrmaPath) || !File.Exists(vrmaPath))
            {
                result.error = $"vrma file not found: {vrmaPath}";
                onComplete?.Invoke(result);
                yield break;
            }

            if (target.Runtime == null)
            {
                result.error = "Vrm10Instance.Runtime is null — ensure LoadPathAsync completed in PlayMode";
                onComplete?.Invoke(result);
                yield break;
            }

            // Parse and load the .vrma.
            // AutoGltfFileParser + VrmAnimationData + VrmAnimationImporter is the
            // canonical path (SimpleVrma.cs, Vrm10PoseLoader.cs).
            RuntimeGltfInstance gltfInstance = null;
            // Typed as the interface to avoid pulling in Unity.Timeline
            // (Vrm10AnimationInstance implements ITimeControl from that assembly,
            // and the concrete type reference triggers a reference requirement).
            IVrm10Animation vrmaAnimation = null;

            try
            {
                using GltfData data = new AutoGltfFileParser(vrmaPath).Parse();
                var vrmaData = new VrmAnimationData(data);
                using var loader = new VrmAnimationImporter(vrmaData);

                // ImmediateCaller is safe in PlayMode and matches the rest of the
                // conformance pipeline (Conformance.cs, BatchRunner.cs).
                var loadTask = loader.LoadAsync(new ImmediateCaller());
                if (!loadTask.IsCompletedSuccessfully)
                {
                    result.error = $"VrmAnimationImporter.LoadAsync failed: {loadTask.Exception}";
                    onComplete?.Invoke(result);
                    yield break;
                }

                gltfInstance = loadTask.Result;
                // Query via the interface type — Unity's GetComponent<T> supports
                // interface queries (same pattern as GetComponent<IVrm10SpringBoneRuntimeProvider>
                // in Vrm10Instance.cs). This avoids referencing Vrm10AnimationInstance
                // directly, which would pull in Unity.Timeline (the concrete type
                // implements ITimeControl from that assembly).
                vrmaAnimation = gltfInstance.GetComponent<IVrm10Animation>();
            }
            catch (Exception e)
            {
                result.error = $"vrma load exception: {e}";
                onComplete?.Invoke(result);
                yield break;
            }

            if (vrmaAnimation == null)
            {
                if (gltfInstance != null)
                    UnityEngine.Object.DestroyImmediate(gltfInstance.gameObject);
                result.error = "IVrm10Animation component not found on loaded vrma GameObject";
                onComplete?.Invoke(result);
                yield break;
            }

            // Yield one frame so the freshly-instantiated source GO has had
            // Awake/Start called before we poke its Animation component.
            yield return null;

            try
            {
                // Sample the source animation at the requested time.
                // Mirrors Vrm10AnimationInstance.ITimeControl.SetTime(t):
                //   state.speed = 0; animation.Play(); state.time = t; animation.Sample();
                var animation = gltfInstance.GetComponent<Animation>();
                if (animation != null)
                {
                    AnimationState firstState = null;
                    foreach (AnimationState s in animation)
                    {
                        firstState = s;
                        break;
                    }

                    if (firstState != null)
                    {
                        firstState.speed = 0f;
                        firstState.enabled = true;
                        animation.Play();
                        firstState.time = time;
                        animation.Sample();
                    }
                    // If no clips exist (static pose .vrma), the ControlRig
                    // is already at rest pose — still valid to retarget.
                }

                // Wire up the source to the target and drive one retarget pass.
                // Vrm10Runtime.Process() reads VrmAnimation.ControlRig and calls
                // Vrm10Retarget.Retarget, then applies ExpressionMap weights
                // and LookAt. This is the canonical apply path.
                target.Runtime.VrmAnimation = vrmaAnimation;
                target.Runtime.Process();
            }
            catch (Exception e)
            {
                if (gltfInstance != null)
                    UnityEngine.Object.DestroyImmediate(gltfInstance.gameObject);
                result.error = $"vrma apply exception: {e}";
                onComplete?.Invoke(result);
                yield break;
            }

            // Tear down the source GO — the pose has been retargeted into the
            // target's skeleton/expressions; we no longer need the source.
            // Clear the Runtime reference first so Process() has no dangling ref.
            target.Runtime.VrmAnimation = null;
            UnityEngine.Object.DestroyImmediate(gltfInstance.gameObject);

            result.ok = true;
            onComplete?.Invoke(result);
        }

        // VRM 1.0 spec humanoid bones — 15 required + 4 commonly-supported optional.
        // Order matches the VRMA spec's humanBones declaration order.
        private static readonly (HumanBodyBones Enum, string VrmName)[] HumanoidBones = new[]
        {
            (HumanBodyBones.Hips,          "hips"),
            (HumanBodyBones.Spine,         "spine"),
            (HumanBodyBones.Chest,         "chest"),
            (HumanBodyBones.Neck,          "neck"),
            (HumanBodyBones.Head,          "head"),
            (HumanBodyBones.LeftShoulder,  "leftShoulder"),
            (HumanBodyBones.LeftUpperArm,  "leftUpperArm"),
            (HumanBodyBones.LeftLowerArm,  "leftLowerArm"),
            (HumanBodyBones.LeftHand,      "leftHand"),
            (HumanBodyBones.RightShoulder, "rightShoulder"),
            (HumanBodyBones.RightUpperArm, "rightUpperArm"),
            (HumanBodyBones.RightLowerArm, "rightLowerArm"),
            (HumanBodyBones.RightHand,     "rightHand"),
            (HumanBodyBones.LeftUpperLeg,  "leftUpperLeg"),
            (HumanBodyBones.LeftLowerLeg,  "leftLowerLeg"),
            (HumanBodyBones.LeftFoot,      "leftFoot"),
            (HumanBodyBones.RightUpperLeg, "rightUpperLeg"),
            (HumanBodyBones.RightLowerLeg, "rightLowerLeg"),
            (HumanBodyBones.RightFoot,     "rightFoot"),
        };

        // 14 spec-defined VRMA presets. Order matches the spec's enum.
        // lookUp/lookDown/lookLeft/lookRight are excluded from VRMA preset
        // expressions per spec — they are driven by LookAt instead.
        private static readonly ExpressionPreset[] PresetEnum = new[]
        {
            ExpressionPreset.happy,
            ExpressionPreset.angry,
            ExpressionPreset.sad,
            ExpressionPreset.relaxed,
            ExpressionPreset.surprised,
            ExpressionPreset.aa,
            ExpressionPreset.ih,
            ExpressionPreset.ou,
            ExpressionPreset.ee,
            ExpressionPreset.oh,
            ExpressionPreset.blink,
            ExpressionPreset.blinkLeft,
            ExpressionPreset.blinkRight,
            ExpressionPreset.neutral,
        };

        private static void AppendExpressions(StringBuilder sb, Vrm10Instance target)
        {
            var inv = System.Globalization.CultureInfo.InvariantCulture;

            sb.Append("\"expressions\":{\"presets\":{");
            bool first = true;
            foreach (var preset in PresetEnum)
            {
                var key = ExpressionKey.CreateFromPreset(preset);
                float weight = target.Runtime.Expression.GetWeight(key);
                if (!first) sb.Append(',');
                first = false;
                sb.AppendFormat(inv, "\"{0}\":{1}", preset.ToString(), weight);
            }
            sb.Append("},\"custom\":{");

            // Iterate all expression keys known to the runtime; emit those whose
            // Preset == ExpressionPreset.custom (i.e. non-preset / author-defined).
            // ExpressionKeys is IReadOnlyList<ExpressionKey> populated during
            // Vrm10RuntimeExpression construction from target.Vrm.Expression.Clips.
            bool firstCustom = true;
            foreach (var key in target.Runtime.Expression.ExpressionKeys)
            {
                if (key.Preset != ExpressionPreset.custom) continue;
                float weight = target.Runtime.Expression.GetWeight(key);
                if (!firstCustom) sb.Append(',');
                firstCustom = false;
                sb.AppendFormat(inv, "\"{0}\":{1}", key.Name, weight);
            }

            sb.Append("}}");
        }

        private static void AppendHumanoidPose(StringBuilder sb, Vrm10Instance target)
        {
            sb.Append("\"humanoid\":{\"bones\":[");

            bool first = true;
            var missing = new System.Collections.Generic.List<string>();
            var inv = System.Globalization.CultureInfo.InvariantCulture;

            Transform hipsT = null;

            foreach (var (boneEnum, vrmName) in HumanoidBones)
            {
                if (!target.TryGetBoneTransform(boneEnum, out var t))
                {
                    missing.Add(vrmName);
                    continue;
                }
                if (boneEnum == HumanBodyBones.Hips) hipsT = t;

                if (!first) sb.Append(',');
                first = false;
                var q = t.localRotation;
                sb.AppendFormat(inv,
                    "{{\"name\":\"{0}\",\"local_rotation_quat\":[{1},{2},{3},{4}]}}",
                    vrmName, q.x, q.y, q.z, q.w);
            }

            sb.Append("],");

            if (hipsT != null)
            {
                var p = hipsT.localPosition;
                sb.AppendFormat(inv,
                    "\"hips_translation\":[{0},{1},{2}]",
                    p.x, p.y, p.z);
            }
            else
            {
                sb.Append("\"hips_translation\":[0.0,0.0,0.0]");
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
        }

        public static string BuildPoseJson(Vrm10Instance target)
        {
            var sb = new StringBuilder(2048);
            sb.Append('{');
            AppendHumanoidPose(sb, target);
            sb.Append(',');
            AppendExpressions(sb, target);
            sb.Append(",\"look_at\":null}");
            return sb.ToString();
        }

        public static void WritePoseJson(string path, string json)
        {
            var dir = Path.GetDirectoryName(path);
            if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);
            File.WriteAllText(path, json, Encoding.UTF8);
        }
    }
}
