// PlayMode batch entry — L4. Runs Conformance.RunBatch logic inside
// PlayMode so UniVRM's FastSpringBone runtime initializes
// (Application.isPlaying == true), and PhysicsDriver.Settle /
// AnimateRootTransform become real instead of no-ops.
//
// Invoked from launcher.sh as:
//   Unity -batchmode -projectPath ... \
//         -runTests -testPlatform PlayMode \
//         -testFilter "Conformance.Tests.Play.BatchRunner.RunBatchInPlayMode" \
//         -- manifest.json results.ndjson
//
// Output goes to the same results.ndjson the EditMode entry writes; the
// Rust runner doesn't care which Unity entry point produced it.

using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Text;
using NUnit.Framework;
using UniGLTF;
using UnityEngine;
using UnityEngine.TestTools;
using UniVRM10;

namespace Conformance.Tests.Play
{
    public class BatchRunner
    {
        [UnityTest]
        public IEnumerator RunBatchInPlayMode()
        {
            // PlayMode is now active; spring-bone runtime can initialize.
            Assert.IsTrue(Application.isPlaying,
                "PlayMode required for UniVRM spring-bone runtime — this test runs the conformance batch");

            var args = ExtractAdapterArgs();
            if (args.Count < 2)
            {
                Assert.Fail($"expected 2 args after '--' (manifest, results); got {args.Count}");
                yield break;
            }
            var manifestPath = args[0];
            var resultsPath = args[1];

            if (!File.Exists(manifestPath))
            {
                Assert.Fail($"manifest not found at {manifestPath}");
                yield break;
            }

            var manifest = JsonUtility.FromJson<Manifest.ManifestDto>(File.ReadAllText(manifestPath));
            Assert.IsNotNull(manifest, "manifest must parse");
            Assert.IsNotNull(manifest.tests, "manifest.tests must be non-null");

            using var stream = new FileStream(
                resultsPath, FileMode.Create, FileAccess.Write, FileShare.Read);

            WriteLine(stream, JsonUtility.ToJson(new Manifest.MetaDto
            {
                _meta = true,
                manifest_version = manifest.manifest_version,
                renderer_name = manifest.renderer_name,
                renderer_version = "v0.131.0",
                unity_version = Application.unityVersion,
                render_pipeline = "Built-in RP",
                total_tests = manifest.tests.Length,
            }));

            // Iterate. PhysicsDriver now does real spring-bone work
            // because Application.isPlaying is true.
            foreach (var t in manifest.tests)
            {
                Manifest.EntryDto entry = null;
                yield return RenderOneCo(manifest.output_dir, manifest.renderer_name, t, e => entry = e);
                WriteLine(stream, JsonUtility.ToJson(entry));
            }

            stream.Close();
        }

        // Coroutine-form RenderOne. Same shape as Conformance.RenderOne
        // (the EditMode path), but with `yield return null` between key
        // steps so Unity has frames to process spring-bone scheduling and
        // shader compilation.
        private IEnumerator RenderOneCo(string outputDir, string rendererName, Manifest.TestEntryDto t, Action<Manifest.EntryDto> setEntry)
        {
            try
            {
                SceneSetup.AssertPostProcessingSupported(t.post_processing);
            }
            catch (SceneSetup.UnsupportedFeatureException ex)
            {
                setEntry(new Manifest.EntryDto
                {
                    test_id = t.test_id,
                    status = "error",
                    error = new Manifest.ErrorDto
                    {
                        code = -32602,
                        message = $"unsupported {ex.Feature}: {ex.Value}",
                        data = new Manifest.ErrorDataDto
                        {
                            feature = ex.Feature, value = ex.Value, supported = ex.Supported,
                        },
                    },
                });
                yield break;
            }

            GameObject vrmGo = null, lightGo = null, cameraGo = null;
            Manifest.EntryDto result = null;

            // Load — synchronous via ImmediateCaller works in PlayMode too.
            System.Threading.Tasks.Task<Vrm10Instance> loadTask = null;
            Exception loadException = null;
            try
            {
                loadTask = Vrm10.LoadPathAsync(
                    t.vrm_path,
                    canLoadVrm0X: false,
                    showMeshes: true,
                    awaitCaller: new ImmediateCaller(),
                    ct: System.Threading.CancellationToken.None);
            }
            catch (Exception e)
            {
                loadException = e;
            }

            if (loadException != null || loadTask == null || !loadTask.IsCompletedSuccessfully)
            {
                setEntry(ErrorEntry(t.test_id, -32001, "LoadFailed", "L4",
                    loadException?.ToString() ?? loadTask?.Exception?.ToString()));
                yield break;
            }

            var vrm = loadTask.Result;
            vrmGo = vrm.gameObject;

            // Let the scene tick once so spring-bone runtime finishes any
            // PlayMode-side init (Awake/Start on the freshly-instantiated
            // Vrm10Instance components).
            yield return null;

            // Setup phase — no yield, so a plain try/catch is fine here.
            Camera cam = null;
            Exception setupException = null;
            try
            {
                PhysicsDriver.DisableAutoUpdate(vrm);

                cameraGo = new GameObject("Camera");
                cam = cameraGo.AddComponent<Camera>();
                SceneSetup.ConfigureCamera(cam, t.camera);

                lightGo = new GameObject("Directional");
                var light = lightGo.AddComponent<Light>();
                SceneSetup.ConfigureLighting(light, t.lighting);

                // Spring-bone settle + animation. PlayMode == real work.
                PhysicsDriver.Settle(vrm, t.physics);
                PhysicsDriver.AnimateRootTransform(vrm, t.animation);
            }
            catch (Exception e)
            {
                setupException = e;
            }

            if (setupException != null)
            {
                if (cameraGo != null) UnityEngine.Object.DestroyImmediate(cameraGo);
                if (lightGo != null) UnityEngine.Object.DestroyImmediate(lightGo);
                if (vrmGo != null) UnityEngine.Object.DestroyImmediate(vrmGo);
                setEntry(ErrorEntry(t.test_id, -32002, "RenderFailed", "L4", setupException.ToString()));
                yield break;
            }

            // VRMA — load + apply at time t, then dump pose state alongside the
            // rendered PNG. Only runs when the test plan declares animation.vrma.
            // Yielding the coroutine requires being outside a try-with-catch block
            // (C# iterator restriction), hence the split from the setup phase above.
            // JsonUtility deserializes absent JSON sub-objects as default-
            // constructed instances rather than null (Unity quirk). So a plan
            // with `animation: { root_transform: {...} }` and no `vrma` field
            // still produces a non-null `t.animation.vrma` instance with an
            // empty `path`. Guard on both null AND empty-path so non-VRMA
            // test plans (e.g. the spring-bone swing sweep) skip the VRMA
            // branch cleanly instead of trying to load "" and reporting
            // VrmaApplyFailed on tests that aren't VRMA tests at all.
            if (t.animation != null
                && t.animation.vrma != null
                && !string.IsNullOrEmpty(t.animation.vrma.path))
            {
                VrmaDriver.ApplyResult vrmaResult = null;
                yield return VrmaDriver.LoadAndApply(
                    t.animation.vrma.path,
                    vrm,
                    t.animation.vrma.apply_at_time,
                    r => { vrmaResult = r; });

                if (vrmaResult == null || !vrmaResult.ok)
                {
                    if (cameraGo != null) UnityEngine.Object.DestroyImmediate(cameraGo);
                    if (lightGo != null) UnityEngine.Object.DestroyImmediate(lightGo);
                    if (vrmGo != null) UnityEngine.Object.DestroyImmediate(vrmGo);
                    setEntry(ErrorEntry(
                        t.test_id, -32000, "VrmaApplyFailed", "L4",
                        vrmaResult?.error ?? "VrmaDriver did not invoke onComplete"));
                    yield break;
                }

                var poseJson = VrmaDriver.BuildPoseJson(vrm);
                var posePath = Path.Combine(
                    outputDir,
                    $"{t.test_id}_{rendererName}.pose.json");
                VrmaDriver.WritePoseJson(posePath, poseJson);
            }

            // Sequence-mode dispatch (RFC-0004 render_sequence). If the test plan
            // declares render_sequence, run a per-frame loop instead of the
            // single-frame render. Coroutine yields require splitting cleanup
            // from the loop (C# iterator restriction on try/finally across yield).
            if (t.render_sequence != null)
            {
                Manifest.EntryDto sequenceResult = null;
                yield return RenderSequenceCo(outputDir, rendererName, t, cam, vrm, e => sequenceResult = e);

                // Manual cleanup matching the single-frame finally block.
                if (cameraGo != null) UnityEngine.Object.DestroyImmediate(cameraGo);
                if (lightGo != null) UnityEngine.Object.DestroyImmediate(lightGo);
                if (vrmGo != null) UnityEngine.Object.DestroyImmediate(vrmGo);
                setEntry(sequenceResult);
                yield break;
            }

            // Render phase — has finally for guaranteed cleanup.
            try
            {
                var outputPath = Path.Combine(outputDir, t.test_id + ".png");
                var captureResult = Capture.Render(cam, t.output, outputPath);

                result = new Manifest.EntryDto
                {
                    test_id = t.test_id,
                    status = "ok",
                    output_path = captureResult.outputPath,
                    actual_color_space = captureResult.actualColorSpace,
                    render_seconds = captureResult.renderSeconds,
                };
            }
            catch (Exception e)
            {
                result = ErrorEntry(t.test_id, -32002, "RenderFailed", "L4", e.ToString());
            }
            finally
            {
                if (cameraGo != null) UnityEngine.Object.DestroyImmediate(cameraGo);
                if (lightGo != null) UnityEngine.Object.DestroyImmediate(lightGo);
                if (vrmGo != null) UnityEngine.Object.DestroyImmediate(vrmGo);
            }

            setEntry(result);
        }

        private IEnumerator RenderSequenceCo(
            string outputDir,
            string rendererName,
            Manifest.TestEntryDto t,
            Camera cam,
            UniVRM10.Vrm10Instance vrm,
            Action<Manifest.EntryDto> setEntry)
        {
            var rs = t.render_sequence;

            // ---- RFC-0004 validation ----
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

            // ---- Frames dir per the runner's convention:
            //   <output_dir>/<test_id>_<renderer>_frames/<NNNN>.png ----
            var framesDir = Path.Combine(outputDir, $"{t.test_id}_{rendererName}_frames");
            Exception dirErr = null;
            try { Directory.CreateDirectory(framesDir); }
            catch (Exception e) { dirErr = e; }
            if (dirErr != null)
            {
                setEntry(ErrorEntry(t.test_id, -32002, "RenderFailed", "L4-sequence",
                    $"create frames dir: {dirErr}"));
                yield break;
            }

            // ---- Snapshot origin + parse translations ----
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
            var framesOut = new List<Manifest.RenderSequenceFrameOutputDto>(rs.frame_count);

            // ---- Frame loop ----
            for (int i = 0; i < rs.frame_count; i++)
            {
                // Interpolate root translation. For frame_count==1, ti=0.
                float ti = rs.frame_count > 1 ? (float)i / (rs.frame_count - 1) : 0f;
                vrm.transform.position = origPosition + Vector3.Lerp(startV, endV, ti);

                // Step spring-bone physics one tick (matches AnimateRootTransform).
                if (runtime != null && runtime.SpringBone != null)
                {
                    runtime.SpringBone.Process(rs.physics_dt_seconds);
                }

                // Yield so Unity actually renders this frame.
                yield return null;

                // Capture to <i:04>.png. try/catch is safe here — no yield inside.
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

            // Restore original root.
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

        private static Manifest.EntryDto ErrorEntry(string test_id, int code, string label, string phase, string detail)
        {
            const int max = 1000;
            if (detail != null && detail.Length > max) detail = detail.Substring(0, max) + "…";
            return new Manifest.EntryDto
            {
                test_id = test_id,
                status = "error",
                error = new Manifest.ErrorDto
                {
                    code = code,
                    message = $"{label}: {detail ?? "no detail"}",
                    data = new Manifest.ErrorDataDto { phase = phase },
                },
            };
        }

        private static List<string> ExtractAdapterArgs()
        {
            var args = System.Environment.GetCommandLineArgs();
            var result = new List<string>();
            var capture = false;
            foreach (var a in args)
            {
                if (capture) result.Add(a);
                else if (a == "--") capture = true;
            }
            return result;
        }

        private static void WriteLine(FileStream stream, string json)
        {
            var bytes = Encoding.UTF8.GetBytes(json + "\n");
            stream.Write(bytes, 0, bytes.Length);
            stream.Flush(flushToDisk: true);
        }
    }
}
