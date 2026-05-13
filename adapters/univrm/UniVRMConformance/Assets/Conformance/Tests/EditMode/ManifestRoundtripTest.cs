// EditMode test: locks the Manifest DTO contract by serializing a
// known-good manifest body through JsonUtility and asserting key
// fields survive the round-trip. Runs inside Unity Test Framework;
// no scene, no Play mode.

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
                { ""test_id"": ""alpha"", ""vrm_path"": ""/tmp/alpha.vrm"", ""spec_section"": ""VRMC_materials_mtoon"" },
                { ""test_id"": ""bravo"", ""vrm_path"": ""/tmp/bravo.vrm"", ""spec_section"": ""VRMC_materials_mtoon"" }
            ]
        }";

        [Test]
        public void ManifestDeserializesPreservingTestIds()
        {
            var manifest = JsonUtility.FromJson<ManifestDto>(FixtureJson);
            Assert.IsNotNull(manifest, "manifest should parse");
            Assert.AreEqual(1, manifest.manifest_version);
            Assert.AreEqual("univrm", manifest.renderer_name);
            Assert.AreEqual(2, manifest.tests.Length);
            Assert.AreEqual("alpha", manifest.tests[0].test_id);
            Assert.AreEqual("/tmp/alpha.vrm", manifest.tests[0].vrm_path);
            Assert.AreEqual("bravo", manifest.tests[1].test_id);
        }
    }
}
