// L1+L2 stub: parses the manifest, writes one `_meta` line + one
// `Unimplemented` entry per test_id, exits cleanly. Real rendering
// arrives in L3; spring-bone physics in L4.
//
// Invoked from launcher.sh as:
//   Unity -batchmode -projectPath ... -executeMethod Conformance.RunBatch -- manifest.json results.ndjson
//
// The `--` separator routes everything after it into
// `Environment.GetCommandLineArgs()` as plain strings (Unity passes
// everything past `--` through unmodified).

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
                var manifest = JsonUtility.FromJson<ManifestDto>(manifestJson);
                if (manifest == null || manifest.tests == null)
                {
                    Debug.LogError($"Conformance.RunBatch: failed to parse manifest at {manifestPath}");
                    EditorApplication.Exit(3);
                    return;
                }

                using var stream = new FileStream(
                    resultsPath, FileMode.Create, FileAccess.Write, FileShare.Read);

                // _meta envelope (line 1).
                var meta = new MetaDto
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
                    var entry = new EntryDto
                    {
                        test_id = t.test_id,
                        status = "error",
                        error = new ErrorDto
                        {
                            code = -32000,
                            message = "Unimplemented (L1+L2 stub)",
                            data = new ErrorDataDto { phase = "L3" },
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

    [Serializable]
    internal class ManifestDto
    {
        public int manifest_version;
        public string output_dir;
        public string renderer_name;
        public string renderer_version;
        public TestEntryDto[] tests;
    }

    [Serializable]
    internal class TestEntryDto
    {
        public string test_id;
        public string vrm_path;
        public string spec_section;
    }

    [Serializable]
    internal class MetaDto
    {
        public bool _meta;
        public int manifest_version;
        public string renderer_name;
        public string renderer_version;
        public string unity_version;
        public string render_pipeline;
        public int total_tests;
    }

    [Serializable]
    internal class EntryDto
    {
        public string test_id;
        public string status;
        public ErrorDto error;
    }

    [Serializable]
    internal class ErrorDto
    {
        public int code;
        public string message;
        public ErrorDataDto data;
    }

    [Serializable]
    internal class ErrorDataDto
    {
        public string phase;
    }
}
