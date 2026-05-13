// L1+L2 stub (post-refactor): parses the manifest, writes one `_meta` line
// + one `Unimplemented` entry per test_id, exits cleanly. Real rendering
// arrives in L3 Task 11; spring-bone physics in L4.
//
// DTOs live in Manifest.cs. The full per-test rendering pipeline replaces
// this Unimplemented loop in Task 11.

using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using UnityEditor;
using UnityEngine;

namespace Conformance
{
    public static class Conformance
    {
        public static void RunBatch()
        {
            try
            {
                var args = ExtractAdapterArgs();
                if (args.Count < 2)
                {
                    Debug.LogError(
                        $"Conformance.RunBatch: expected 2 args (manifest, results); got {args.Count}");
                    EditorApplication.Exit(2);
                    return;
                }
                var manifestPath = args[0];
                var resultsPath = args[1];

                var manifestJson = File.ReadAllText(manifestPath);
                var manifest = JsonUtility.FromJson<Manifest.ManifestDto>(manifestJson);
                if (manifest == null || manifest.tests == null)
                {
                    Debug.LogError($"Conformance.RunBatch: failed to parse manifest at {manifestPath}");
                    EditorApplication.Exit(3);
                    return;
                }

                using var stream = new FileStream(
                    resultsPath, FileMode.Create, FileAccess.Write, FileShare.Read);

                // _meta envelope (line 1).
                var meta = new Manifest.MetaDto
                {
                    _meta = true,
                    manifest_version = manifest.manifest_version,
                    renderer_name = manifest.renderer_name,
                    renderer_version = "L1L2-stub",
                    unity_version = Application.unityVersion,
                    render_pipeline = "Built-in RP",
                    total_tests = manifest.tests.Length,
                };
                WriteLine(stream, JsonUtility.ToJson(meta));

                // One entry per test_id: all Unimplemented at this layer.
                foreach (var t in manifest.tests)
                {
                    var entry = new Manifest.EntryDto
                    {
                        test_id = t.test_id,
                        status = "error",
                        error = new Manifest.ErrorDto
                        {
                            code = -32000,
                            message = "Unimplemented (L1+L2 stub)",
                            data = new Manifest.ErrorDataDto { phase = "L3" },
                        },
                    };
                    WriteLine(stream, JsonUtility.ToJson(entry));
                }

                EditorApplication.Exit(0);
            }
            catch (Exception e)
            {
                Debug.LogError($"Conformance.RunBatch: unhandled exception: {e}");
                EditorApplication.Exit(1);
            }
        }

        private static List<string> ExtractAdapterArgs()
        {
            var args = Environment.GetCommandLineArgs();
            var result = new List<string>();
            var capture = false;
            foreach (var a in args)
            {
                if (capture)
                {
                    result.Add(a);
                }
                else if (a == "--")
                {
                    capture = true;
                }
            }
            return result;
        }

        private static void WriteLine(FileStream stream, string json)
        {
            var bytes = Encoding.UTF8.GetBytes(json + "\n");
            stream.Write(bytes, 0, bytes.Length);
            // Flush-to-disk after each entry: survives OOM kill / segfault
            // mid-batch. See docs/superpowers/specs/2026-05-12-adapter-
            // univrm-design.md "Partial output" for rationale.
            stream.Flush(flushToDisk: true);
        }
    }
}
