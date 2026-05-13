// Asserts project-level settings match the design spec on every editor
// load. Does NOT auto-correct — drift is logged as an error so the
// engineer notices and commits the correction. Auto-correcting would
// fight the committed .asset files.

using UnityEditor;
using UnityEngine;
using UnityEngine.Rendering;

namespace Conformance.Editor
{
    [InitializeOnLoad]
    public static class ProjectSetup
    {
        static ProjectSetup()
        {
            AssertColorSpace();
            AssertRenderPipeline();
        }

        private static void AssertColorSpace()
        {
            if (PlayerSettings.colorSpace != ColorSpace.Linear)
            {
                Debug.LogError(
                    $"Conformance: PlayerSettings.colorSpace is {PlayerSettings.colorSpace}; " +
                    "the conformance corpus requires Linear. Fix via Edit > Project Settings > " +
                    "Player > Other Settings > Color Space, then commit ProjectSettings.asset.");
            }
        }

        private static void AssertRenderPipeline()
        {
            if (GraphicsSettings.defaultRenderPipeline != null)
            {
                Debug.LogError(
                    $"Conformance: defaultRenderPipeline is {GraphicsSettings.defaultRenderPipeline.GetType().Name}; " +
                    "the corpus targets Built-in RP (null). Fix via Edit > Project Settings > " +
                    "Graphics, then commit GraphicsSettings.asset.");
            }
        }
    }
}
