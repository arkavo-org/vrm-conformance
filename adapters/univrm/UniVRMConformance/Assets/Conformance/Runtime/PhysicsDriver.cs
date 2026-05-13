// L4 — UniVRM spring-bone physics, manually stepped at 60 Hz.
//
// CURRENT STATUS: skeleton only. UniVRM's FastSpringBone runtime
// (Vrm10FastSpringboneRuntimeStandalone) requires `Application.isPlaying
// == true` to construct its internal Burst-compiled job buffers (per
// Packages/VRM10/Runtime/Components/Vrm10Runtime/Springbone/
// Vrm10FastSpringboneRuntimeStandalone.cs:111 in v0.131.0). Our batched
// adapter shape uses `Unity -batchmode -executeMethod`, which runs in
// EditMode where `Application.isPlaying == false`. The result is a
// NullReferenceException inside UniVRM the moment RestoreInitialTransform
// or Process is called.
//
// Closing this gap properly requires a PlayMode batch entry — a separate
// follow-up plan beyond L3. Until then, this driver detects EditMode
// and no-ops cleanly. Spring-bone tests render in rest pose (same as
// the L3 baseline) — well-formed PNGs, just no animated bones.
//
// Mirrors the godot-vrm L4 convention for when PlayMode arrives:
//   - DisableAutoUpdate before any stepping
//   - Settle from RestoreInitialTransform → N × Process(1/60)
//   - AnimateRootTransform → per-frame lerp + Process(1/fps)
//
// Determinism notes for when this path goes live:
// - VRM10's spring-bone runtime is FastSpringBone (Burst-compiled);
//   identical inputs (joint config + dt sequence + root transform
//   sequence) produce identical outputs across runs on the same host.
// - Cross-machine determinism is asserted, not proven; matches the
//   godot-vrm L4 story. Follow-up: bootstrap on different Apple
//   Silicon generations to verify.

using UnityEngine;
using UniVRM10;

namespace Conformance
{
    public static class PhysicsDriver
    {
        // 60 Hz fixed step matches the methodology pin in docs/methodology.md.
        private const float SettleStepHz = 60f;

        // True when UniVRM's spring-bone runtime is usable.
        // In EditMode batch (`Application.isPlaying == false`) the
        // FastSpringBone buffer is never built → all spring-bone API
        // calls throw NullReferenceException.
        private static bool SpringBoneAvailable
            => Application.isPlaying;

        // Disable auto-update so all stepping happens via Process(dt)
        // from this driver. Safe to call in EditMode (UpdateType setter
        // only flips an enum on the MonoBehaviour).
        public static void DisableAutoUpdate(Vrm10Instance instance)
        {
            instance.UpdateType = Vrm10Instance.UpdateTypes.None;
        }

        // Settle spring-bones from the rest pose for `settle_steps` × 1/60 s.
        // In EditMode: no-op (renders rest pose, same as L3 baseline).
        public static void Settle(Vrm10Instance instance, Manifest.PhysicsDto p)
        {
            if (p == null || p.settle_steps <= 0) return;
            if (!SpringBoneAvailable) return;

            var runtime = instance.Runtime;
            if (runtime == null || runtime.SpringBone == null) return;

            runtime.SpringBone.RestoreInitialTransform();

            var dt = 1f / SettleStepHz;
            for (int i = 0; i < p.settle_steps; i++)
            {
                runtime.SpringBone.Process(dt);
            }
        }

        // Animate the avatar root transform from translation_start to
        // translation_end over duration_seconds at fps, stepping the
        // spring-bone runtime each frame. Used by the swing tests to
        // excite drag/stiffness/inertia (gravity-only settle isn't enough).
        // In EditMode: applies the final root position (so the camera
        // composition reflects translation_end) but doesn't step physics —
        // bones stay in rest pose.
        public static void AnimateRootTransform(Vrm10Instance instance, Manifest.AnimationDto a)
        {
            if (a == null || a.root_transform == null) return;
            var rt = a.root_transform;
            if (rt.fps <= 0 || rt.duration_seconds <= 0) return;
            if (rt.translation_start == null || rt.translation_end == null) return;

            var start = SceneSetup.GltfToUnity(rt.translation_start);
            var end = SceneSetup.GltfToUnity(rt.translation_end);

            if (!SpringBoneAvailable)
            {
                // Park the avatar at translation_end so camera framing
                // matches what the swing tests expect to measure; bones
                // remain in rest pose without physics stepping.
                instance.transform.position = end;
                return;
            }

            int totalFrames = Mathf.Max(2, Mathf.RoundToInt(rt.duration_seconds * rt.fps));
            var dt = 1f / rt.fps;

            var runtime = instance.Runtime;

            for (int frame = 0; frame < totalFrames; frame++)
            {
                // t spans [0, 1] inclusive across frames so the last frame
                // sees translation_end exactly.
                var t = (float)frame / (totalFrames - 1);
                instance.transform.position = Vector3.Lerp(start, end, t);

                if (runtime != null && runtime.SpringBone != null)
                {
                    runtime.SpringBone.Process(dt);
                }
            }
        }
    }
}
