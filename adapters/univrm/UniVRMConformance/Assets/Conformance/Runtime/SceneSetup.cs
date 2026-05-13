// Scene configuration for the conformance suite. Owns:
//   - coordinate conversion (glTF right-handed Y-up → Unity left-handed Y-up)
//   - per-test camera setup (LookAt, FOV, magenta clear)
//   - per-test directional light + ambient
//   - per-test post-processing validation (None passes; others throw)
//
// Camera/light GameObjects are owned by callers; this class only sets
// values on already-spawned components.

using UnityEngine;

namespace Conformance
{
    public static class SceneSetup
    {
        public static Vector3 GltfToUnity(float[] v)
        {
            return new Vector3(v[0], v[1], -v[2]);
        }

        public static void ConfigureCamera(Camera cam, Manifest.CameraDto p)
        {
            cam.transform.position = GltfToUnity(p.position);
            var target = GltfToUnity(p.target);
            var up = GltfToUnity(p.up);
            cam.transform.LookAt(target, up);
            cam.fieldOfView = p.fov_degrees;
            cam.clearFlags = CameraClearFlags.SolidColor;
            cam.backgroundColor = new Color(1f, 0f, 1f, 1f);
        }

        public static void ConfigureLighting(Light light, Manifest.LightingDto p)
        {
            light.type = LightType.Directional;
            light.transform.forward = GltfToUnity(p.directional.dir).normalized;
            light.color = new Color(
                p.directional.color[0],
                p.directional.color[1],
                p.directional.color[2]);
            light.intensity = p.directional.intensity;
            light.shadows = p.cast_shadows ? LightShadows.Soft : LightShadows.None;

            RenderSettings.ambientMode = UnityEngine.Rendering.AmbientMode.Flat;
            RenderSettings.ambientLight = new Color(
                p.ambient.color[0] * p.ambient.intensity,
                p.ambient.color[1] * p.ambient.intensity,
                p.ambient.color[2] * p.ambient.intensity);
        }

        public class UnsupportedFeatureException : System.Exception
        {
            public string Feature { get; }
            public string Value { get; }
            public string[] Supported { get; }

            public UnsupportedFeatureException(string feature, string value, string[] supported)
                : base($"unsupported {feature}: {value}")
            {
                Feature = feature;
                Value = value;
                Supported = supported;
            }
        }

        public static void AssertPostProcessingSupported(Manifest.PostProcessingDto p)
        {
            // The runner serializes ToneMapping with `rename_all = "lowercase"`,
            // so the wire value is "none" (not "None"). Accept the canonical
            // serialized form.
            if (p.tone_mapping != "none")
            {
                throw new UnsupportedFeatureException(
                    feature: "tone_mapping",
                    value: p.tone_mapping,
                    supported: new[] { "none" });
            }
        }
    }
}
