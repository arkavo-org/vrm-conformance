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
            if (t.animation != null && t.animation.vrma != null)
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
