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
    }
}
