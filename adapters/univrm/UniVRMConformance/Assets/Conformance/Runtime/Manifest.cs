// Single source of truth for the wire JSON shapes shared with the Rust
// runner. JsonUtility-friendly: every type is [Serializable], every
// field is public, no generics, no nullable value types, no IDictionary.
// Arrays-of-float for vec3/vec4 because JsonUtility cannot serialize
// nested struct fields without ScriptableObject overhead.
//
// Mirrors crates/vrm-runner/src/execute_batch.rs BatchManifest +
// BatchTestEntry and vrm-test-plan TestPlan. When the Rust side adds
// a field, mirror it here, extend ManifestRoundtripTest, and bump
// manifest_version.

using System;

namespace Conformance
{
    public static class Manifest
    {
        // ============== runner → Unity ==============

        [Serializable]
        public class ManifestDto
        {
            public int manifest_version;
            public string output_dir;
            public string renderer_name;
            public string renderer_version;
            public TestEntryDto[] tests;
        }

        [Serializable]
        public class TestEntryDto
        {
            public string test_id;
            public string vrm_path;
            public string spec_section;
            public CameraDto camera;
            public LightingDto lighting;
            public PostProcessingDto post_processing;
            public OutputDto output;
            public PhysicsDto physics;
            public AnimationDto animation;
        }

        [Serializable]
        public class CameraDto
        {
            public float[] position;
            public float[] target;
            public float[] up;
            public float fov_degrees;
        }

        [Serializable]
        public class LightingDto
        {
            public DirectionalDto directional;
            public AmbientDto ambient;
            public bool cast_shadows;
            public bool receive_shadows;
        }

        [Serializable]
        public class DirectionalDto
        {
            public float[] dir;
            public float[] color;
            public float intensity;
        }

        [Serializable]
        public class AmbientDto
        {
            public float[] color;
            public float intensity;
        }

        [Serializable]
        public class PostProcessingDto
        {
            public string tone_mapping;
            public float exposure;
        }

        [Serializable]
        public class OutputDto
        {
            public int width;
            public int height;
            public string color_space;
            public int msaa;
        }

        [Serializable]
        public class PhysicsDto
        {
            public int settle_steps;
        }

        [Serializable]
        public class AnimationDto
        {
            public RootTransformDto root_transform;
        }

        [Serializable]
        public class RootTransformDto
        {
            public float[] translation_start;
            public float[] translation_end;
            public float duration_seconds;
            public int fps;
        }

        // ============== Unity → runner ==============

        [Serializable]
        public class MetaDto
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
        public class EntryDto
        {
            public string test_id;
            public string status;
            public string output_path;
            public string actual_color_space;
            public float render_seconds;
            public ErrorDto error;
        }

        [Serializable]
        public class ErrorDto
        {
            public int code;
            public string message;
            public ErrorDataDto data;
        }

        [Serializable]
        public class ErrorDataDto
        {
            public string phase;
            public string feature;
            public string value;
            public string[] supported;
        }
    }
}
