// Task 28: EditMode tests for source_spec_version detection and wire serialization.
//
// Tests are organised into two groups:
//
//   1. Wire-shape tests (no Unity fixture required): confirm EntryDto.source_spec_version
//      is serialized/deserialized correctly by JsonUtility.
//
//   2. SpecVersionDetector unit tests: write a minimal synthetic GLB header to a
//      temp file and confirm the detector returns "0.x", "1.0", or null.
//
// Fixture-gated tests that load a real VRM 0.x asset are explicitly skipped
// when the fixture is absent; source_spec_version correctness on real assets
// will be verified via the end-of-slice bootstrap (Task 39).

using System;
using System.IO;
using System.Text;
using NUnit.Framework;
using UnityEngine;

namespace Conformance.Tests
{
    public class SourceSpecVersionTest
    {
        // ------------------------------------------------------------------ //
        // Wire-shape (EntryDto serialization)
        // ------------------------------------------------------------------ //

        [Test]
        public void EntryDtoSerializesSourceSpecVersionV0()
        {
            var entry = new Manifest.EntryDto
            {
                test_id = "test_v0",
                status = "ok",
                source_spec_version = "0.x",
            };
            var json = JsonUtility.ToJson(entry);
            StringAssert.Contains("\"source_spec_version\":\"0.x\"", json,
                "source_spec_version '0.x' must appear in serialized EntryDto");
        }

        [Test]
        public void EntryDtoSerializesSourceSpecVersionV1()
        {
            var entry = new Manifest.EntryDto
            {
                test_id = "test_v1",
                status = "ok",
                source_spec_version = "1.0",
            };
            var json = JsonUtility.ToJson(entry);
            StringAssert.Contains("\"source_spec_version\":\"1.0\"", json,
                "source_spec_version '1.0' must appear in serialized EntryDto");
        }

        [Test]
        public void EntryDtoRoundtripsSourceSpecVersion()
        {
            var original = new Manifest.EntryDto
            {
                test_id = "roundtrip",
                status = "ok",
                output_path = "/tmp/out.png",
                actual_color_space = "Srgb",
                source_spec_version = "0.x",
            };
            var json = JsonUtility.ToJson(original);
            var back  = JsonUtility.FromJson<Manifest.EntryDto>(json);
            Assert.AreEqual("0.x", back.source_spec_version,
                "source_spec_version must survive JSON round-trip");
        }

        // ------------------------------------------------------------------ //
        // SpecVersionDetector unit tests (synthetic GLB headers)
        // ------------------------------------------------------------------ //

        /// Build a minimal GLB binary whose JSON chunk contains the given JSON fragment.
        /// GLB layout:
        ///   [0-3]  magic       0x46546C67 ("glTF" in LE)
        ///   [4-7]  version     2 (u32 LE)
        ///   [8-11] totalLen    u32 LE
        ///   [12-15] chunk0Len  u32 LE
        ///   [16-19] chunk0Type 0x4E4F534A ("JSON" in LE)
        ///   [20..]  chunk0Data UTF-8 JSON text
        private static byte[] MakeGlb(string jsonFragment)
        {
            var jsonBytes  = Encoding.UTF8.GetBytes(jsonFragment);
            var totalLen   = (uint)(12 + 8 + jsonBytes.Length);
            using var ms   = new MemoryStream();
            using var bw   = new BinaryWriter(ms, Encoding.UTF8, leaveOpen: true);
            bw.Write(0x46546C67u); // magic "glTF"
            bw.Write(2u);          // version
            bw.Write(totalLen);    // total length
            bw.Write((uint)jsonBytes.Length); // chunk0 length
            bw.Write(0x4E4F534Au); // chunk0 type "JSON"
            bw.Write(jsonBytes);
            bw.Flush();
            return ms.ToArray();
        }

        private static string WriteTempGlb(string jsonFragment)
        {
            var path  = Path.Combine(Path.GetTempPath(), $"spec_ver_test_{Guid.NewGuid()}.glb");
            File.WriteAllBytes(path, MakeGlb(jsonFragment));
            return path;
        }

        [Test]
        public void DetectorReturnsV0ForVrm0xExtension()
        {
            var json = "{\"asset\":{\"version\":\"2.0\"},\"extensionsUsed\":[\"VRM\"]}";
            var path = WriteTempGlb(json);
            try
            {
                var result = SpecVersionDetector.DetectFromGlbPath(path);
                Assert.AreEqual("0.x", result,
                    "GLB with extensionsUsed=[\"VRM\"] should be detected as 0.x");
            }
            finally { File.Delete(path); }
        }

        [Test]
        public void DetectorReturnsV1ForVrm10Extension()
        {
            var json = "{\"asset\":{\"version\":\"2.0\"},\"extensionsUsed\":[\"VRMC_vrm\"]}";
            var path = WriteTempGlb(json);
            try
            {
                var result = SpecVersionDetector.DetectFromGlbPath(path);
                Assert.AreEqual("1.0", result,
                    "GLB with extensionsUsed=[\"VRMC_vrm\"] should be detected as 1.0");
            }
            finally { File.Delete(path); }
        }

        [Test]
        public void DetectorPrefersVrmc_vrmOverVrm()
        {
            // A well-formed 1.0 file should never have "VRM" in extensionsUsed,
            // but if it somehow does, "VRMC_vrm" wins because the detector
            // checks for it first.
            var json = "{\"extensionsUsed\":[\"VRMC_vrm\",\"VRM\"]}";
            var path = WriteTempGlb(json);
            try
            {
                var result = SpecVersionDetector.DetectFromGlbPath(path);
                Assert.AreEqual("1.0", result,
                    "VRMC_vrm takes precedence over VRM when both are present");
            }
            finally { File.Delete(path); }
        }

        [Test]
        public void DetectorReturnsNullForNonGlbFile()
        {
            var path = Path.GetTempFileName();
            try
            {
                File.WriteAllText(path, "not a glb file");
                var result = SpecVersionDetector.DetectFromGlbPath(path);
                Assert.IsNull(result,
                    "non-GLB file should yield null (unknown spec version)");
            }
            finally { File.Delete(path); }
        }

        [Test]
        public void DetectorReturnsNullForMissingFile()
        {
            var result = SpecVersionDetector.DetectFromGlbPath("/tmp/definitely_does_not_exist.glb");
            Assert.IsNull(result, "missing file should yield null without throwing");
        }

        [Test]
        public void DetectorReturnsNullForNoKnownExtension()
        {
            // GLB with neither "VRM" nor "VRMC_vrm" in extensionsUsed.
            var json = "{\"asset\":{\"version\":\"2.0\"},\"extensionsUsed\":[\"KHR_draco_mesh_compression\"]}";
            var path = WriteTempGlb(json);
            try
            {
                var result = SpecVersionDetector.DetectFromGlbPath(path);
                Assert.IsNull(result, "GLB without a VRM extension should yield null");
            }
            finally { File.Delete(path); }
        }

        // ------------------------------------------------------------------ //
        // Fixture-gated test (VRM 0.x real asset)
        // ------------------------------------------------------------------ //
        //
        // Skipped when the fixture is absent. To run locally:
        //   cargo run -p vrm-asset-generator -- emit-default --id smoke_v0 --output-dir /tmp/univrm-v0-fixture
        //   # (or use a real VRM 0.x GLB)
        //   Then set FIXTURE_VRM0X_PATH env or drop the file at the path below.

        private const string FixtureVrm0xPath = "/tmp/univrm-v0-fixture/smoke_v0.vrm";

        [Test]
        public void DetectorReturnsV0ForRealVrm0xAsset()
        {
            if (!File.Exists(FixtureVrm0xPath))
            {
                Assert.Ignore(
                    $"VRM 0.x fixture not present at {FixtureVrm0xPath}; " +
                    "generate via `cargo run -p vrm-asset-generator -- emit-default --id smoke_v0 --output-dir /tmp/univrm-v0-fixture` " +
                    "with a 0.x asset, or see Task 39 for bootstrap-level verification");
            }
            var result = SpecVersionDetector.DetectFromGlbPath(FixtureVrm0xPath);
            Assert.AreEqual("0.x", result,
                $"Real VRM 0.x asset at {FixtureVrm0xPath} should report '0.x'");
        }
    }
}
