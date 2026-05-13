// EditMode test: locks the JSON wire shape of the error envelopes the
// Rust runner expects. Per docs/operation-contract.md the codes are:
//   -32000  Unimplemented           (data.phase: "Phase 2" | "Phase 3" | "L3" | "L4")
//   -32001  LoadFailed              (data.phase tracks layer of detection)
//   -32002  RenderFailed            (data.phase tracks layer of detection)
//   -32602  invalid params          (data.feature/value/supported)

using NUnit.Framework;
using UnityEngine;

namespace Conformance.Tests
{
    public class ErrorEnvelopeTest
    {
        [Test]
        public void UnimplementedSerializesWithPhase()
        {
            var entry = new Manifest.EntryDto
            {
                test_id = "x",
                status = "error",
                error = new Manifest.ErrorDto
                {
                    code = -32000,
                    message = "Unimplemented (phase 3)",
                    data = new Manifest.ErrorDataDto { phase = "Phase 3" },
                },
            };
            var json = JsonUtility.ToJson(entry);
            StringAssert.Contains("\"code\":-32000", json);
            StringAssert.Contains("\"phase\":\"Phase 3\"", json);
        }

        [Test]
        public void InvalidParamsCarriesFeatureValueSupported()
        {
            var entry = new Manifest.EntryDto
            {
                test_id = "x",
                status = "error",
                error = new Manifest.ErrorDto
                {
                    code = -32602,
                    message = "unsupported tone_mapping: Aces",
                    data = new Manifest.ErrorDataDto
                    {
                        feature = "tone_mapping",
                        value = "Aces",
                        supported = new[] { "none" },
                    },
                },
            };
            var json = JsonUtility.ToJson(entry);
            StringAssert.Contains("\"code\":-32602", json);
            StringAssert.Contains("\"feature\":\"tone_mapping\"", json);
            StringAssert.Contains("\"value\":\"Aces\"", json);
            StringAssert.Contains("\"supported\":[\"none\"]", json);
        }

        [Test]
        public void LoadFailedAndRenderFailedShareEnvelope()
        {
            foreach (var spec in new[] {
                new { code = -32001, label = "LoadFailed" },
                new { code = -32002, label = "RenderFailed" },
            })
            {
                var entry = new Manifest.EntryDto
                {
                    test_id = "x",
                    status = "error",
                    error = new Manifest.ErrorDto
                    {
                        code = spec.code,
                        message = spec.label + ": detail",
                        data = new Manifest.ErrorDataDto { phase = "L3" },
                    },
                };
                var json = JsonUtility.ToJson(entry);
                StringAssert.Contains($"\"code\":{spec.code}", json);
                StringAssert.Contains("\"phase\":\"L3\"", json);
            }
        }
    }
}
