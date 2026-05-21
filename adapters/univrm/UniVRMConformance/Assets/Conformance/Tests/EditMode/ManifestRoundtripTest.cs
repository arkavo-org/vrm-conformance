// EditMode test: locks the extended Manifest DTO contract. Asserts every
// per-test field the Rust runner emits survives JsonUtility round-trip.

using NUnit.Framework;
using UnityEngine;

namespace Conformance.Tests
{
    public class ManifestRoundtripTest
    {
        private const string FixtureJson = @"{
            ""manifest_version"": 1,
            ""output_dir"": ""/tmp/out"",
            ""renderer_name"": ""univrm"",
            ""tests"": [
                {
                    ""test_id"": ""mtoon_default"",
                    ""vrm_path"": ""/tmp/mtoon_default.vrm"",
                    ""spec_section"": ""VRMC_materials_mtoon"",
                    ""camera"": {
                        ""position"": [0.0, 1.4, 1.5],
                        ""target"":   [0.0, 1.4, 0.0],
                        ""up"":       [0.0, 1.0, 0.0],
                        ""fov_degrees"": 30.0
                    },
                    ""lighting"": {
                        ""directional"": {
                            ""dir"":       [-0.3, -0.6, -0.7],
                            ""color"":     [1.0, 1.0, 1.0],
                            ""intensity"": 1.0
                        },
                        ""ambient"": {
                            ""color"":     [0.5, 0.5, 0.5],
                            ""intensity"": 0.3
                        },
                        ""cast_shadows"": false,
                        ""receive_shadows"": false
                    },
                    ""post_processing"": {
                        ""tone_mapping"": ""None"",
                        ""exposure"": 1.0
                    },
                    ""output"": {
                        ""width"": 1024,
                        ""height"": 1024,
                        ""color_space"": ""Srgb"",
                        ""msaa"": 4
                    }
                }
            ]
        }";

        [Test]
        public void ManifestDeserializesPreservingTestIds()
        {
            var manifest = JsonUtility.FromJson<Manifest.ManifestDto>(FixtureJson);
            Assert.IsNotNull(manifest, "manifest should parse");
            Assert.AreEqual(1, manifest.manifest_version);
            Assert.AreEqual("univrm", manifest.renderer_name);
            Assert.AreEqual(1, manifest.tests.Length);
            Assert.AreEqual("mtoon_default", manifest.tests[0].test_id);
        }

        [Test]
        public void ManifestDeserializesCameraParams()
        {
            var manifest = JsonUtility.FromJson<Manifest.ManifestDto>(FixtureJson);
            var c = manifest.tests[0].camera;
            Assert.AreEqual(0f, c.position[0], 1e-6);
            Assert.AreEqual(1.4f, c.position[1], 1e-6);
            Assert.AreEqual(1.5f, c.position[2], 1e-6);
            Assert.AreEqual(30f, c.fov_degrees, 1e-6);
        }

        [Test]
        public void ManifestDeserializesLightingParams()
        {
            var manifest = JsonUtility.FromJson<Manifest.ManifestDto>(FixtureJson);
            var l = manifest.tests[0].lighting;
            Assert.AreEqual(-0.3f, l.directional.dir[0], 1e-6);
            Assert.AreEqual(-0.6f, l.directional.dir[1], 1e-6);
            Assert.AreEqual(-0.7f, l.directional.dir[2], 1e-6);
            Assert.AreEqual(1f, l.directional.intensity, 1e-6);
            Assert.AreEqual(0.5f, l.ambient.color[0], 1e-6);
            Assert.AreEqual(0.3f, l.ambient.intensity, 1e-6);
            Assert.IsFalse(l.cast_shadows);
            Assert.IsFalse(l.receive_shadows);
        }

        [Test]
        public void ManifestDeserializesOutputParams()
        {
            var manifest = JsonUtility.FromJson<Manifest.ManifestDto>(FixtureJson);
            var o = manifest.tests[0].output;
            Assert.AreEqual(1024, o.width);
            Assert.AreEqual(1024, o.height);
            Assert.AreEqual("Srgb", o.color_space);
            Assert.AreEqual(4, o.msaa);
        }

        [Test]
        public void ManifestDeserializesPostProcessing()
        {
            var manifest = JsonUtility.FromJson<Manifest.ManifestDto>(FixtureJson);
            var pp = manifest.tests[0].post_processing;
            Assert.AreEqual("None", pp.tone_mapping);
            Assert.AreEqual(1f, pp.exposure, 1e-6);
        }

        [Test]
        public void AnimationDtoRoundtripsVrma()
        {
            var input = new Manifest.AnimationDto
            {
                root_transform = null,
                vrma = new Manifest.VrmaDto { path = "/tmp/x.vrma", apply_at_time = 0.5f },
            };
            var json = JsonUtility.ToJson(input);
            var back = JsonUtility.FromJson<Manifest.AnimationDto>(json);
            Assert.AreEqual("/tmp/x.vrma", back.vrma.path);
            Assert.AreEqual(0.5f, back.vrma.apply_at_time);
            Assert.IsNull(back.root_transform);
            StringAssert.Contains("\"vrma\":", json);
        }

        // Regression — JsonUtility instantiates an absent nested object as a
        // default-constructed instance (frame_count=0), not null. Dispatch
        // checks that only test `t.render_sequence != null` would treat every
        // static-frame plan as a sequence test and reject it on `frame_count
        // must be >= 1`. The contract here pins the payload-bearing check
        // that BatchRunner.cs:224 and Conformance.cs:161 must use.
        [Test]
        public void StaticFrameManifestNotDetectedAsSequence()
        {
            var manifest = JsonUtility.FromJson<Manifest.ManifestDto>(FixtureJson);
            var t = manifest.tests[0];

            // The quirk itself — frame_count defaults to 0 for an absent field.
            Assert.AreEqual(0, t.render_sequence.frame_count,
                "absent render_sequence still yields a default DTO with frame_count=0");

            // The payload-bearing presence check — what dispatch sites must use.
            bool isSequence = t.render_sequence != null && t.render_sequence.frame_count > 0;
            Assert.IsFalse(isSequence, "static-frame plans must NOT enter sequence dispatch");
        }

        [Test]
        public void SequenceManifestDetectedAsSequence()
        {
            const string sequenceJson = @"{
                ""manifest_version"": 1,
                ""output_dir"": ""/tmp/out"",
                ""renderer_name"": ""univrm"",
                ""tests"": [
                    {
                        ""test_id"": ""swing_seq_default"",
                        ""vrm_path"": ""/tmp/x.vrm"",
                        ""spec_section"": ""VRMC_springBone"",
                        ""render_sequence"": {
                            ""frame_count"": 60,
                            ""frame_hz"": 60.0,
                            ""physics_dt_seconds"": 0.01666667,
                            ""output_format"": ""png_sequence""
                        }
                    }
                ]
            }";
            var manifest = JsonUtility.FromJson<Manifest.ManifestDto>(sequenceJson);
            var t = manifest.tests[0];

            Assert.AreEqual(60, t.render_sequence.frame_count);
            bool isSequence = t.render_sequence != null && t.render_sequence.frame_count > 0;
            Assert.IsTrue(isSequence, "render_sequence plans with frame_count > 0 must enter sequence dispatch");
        }
    }
}
