// Per-test rendering capture. Owns RenderTexture lifecycle, MSAA,
// color-space flag, ReadPixels, PNG encode. Does NOT own scene setup —
// caller (Conformance.RunBatch) configures the camera, then hands it
// to Capture.Render along with the output spec.
//
// Linear color-space output is intentionally lossy (8-bit / channel,
// no sRGB OETF). Use Srgb for any SSIM-grade comparison; Linear is
// diagnostic only.

using System.IO;
using UnityEngine;

namespace Conformance
{
    public static class Capture
    {
        public struct Result
        {
            public string outputPath;
            public string actualColorSpace;
            public float renderSeconds;
        }

        public static Result Render(Camera cam, Manifest.OutputDto output, string outputPath)
        {
            var sw = System.Diagnostics.Stopwatch.StartNew();

            var sRgb = output.color_space == "Srgb";
            var desc = new RenderTextureDescriptor(output.width, output.height, RenderTextureFormat.ARGB32, 24);
            desc.sRGB = sRgb;
            desc.msaaSamples = Mathf.Max(1, output.msaa);

            var rt = new RenderTexture(desc);
            rt.Create();
            cam.targetTexture = rt;

            try
            {
                cam.Render();

                var prev = RenderTexture.active;
                RenderTexture.active = rt;
                var tex = new Texture2D(output.width, output.height, TextureFormat.RGBA32, mipChain: false, linear: !sRgb);
                tex.ReadPixels(new Rect(0, 0, output.width, output.height), 0, 0);
                tex.Apply();
                RenderTexture.active = prev;

                var png = tex.EncodeToPNG();
                Directory.CreateDirectory(Path.GetDirectoryName(outputPath));
                File.WriteAllBytes(outputPath, png);

                Object.DestroyImmediate(tex);
            }
            finally
            {
                cam.targetTexture = null;
                rt.Release();
                Object.DestroyImmediate(rt);
            }

            sw.Stop();
            return new Result
            {
                outputPath = outputPath,
                actualColorSpace = sRgb ? "Srgb" : "Linear",
                renderSeconds = (float)sw.Elapsed.TotalSeconds,
            };
        }
    }
}
