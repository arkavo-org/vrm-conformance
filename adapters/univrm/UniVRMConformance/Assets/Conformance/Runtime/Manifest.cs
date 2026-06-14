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
            public RenderSequenceDto render_sequence;
            public BenchmarkDto benchmark;
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
            public VrmaDto vrma;
        }

        [Serializable]
        public class VrmaDto
        {
            public string path;
            public float apply_at_time;
        }

        [Serializable]
        public class RootTransformDto
        {
            public float[] translation_start;
            public float[] translation_end;
            public float duration_seconds;
            public int fps;
        }

        [Serializable]
        public class RenderSequenceDto
        {
            public int frame_count;
            public float frame_hz;
            public float physics_dt_seconds;
            public string output_format;                            // "png_sequence"
            public RenderSequenceAnimateDto animate_root_transform; // may be null
            public RenderSequenceVrmaDto apply_vrma;                // may be null
            public float temporal_ssim_threshold;                   // 0 ⇒ unset (use RFC default)
            public bool capture_positions;                          // emit per-frame spring positions
        }

        [Serializable]
        public class RenderSequenceAnimateDto
        {
            public float[] translation_start;
            public float[] translation_end;
        }

        [Serializable]
        public class RenderSequenceVrmaDto
        {
            public int vrma_handle;
            public float start_seconds;
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
            /// <summary>
            /// Source spec version of the loaded VRM asset, detected from
            /// extensionsUsed in the GLB JSON chunk. "0.x" for VRM 0.x
            /// (extension "VRM"), "1.0" for VRM 1.0 (extension "VRMC_vrm").
            /// Null/empty when the load failed before detection (error entries).
            /// </summary>
            public string source_spec_version;
            public ErrorDto error;
            // Sequence-shape result fields. Null/zero for single-frame entries.
            public RenderSequenceFrameOutputDto[] frames;
            public float duration_seconds;
            public float frame_hz_achieved;
            // Benchmark measurement. Null for non-benchmark entries.
            public PerfMeasurementDto measurement;
        }

        [Serializable]
        public class RenderSequenceFrameOutputDto
        {
            public int index;
            public float timestamp_seconds;
            public string path;
            public string blake3;
            // Per-spring joint positions, present only when capture_positions
            // was set. Null/empty for normal sequence frames.
            public SpringPositionsDto[] spring_positions;
        }

        // Per-spring joint world positions for one frame. joint_positions is
        // FLAT ([x0,y0,z0, x1,y1,z1, ...]) — Unity JsonUtility cannot emit
        // nested arrays, so this follows the adapter's flat-float[] vec3
        // convention. The runner reshapes to canonical [[x,y,z],...] on disk.
        [Serializable]
        public class SpringPositionsDto
        {
            public string name;
            public float[] joint_positions;
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

        // ============== benchmark DTOs (runner → Unity request; Unity → runner result) ==============

        /// <summary>
        /// Benchmark parameters stamped on a test entry by the runner when
        /// `execute-test-batch --benchmark` is used. Unity runs a warmup+measured
        /// render loop and returns a PerfMeasurementDto on the result entry.
        /// </summary>
        [Serializable]
        public class BenchmarkDto
        {
            public int warmup_frames;
            public int measured_frames;
            public bool animate;
        }

        [Serializable]
        public class PerfFrameTimeDto
        {
            public float p50;
            public float p95;
            public float p99;
        }

        [Serializable]
        public class PerfTimingDto
        {
            public PerfFrameTimeDto frame_time_ms;
            public float fps_mean;
            public string clock; // "cpu"
        }

        /// <summary>
        /// Structural metrics. ONLY draw_calls is declared — state_changes and
        /// texture_bindings are not exposed by Unity, so they are intentionally
        /// absent here to avoid emitting false-zero values. Absent C# field →
        /// absent JSON key → Rust reads None (honesty principle).
        /// </summary>
        [Serializable]
        public class PerfStructuralDto
        {
            public float draw_calls; // ONLY draw_calls — state_changes/texture_bindings NOT declared
        }

        [Serializable]
        public class PerfGeometryDto
        {
            public long triangles;
            public long vertices; // UnityStats.vertices IS available
        }

        [Serializable]
        public class PerfResourcesDto
        {
            public long peak_memory_bytes;
            public string memory_kind; // "host"
            public float load_ms;
            public float first_frame_ms;
        }

        [Serializable]
        public class PerfHostDto
        {
            public string os;
            public string os_version;
            public string gpu_vendor;
            public string gpu_model;
            public string driver_version;
            public string build_flags;
        }

        [Serializable]
        public class PerfProtocolDto
        {
            public int warmup_frames;
            public int measured_frames;
            public bool animated;
        }

        [Serializable]
        public class PerfMeasurementDto
        {
            public PerfProtocolDto protocol;
            public PerfTimingDto timing;
            public PerfStructuralDto structural;
            public PerfGeometryDto geometry;
            public PerfResourcesDto resources;
            public PerfHostDto host;
            public string[] capabilities;
        }
    }
}
