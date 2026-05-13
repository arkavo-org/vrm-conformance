# adapter-univrm L3 — Phase 1 ops real

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Builds on:** [`2026-05-12-adapter-univrm-scaffold.md`](./2026-05-12-adapter-univrm-scaffold.md) (L1+L2). L1+L2 landed the Rust `execute-test-batch` subcommand, mock-fixture contract tests, Unity project skeleton, `launcher.sh`, EditMode round-trip test, and `Conformance.RunBatch` returning `-32000 Unimplemented` for every test_id. L3 replaces the Unimplemented loop with real per-test rendering for the five Phase 1 ops (`load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`), so the 44-variant MToon corpus produces PNGs through UniVRM. L4 (spring-bone physics for the 36 swing/settle variants) is a separate follow-up plan.

**Goal:** Render the 44 MToon test_ids end-to-end through Unity + UniVRM v0.131.0 + Built-in RP, producing PNGs and a `results.ndjson` that the Rust runner ingests into `goldens-cache/univrm/local-manifest.json`. After this plan lands, `scripts/bootstrap-goldens.sh RUN_UNIVRM=1` writes a complete UniVRM golden set for the 44 MToon variants, and `scripts/consensus-report.sh` includes UniVRM as a fourth voter for those tests.

**Architecture:** Single C# entry point (`Conformance.RunBatch`) invoked via `Unity -batchmode -executeMethod`. The entry point parses the manifest, then for each test iterates: load `.vrm` via `UniVRM10.Vrm10.LoadPathAsync` with `ImmediateCaller` for synchronous-in-batch execution, configure scene (camera + DirectionalLight + ambient + magenta clear) with `GltfToUnity` Z-mirror for coordinates, render into a per-test `RenderTexture` with MSAA, `ReadPixels` to `Texture2D`, `EncodeToPNG`, write to `output_dir/<test_id>.png`, append the result entry to `results.ndjson` with `Flush(flushToDisk: true)`. Per-test errors append `status: "error"` entries; the batch keeps running. Filesystem-as-protocol — no stdio framing inside Unity.

```
                  Rust runner                            Unity Editor (batchmode + Metal)
              ┌──────────────────┐                  ┌──────────────────────────────┐
              │ execute-test-    │  manifest.json   │ Conformance.RunBatch         │
   plans/  ──▶│ batch            │ ───────────────▶ │                              │
   *.vrm     │  - build manifest │                  │ for each test:               │
              │  - spawn adapter │                  │   Vrm10.LoadPathAsync(...)   │
              │  - ingest results│                  │   SceneSetup.Apply(test)     │
              │  - blake3 fill   │  results.ndjson  │   Capture.Render(test)       │
              │  - local manifest│ ◀─────────────── │   append result line + fsync │
              └──────────────────┘                  │   GameObject.Destroy(vrm)    │
                                                    │ EditorApplication.Exit(0)    │
                                                    └──────────────────────────────┘
```

**Tech Stack:** Unity 6000.4.6f1 (Unity 6 LTS), Built-in Render Pipeline, Linear color space project setting, UniVRM v0.131.0 via UPM git URL (`com.vrmc.vrm` + `com.vrmc.gltf` packages, `path=/Packages/...#v0.131.0`), C# 9 (Unity 6 default), .NET Standard 2.1, NUnit for EditMode tests. macOS-only for v1.0 (Metal pipeline). No changes to the Rust runner — it already emits the full per-test manifest schema and reconciles results back into the local manifest with BLAKE3 backfill (`crates/vrm-runner/src/execute_batch.rs:207-211`).

---

## Pre-flight assumptions to verify

Four load-bearing assumptions; each gets a spike task before implementation. Spike 0 is a fix-and-verify (the L1+L2 manifest pin is broken — must be corrected before Unity will even open the project). Spikes 1-3 are pure verifications. A failure on any one stops the plan for re-spec.

1. **The UPM manifest pin resolves.** The L1+L2 `Packages/manifest.json` references `com.vrmc.univrm`+`com.vrmc.vrmshaders` at `path=/Assets/VRM10#v0.131.2` and `path=/Assets/VRMShaders#v0.131.2`. **None of these resolve** — the actual UniVRM v0.131.0 (the current latest, no v0.131.x exists past .0) restructured to `Packages/VRM10` and renamed the package to `com.vrmc.vrm` with a `com.vrmc.gltf` dependency at `Packages/UniGLTF`. The L1+L2 scaffold compiled cleanly only because there was no UniVRM API call in the stub. **Spike 0 (Task 1)** corrects the manifest and verifies Unity resolves the package on a clean import.

2. **`Vrm10.LoadPathAsync` runs synchronously under `ImmediateCaller` in batch mode.** UniVRM's loader is async (`Task<Vrm10Instance>`). `-executeMethod` invokes a synchronous void entry point. Calling `.GetAwaiter().GetResult()` on a `Task` that posts continuations back to the main thread can deadlock under Unity's `SynchronizationContext`. The documented workaround is passing `awaitCaller: new ImmediateCaller()` (defined at `Packages/UniGLTF/Runtime/UniGLTF/IO/AwaitCaller/ImmediateCaller.cs`), which runs all awaits on the calling thread without posting back. **Spike 1 (Task 4)** verifies this completes for a generated `.vrm` and produces a `Vrm10Instance` with a non-null `SkinnedMeshRenderer` in its scene graph.

3. **Headless Metal render path produces non-trivial pixels.** Unity's `-batchmode` flag (without `-nographics`) keeps Metal initialized; `Camera.Render()` against a `RenderTexture` followed by `Texture2D.ReadPixels` + `EncodeToPNG` should produce a PNG with the magenta clear color and any drawn geometry. This works in practice for the `vrm-metal-kit` adapter (different stack, but same macOS Metal substrate); we still verify here because Unity's RP behavior in batch mode has its own quirks. **Spike 2 (Task 5)** writes a single magenta-clear-only PNG (no VRM, no lighting) and asserts the file is >1 KB and pixel `(512, 512)` is `(255, 0, 255, 255)`.

4. **MToon shaders compile under Built-in RP at runtime load.** UniVRM ships `BuiltInVrm10MToonMaterialImporter` for the Built-in RP path. Shaders compile lazily; a missing shader produces Unity's pink-magenta error material on the affected mesh. **Spike 3 (Task 6)** loads `mtoon_default.vrm`, samples 5 pixels along the centerline, asserts none of them are the pink-magenta error tint (RGB ≈ `(255, 0, 255)` matches our clear color — to disambiguate, the spike checks `mtoon_default` whose shaded gray sphere should give RGB near `(135, 135, 135)`, not magenta).

If any of these spikes fail, **stop and surface the failure** before continuing — the rest of the plan assumes all four hold.

---

## File Structure

```
adapters/univrm/
├── README.md                                                       MODIFY (version refs, L3 status table row)
├── launcher.sh                                                     (unchanged)
└── UniVRMConformance/
    ├── Packages/manifest.json                                      MODIFY (UPM URLs + tag fix)
    ├── ProjectSettings/ProjectVersion.txt                          (unchanged)
    ├── ProjectSettings/ProjectSettings.asset                       CREATE (Linear color space pin)
    ├── ProjectSettings/QualitySettings.asset                       CREATE (MSAA defaults; per-RT override at runtime)
    ├── ProjectSettings/GraphicsSettings.asset                      CREATE (Built-in RP pin)
    └── Assets/Conformance/
        ├── Runtime/
        │   ├── Conformance.asmdef                                  MODIFY (add VRM + UniGLTF references)
        │   ├── Conformance.cs                                      MODIFY (RunBatch becomes thin dispatcher)
        │   ├── Manifest.cs                                         CREATE (extracted + extended DTOs)
        │   ├── SceneSetup.cs                                       CREATE (coord-conv, camera, lighting, post-fx)
        │   └── Capture.cs                                          CREATE (RenderTexture → PNG, color-space)
        ├── Editor/
        │   ├── Conformance.Editor.asmdef                           CREATE (Editor-only asmdef)
        │   └── ProjectSetup.cs                                     CREATE (InitializeOnLoad — assert Linear)
        └── Tests/EditMode/
            ├── Conformance.Tests.EditMode.asmdef                   (unchanged)
            ├── ManifestRoundtripTest.cs                            MODIFY (cover extended schema)
            ├── CoordinateConversionTest.cs                         CREATE
            ├── CaptureColorSpaceTest.cs                            CREATE
            └── ErrorEnvelopeTest.cs                                CREATE

scripts/
├── smoke-univrm.sh                                                 CREATE
└── bootstrap-goldens.sh                                            MODIFY (RUN_UNIVRM=1 env flag)

# Top-level docs
README.md                                                           MODIFY (univrm row in adapter status)
CLAUDE.md                                                           MODIFY (adapter-status bullet)
docs/findings.md                                                    MODIFY (new run entry after first corpus render)
```

**Boundaries:**
- `Conformance.cs` owns the batch entry point and the per-test loop. No per-test rendering code lives here — it delegates to `SceneSetup` and `Capture`.
- `Manifest.cs` owns all `[Serializable]` DTOs. Single file lets one engineer see the full wire schema at a glance.
- `SceneSetup.cs` owns coordinate conversion, camera, lighting, magenta clear, post-fx error handling. Pure setup; no rendering.
- `Capture.cs` owns `RenderTexture` lifecycle, `ReadPixels`, color-space handling, and `EncodeToPNG`.
- `ProjectSetup.cs` (Editor-only) asserts the project's color space and render pipeline match the design spec on each editor load. If they don't, it logs an error — does **not** auto-fix, to keep the project under VCS control.

---

## Task 1 — Fix UPM manifest pin (Spike 0)

The L1+L2 manifest points at non-existent paths and tags. Correcting it is the absolute precondition for everything else — without this, Unity refuses to open the project.

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Packages/manifest.json`
- Modify: `adapters/univrm/README.md` (one line, version reference)
- Modify: `docs/superpowers/plans/2026-05-12-adapter-univrm-scaffold.md` (no — leave the historical plan unchanged; correct text in `adapters/univrm/README.md` only)

- [ ] **Step 1.1: Verify upstream tag + path**

Run:
```bash
gh release list --repo vrm-c/UniVRM --limit 5
gh api 'repos/vrm-c/UniVRM/contents/Packages/VRM10/package.json?ref=v0.131.0' --jq '.path'
gh api 'repos/vrm-c/UniVRM/contents/Packages/UniGLTF/package.json?ref=v0.131.0' --jq '.path'
```

Expected:
- Latest tag: `v0.131.0`
- VRM10 package.json path: `Packages/VRM10/package.json`
- UniGLTF package.json path: `Packages/UniGLTF/package.json`

If a later v0.131.x has shipped by execution time, use that. Tag must exist; package names in step 1.2 are stable across v0.131.x.

- [ ] **Step 1.2: Rewrite `Packages/manifest.json`**

Overwrite `adapters/univrm/UniVRMConformance/Packages/manifest.json` with:

```json
{
  "dependencies": {
    "com.vrmc.gltf": "https://github.com/vrm-c/UniVRM.git?path=/Packages/UniGLTF#v0.131.0",
    "com.vrmc.vrm": "https://github.com/vrm-c/UniVRM.git?path=/Packages/VRM10#v0.131.0",
    "com.unity.timeline": "1.7.6",
    "com.unity.test-framework": "1.4.5",
    "com.unity.ide.rider": "3.0.31",
    "com.unity.ide.visualstudio": "2.0.22",
    "com.unity.modules.imageconversion": "1.0.0",
    "com.unity.modules.jsonserialize": "1.0.0",
    "com.unity.modules.physics": "1.0.0",
    "com.unity.modules.unitywebrequest": "1.0.0"
  },
  "scopedRegistries": []
}
```

Key changes from L1+L2:
- `com.vrmc.univrm` → `com.vrmc.vrm` (real package name per VRM10/package.json)
- `com.vrmc.vrmshaders` removed (no longer a separate UPM package in v0.131.x)
- `com.vrmc.gltf` added (declared dependency of `com.vrmc.vrm` per its package.json `dependencies` field)
- Path `Assets/VRM10` → `Packages/VRM10`
- Path `Assets/VRMShaders` → `Packages/UniGLTF`
- Tag `v0.131.2` → `v0.131.0`
- `com.unity.timeline` added (transitive dep of `com.vrmc.vrm`; UPM does not auto-fetch transitive git deps so it must be top-level)

- [ ] **Step 1.3: Update README version references**

Open `adapters/univrm/README.md` and replace `v0.131.2` with `v0.131.0` in every occurrence. There are ~3 references; use `replace_all` semantics.

- [ ] **Step 1.4: Open Unity to verify packages resolve**

Run (requires Unity 6000.4.6f1 installed):
```bash
"$UNITY_BIN" \
  -batchmode \
  -projectPath adapters/univrm/UniVRMConformance \
  -quit \
  -logFile - \
  -nographics \
  2>&1 | tee /tmp/univrm-resolve.log
```

Expected: exit code 0, log contains `[Package Manager] Done resolving packages` or similar, no `Failed to resolve dependencies`. First open takes ~5 minutes (UPM clones UniVRM); subsequent opens are fast (cached in `Library/PackageCache/`).

If the log shows `Failed to resolve` for `com.vrmc.gltf`, retry without `-nographics` — UPM has known issues resolving git deps in pure-headless mode for some Unity versions.

- [ ] **Step 1.5: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Packages/manifest.json adapters/univrm/README.md
git commit -m "$(cat <<'EOF'
fix(adapters/univrm): correct UPM pin to v0.131.0 + path/Packages restructure

L1+L2 pinned com.vrmc.univrm at path=/Assets/VRM10#v0.131.2; none of these resolve
in v0.131.0:
  - package renamed to com.vrmc.vrm (per Packages/VRM10/package.json)
  - VRMShaders consolidated into com.vrmc.gltf at /Packages/UniGLTF
  - v0.131.2 tag never existed; v0.131.0 is current latest

Adds com.unity.timeline as a top-level dep (transitive of com.vrmc.vrm, but UPM
does not auto-fetch transitive git deps).

Verified resolution against Unity 6000.4.6f1 + UniVRM v0.131.0.
EOF
)"
```

---

## Task 2 — Project settings: Linear color space + Built-in RP

Per design spec (`docs/superpowers/specs/2026-05-12-adapter-univrm-design.md:219`): `PlayerSettings.colorSpace = ColorSpace.Linear`. With Linear set, Built-in RP shades in linear space; the swap-chain texture's sRGB flag controls output OETF.

These settings live in `ProjectSettings/ProjectSettings.asset` (color space) and `ProjectSettings/GraphicsSettings.asset` (render pipeline = null → Built-in RP default). Unity auto-creates them on first open with default values (Gamma color space). We override via an Editor script that runs on load and via an explicit ProjectSettings.asset committed to the repo.

**Files:**
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Editor/Conformance.Editor.asmdef`
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Editor/ProjectSetup.cs`
- Create: `adapters/univrm/UniVRMConformance/ProjectSettings/ProjectSettings.asset` (committed; will be created by Unity on first open then committed)
- Create: `adapters/univrm/UniVRMConformance/ProjectSettings/GraphicsSettings.asset` (same)
- Create: `adapters/univrm/UniVRMConformance/ProjectSettings/QualitySettings.asset` (same; MSAA defaults)

- [ ] **Step 2.1: Create the Editor asmdef**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Editor/Conformance.Editor.asmdef`:

```json
{
  "name": "Conformance.Editor",
  "rootNamespace": "Conformance.Editor",
  "references": [ "Conformance" ],
  "includePlatforms": [ "Editor" ],
  "excludePlatforms": [],
  "allowUnsafeCode": false,
  "overrideReferences": false,
  "precompiledReferences": [],
  "autoReferenced": true,
  "defineConstraints": [],
  "versionDefines": [],
  "noEngineReferences": false
}
```

- [ ] **Step 2.2: Create `ProjectSetup.cs` (warn on drift, do not auto-fix)**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Editor/ProjectSetup.cs`:

```csharp
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
```

- [ ] **Step 2.3: Open Unity, set color space, commit asset files**

Open the project in Unity (`open -a Unity ...UniVRMConformance` or via Hub). Once it loads, the `ProjectSetup` script will log errors about Gamma color space.

In the editor: **Edit → Project Settings → Player → Other Settings → Color Space → Linear**. Unity asks if it should re-bake; click Continue. (Re-bake is moot — no lightmaps in this project.)

Verify the error stops on next editor reload (`File → Close Project` then re-open).

Exit Unity. The following ProjectSettings/*.asset files should now exist (Unity wrote them):
```bash
ls adapters/univrm/UniVRMConformance/ProjectSettings/
# Expected:
# AudioManager.asset  EditorBuildSettings.asset  GraphicsSettings.asset
# InputManager.asset  PackageManager.asset       PhysicsManager.asset
# Physics2DSettings.asset  ProjectSettings.asset  ProjectVersion.txt
# QualitySettings.asset  TagManager.asset        TimeManager.asset
# URLPicker.asset      VFXManager.asset          XRSettings.asset
```

- [ ] **Step 2.4: Verify Linear was committed**

```bash
grep -A 1 "m_ActiveColorSpace" adapters/univrm/UniVRMConformance/ProjectSettings/ProjectSettings.asset
```

Expected: `m_ActiveColorSpace: 1` (1 = Linear; 0 = Gamma).

- [ ] **Step 2.5: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Editor \
        adapters/univrm/UniVRMConformance/ProjectSettings
git commit -m "$(cat <<'EOF'
feat(adapters/univrm): project settings — Linear color space + Built-in RP

Adds ProjectSetup.cs (Editor InitializeOnLoad) that asserts on each editor
open: PlayerSettings.colorSpace == Linear, GraphicsSettings.defaultRenderPipeline == null
(Built-in RP). Drift is logged as error; the engineer fixes via Project Settings
UI and commits the .asset.

Commits the full ProjectSettings/ directory so VCS owns the project config
end-to-end. No more "Unity will create them on first open" footgun.
EOF
)"
```

---

## Task 3 — Extend Manifest DTOs

The L1+L2 stub had minimal DTOs (test_id, vrm_path, spec_section). The Rust runner already emits the full schema (camera, lighting, post_processing, output, optional physics, optional animation). L3 needs DTOs covering everything Unity reads. Extract them into a dedicated file.

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs` (remove inline DTOs)
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Manifest.cs`
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/ManifestRoundtripTest.cs` (cover extended schema)

- [ ] **Step 3.1: Write the extended round-trip test FIRST**

Overwrite `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/ManifestRoundtripTest.cs`:

```csharp
// EditMode test: locks the extended Manifest DTO contract. Asserts every
// per-test field the runner emits survives JsonUtility round-trip.

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
```

- [ ] **Step 3.2: Run the test — expect compile failure (referenced types don't exist yet)**

```bash
"$UNITY_BIN" -batchmode -projectPath adapters/univrm/UniVRMConformance \
  -runTests -testPlatform EditMode \
  -testResults /tmp/results.xml \
  -logFile - 2>&1 | grep -E "(FAIL|error CS|missing)" | head -20
```

Expected: compile errors `The type or namespace 'Manifest' could not be found`. This is the failing-first state.

- [ ] **Step 3.3: Create `Manifest.cs` with the full DTO graph**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Manifest.cs`:

```csharp
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
            public string renderer_version;       // unused on Unity side; reflected back unchanged
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
            public PhysicsDto physics;            // optional; null when not present
            public AnimationDto animation;        // optional; null when not present
        }

        [Serializable]
        public class CameraDto
        {
            public float[] position;              // glTF: right-handed Y-up, length 3
            public float[] target;
            public float[] up;
            public float fov_degrees;             // vertical FOV
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
            public float[] dir;                   // glTF direction-of-travel; length 3
            public float[] color;                 // length 3 or 4
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
            public string tone_mapping;           // "None" supported in v1.0; others return -32602
            public float exposure;
        }

        [Serializable]
        public class OutputDto
        {
            public int width;
            public int height;
            public string color_space;            // "Srgb" or "Linear"
            public int msaa;                      // 1, 2, 4, or 8 — applied per RT
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
            public string status;                 // "ok" or "error"
            public string output_path;            // populated for ok only
            public string actual_color_space;     // "Srgb" or "Linear" — what we actually wrote
            public float render_seconds;
            public ErrorDto error;                // populated for error only
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
            public string phase;                  // -32000 Unimplemented
            public string feature;                // -32602 invalid params
            public string value;                  // -32602 invalid params
            public string[] supported;            // -32602 invalid params
        }
    }
}
```

- [ ] **Step 3.4: Strip inline DTOs from `Conformance.cs`**

Modify `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs` — delete the inline `ManifestDto`, `TestEntryDto`, `MetaDto`, `EntryDto`, `ErrorDto`, `ErrorDataDto` class definitions (lines 119-170 in the L1+L2 version). Update the `RunBatch` body to reference the new `Manifest.*` types: `JsonUtility.FromJson<Manifest.ManifestDto>(...)`, `new Manifest.MetaDto { ... }`, `new Manifest.EntryDto { ... }`, etc.

The RunBatch body stays an Unimplemented loop at this task — real per-test execution arrives in Task 11.

- [ ] **Step 3.5: Run the test — expect pass**

```bash
"$UNITY_BIN" -batchmode -projectPath adapters/univrm/UniVRMConformance \
  -runTests -testPlatform EditMode \
  -testResults /tmp/results.xml \
  -logFile - 2>&1 | tee /tmp/test-run.log
grep -E "(Passed|Failed|Errors)" /tmp/results.xml | head -5
```

Expected: All 5 ManifestRoundtripTest cases pass.

- [ ] **Step 3.6: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance
git commit -m "$(cat <<'EOF'
refactor(adapters/univrm): extract Manifest DTOs into single Manifest.cs

Moves the JsonUtility-friendly DTOs out of Conformance.cs into Manifest.cs
and extends them to cover the full per-test schema the Rust runner emits
(camera, lighting, post_processing, output, optional physics + animation).
ManifestRoundtripTest gains 4 new cases asserting every nested field
survives JsonUtility round-trip.

Conformance.RunBatch still returns Unimplemented for every test — real
per-test rendering lands in Task 11.
EOF
)"
```

---

## Task 4 — Spike 1: synchronous VRM load via ImmediateCaller

Verifies Spike-1 assumption (see preflight): `Vrm10.LoadPathAsync(path, awaitCaller: new ImmediateCaller())` runs synchronously in batchmode and produces a usable `Vrm10Instance`.

**Files:**
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/Vrm10LoadSpike.cs` (will be deleted after spike passes; the production code in Task 11 replaces it)
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.asmdef` (add references to UniVRM)

- [ ] **Step 4.1: Add VRM references to the runtime asmdef**

Modify `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.asmdef`:

```json
{
  "name": "Conformance",
  "rootNamespace": "Conformance",
  "references": [
    "VRM10",
    "UniGLTF"
  ],
  "includePlatforms": [],
  "excludePlatforms": [],
  "allowUnsafeCode": false,
  "overrideReferences": false,
  "precompiledReferences": [],
  "autoReferenced": true,
  "defineConstraints": [],
  "versionDefines": [],
  "noEngineReferences": false
}
```

Asmdef reference names follow the names in the UPM-installed assemblies. Verify post-Spike-0 that `Library/ScriptAssemblies/VRM10.dll` and `UniGLTF.dll` exist; if Unity's `Library/PackageCache/com.vrmc.vrm@.../` uses different asmdef names, update accordingly.

- [ ] **Step 4.2: Generate a fixture VRM**

```bash
cargo run --release -p vrm-asset-generator -- emit-default \
  --id spike1_mtoon_default \
  --output-dir /tmp/univrm-spike/
ls -la /tmp/univrm-spike/spike1_mtoon_default.vrm
```

Expected: file exists, ~70 KB.

- [ ] **Step 4.3: Write a one-shot spike test**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/Vrm10LoadSpike.cs`:

```csharp
// Spike 1: verify Vrm10.LoadPathAsync + ImmediateCaller runs synchronously
// in EditMode and produces a Vrm10Instance with a SkinnedMeshRenderer.
// Spike-only — deleted after Task 4 passes.

using NUnit.Framework;
using UniGLTF;
using UnityEngine;
using UniVRM10;

namespace Conformance.Tests
{
    public class Vrm10LoadSpike
    {
        // Path generated by `vrm-asset-generator emit-default` in Step 4.2.
        private const string FixturePath = "/tmp/univrm-spike/spike1_mtoon_default.vrm";

        [Test]
        public void LoadProducesVrm10InstanceWithSkinnedMesh()
        {
            if (!System.IO.File.Exists(FixturePath))
            {
                Assert.Ignore($"fixture not present at {FixturePath}; run Step 4.2 first");
            }

            var task = Vrm10.LoadPathAsync(
                FixturePath,
                canLoadVrm0X: false,
                showMeshes: true,
                awaitCaller: new ImmediateCaller(),
                ct: System.Threading.CancellationToken.None);

            // ImmediateCaller runs awaits synchronously; the task should
            // be completed by the time LoadPathAsync returns its Task.
            Assert.IsTrue(task.IsCompleted, "task should be synchronously complete under ImmediateCaller");
            Assert.AreEqual(System.Threading.Tasks.TaskStatus.RanToCompletion, task.Status);

            var instance = task.Result;
            Assert.IsNotNull(instance, "Vrm10Instance must not be null");

            var smr = instance.GetComponentInChildren<SkinnedMeshRenderer>();
            Assert.IsNotNull(smr, "loaded VRM must have at least one SkinnedMeshRenderer");
            Assert.IsNotNull(smr.sharedMesh, "SkinnedMeshRenderer must have a mesh");
            Assert.Greater(smr.sharedMesh.vertexCount, 0);

            Object.DestroyImmediate(instance.gameObject);
        }
    }
}
```

- [ ] **Step 4.4: Run the spike**

```bash
"$UNITY_BIN" -batchmode -projectPath adapters/univrm/UniVRMConformance \
  -runTests -testPlatform EditMode \
  -testFilter "Conformance.Tests.Vrm10LoadSpike" \
  -testResults /tmp/spike1-results.xml \
  -logFile - 2>&1 | tee /tmp/spike1.log
grep -E "Test (Passed|Failed)" /tmp/spike1.log | head -5
```

Expected: `LoadProducesVrm10InstanceWithSkinnedMesh: Passed`. If it ignores (fixture missing) re-run Step 4.2. If it fails with deadlock symptoms (test runner hangs), the synchronous-load pattern needs revisiting — see Task 11 fallback note.

- [ ] **Step 4.5: Commit the spike**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance
git commit -m "$(cat <<'EOF'
test(adapters/univrm): Spike 1 — Vrm10.LoadPathAsync synchronous via ImmediateCaller

Verifies that passing `awaitCaller: new ImmediateCaller()` causes UniVRM's
async loader to complete synchronously within the calling thread — required
for our `-executeMethod`-invoked RunBatch entry point.

Fixture: emit-default-generated VRM at /tmp/univrm-spike/.
EOF
)"
```

The spike test stays in tree until Task 11 lands — at that point the per-test render loop subsumes its purpose and Task 11 deletes the spike file.

---

## Task 5 — Spike 2: magenta-clear PNG produces non-trivial output

Verifies Spike-2 assumption: `Camera.Render()` against a `RenderTexture` in `-batchmode` (no `-nographics`) produces actual pixels.

**Files:**
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/MagentaClearSpike.cs`

- [ ] **Step 5.1: Write the spike**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/MagentaClearSpike.cs`:

```csharp
// Spike 2: verify Camera.Render to RenderTexture + ReadPixels +
// EncodeToPNG produces pixels matching the magenta clear color.
// Spike-only; the production code in Capture.cs (Task 9) replaces it.

using System.IO;
using NUnit.Framework;
using UnityEngine;

namespace Conformance.Tests
{
    public class MagentaClearSpike
    {
        [Test]
        public void MagentaClearProducesMagentaPixels()
        {
            const int W = 256, H = 256;
            var rt = new RenderTexture(W, H, 24, RenderTextureFormat.ARGB32);
            rt.antiAliasing = 1;
            rt.Create();

            var cameraGo = new GameObject("SpikeCamera");
            try
            {
                var cam = cameraGo.AddComponent<Camera>();
                cam.clearFlags = CameraClearFlags.SolidColor;
                cam.backgroundColor = new Color(1f, 0f, 1f, 1f);
                cam.targetTexture = rt;
                cam.Render();

                var prev = RenderTexture.active;
                RenderTexture.active = rt;
                var tex = new Texture2D(W, H, TextureFormat.RGBA32, mipChain: false, linear: false);
                tex.ReadPixels(new Rect(0, 0, W, H), 0, 0);
                tex.Apply();
                RenderTexture.active = prev;

                var center = tex.GetPixel(W / 2, H / 2);
                Assert.AreEqual(1f, center.r, 1e-2, "red channel should be ~1.0");
                Assert.AreEqual(0f, center.g, 1e-2, "green channel should be ~0.0");
                Assert.AreEqual(1f, center.b, 1e-2, "blue channel should be ~1.0");

                var png = tex.EncodeToPNG();
                var pngPath = "/tmp/univrm-spike/magenta.png";
                Directory.CreateDirectory(Path.GetDirectoryName(pngPath)!);
                File.WriteAllBytes(pngPath, png);
                Assert.Greater(new FileInfo(pngPath).Length, 200, "PNG should be at least a few hundred bytes");

                Object.DestroyImmediate(tex);
            }
            finally
            {
                Object.DestroyImmediate(cameraGo);
                rt.Release();
                Object.DestroyImmediate(rt);
            }
        }
    }
}
```

- [ ] **Step 5.2: Run the spike**

```bash
"$UNITY_BIN" -batchmode -projectPath adapters/univrm/UniVRMConformance \
  -runTests -testPlatform EditMode \
  -testFilter "Conformance.Tests.MagentaClearSpike" \
  -testResults /tmp/spike2-results.xml \
  -logFile - 2>&1 | tee /tmp/spike2.log
ls -la /tmp/univrm-spike/magenta.png
file /tmp/univrm-spike/magenta.png
```

Expected: test passes, PNG exists, `file` reports `PNG image data, 256 x 256, 8-bit/color RGBA, non-interlaced`.

- [ ] **Step 5.3: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/MagentaClearSpike.cs
git commit -m "test(adapters/univrm): Spike 2 — magenta-clear RT round-trip to PNG"
```

---

## Task 6 — Spike 3: MToon shader sanity (no pink-magenta error material)

Verifies Spike-3 assumption: MToon shaders compile under Built-in RP and `mtoon_default` renders as a shaded gray sphere, not Unity's pink-magenta error tint.

**Files:**
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/MToonShaderSpike.cs`

- [ ] **Step 6.1: Write the spike**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/MToonShaderSpike.cs`:

```csharp
// Spike 3: load mtoon_default.vrm, render with the standard test-plan
// camera + lighting, assert centerline pixels are shaded-gray (NOT
// Unity's pink-magenta shader-error tint). Spike-only.

using System.IO;
using NUnit.Framework;
using UniGLTF;
using UnityEngine;
using UniVRM10;

namespace Conformance.Tests
{
    public class MToonShaderSpike
    {
        private const string FixturePath = "/tmp/univrm-spike/spike1_mtoon_default.vrm";

        [Test]
        public void MtoonDefaultProducesGrayInteriorNotPinkError()
        {
            if (!File.Exists(FixturePath)) Assert.Ignore($"missing fixture {FixturePath}");

            // Load
            var task = Vrm10.LoadPathAsync(FixturePath, canLoadVrm0X: false,
                awaitCaller: new ImmediateCaller());
            var vrm = task.Result;

            // Scene
            var lightGo = new GameObject("Light");
            var light = lightGo.AddComponent<Light>();
            light.type = LightType.Directional;
            light.color = Color.white;
            light.intensity = 1f;
            // glTF dir [-0.3,-0.6,-0.7] → Unity Z-mirrored [-0.3,-0.6,+0.7]; rotation: from forward = (0,0,1) to dir.
            lightGo.transform.forward = new Vector3(-0.3f, -0.6f, 0.7f).normalized;

            RenderSettings.ambientMode = UnityEngine.Rendering.AmbientMode.Flat;
            RenderSettings.ambientLight = new Color(0.5f, 0.5f, 0.5f) * 0.3f;

            const int W = 1024, H = 1024;
            var rt = new RenderTexture(W, H, 24, RenderTextureFormat.ARGB32);
            rt.antiAliasing = 4;
            rt.Create();

            var cameraGo = new GameObject("Cam");
            var cam = cameraGo.AddComponent<Camera>();
            cam.clearFlags = CameraClearFlags.SolidColor;
            cam.backgroundColor = new Color(1f, 0f, 1f, 1f);
            cam.fieldOfView = 30f;
            cam.transform.position = new Vector3(0f, 1.4f, -1.5f);    // glTF (0,1.4,1.5) → Unity Z-mirror
            cam.transform.LookAt(new Vector3(0f, 1.4f, 0f), Vector3.up);
            cam.targetTexture = rt;
            cam.Render();

            // Sample
            var prev = RenderTexture.active;
            RenderTexture.active = rt;
            var tex = new Texture2D(W, H, TextureFormat.RGBA32, false, false);
            tex.ReadPixels(new Rect(0, 0, W, H), 0, 0);
            tex.Apply();
            RenderTexture.active = prev;

            // Centerline samples — the spec'd asset places a sphere head-mounted at
            // (0, 1.4, 0) with radius 0.3, so y ∈ [358, 665] @ y=512 is the centerline.
            int hit = 0;
            for (int y = 360; y <= 660; y += 75)
            {
                var p = tex.GetPixel(W / 2, y);
                // Reject pink-magenta error (R=1, G=0, B=1) and reject background magenta same color.
                // The legitimate shaded gray is around (0.2, 0.2, 0.2) for shadeColor=0.5 in linear-encoded sRGB.
                bool isMagenta = p.r > 0.95f && p.g < 0.05f && p.b > 0.95f;
                if (!isMagenta) hit++;
            }
            Assert.Greater(hit, 0, "at least one centerline sample must be non-magenta (sphere should be visible and shaded)");

            // Cleanup
            Object.DestroyImmediate(tex);
            Object.DestroyImmediate(cameraGo);
            Object.DestroyImmediate(lightGo);
            Object.DestroyImmediate(vrm.gameObject);
            rt.Release();
            Object.DestroyImmediate(rt);
        }
    }
}
```

- [ ] **Step 6.2: Run the spike**

```bash
"$UNITY_BIN" -batchmode -projectPath adapters/univrm/UniVRMConformance \
  -runTests -testPlatform EditMode \
  -testFilter "Conformance.Tests.MToonShaderSpike" \
  -testResults /tmp/spike3-results.xml \
  -logFile - 2>&1 | tee /tmp/spike3.log
```

Expected: passes. If it fails because *every* centerline sample is magenta-error, the MToon shader didn't compile — check the Unity log for `Shader error` messages. The pink-error case is fixable but signals MToon shader path needs investigation before continuing.

- [ ] **Step 6.3: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/MToonShaderSpike.cs
git commit -m "test(adapters/univrm): Spike 3 — MToon shader sanity on mtoon_default"
```

---

## Task 7 — Coordinate conversion + unit test

Per design spec: `GltfToUnity(v) = new Vector3(v[0], v[1], -v[2])`. Applied to camera position/target/up, directional dir, and animation translation_*. Unit-tested in EditMode without any scene.

**Files:**
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/SceneSetup.cs` (initial: only `GltfToUnity` static method)
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/CoordinateConversionTest.cs`

- [ ] **Step 7.1: Write the test FIRST**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/CoordinateConversionTest.cs`:

```csharp
using NUnit.Framework;
using UnityEngine;

namespace Conformance.Tests
{
    public class CoordinateConversionTest
    {
        [Test]
        public void CameraPositionZIsMirrored()
        {
            var unityPos = SceneSetup.GltfToUnity(new[] { 0f, 1.4f, 1.5f });
            Assert.AreEqual(0f, unityPos.x, 1e-6);
            Assert.AreEqual(1.4f, unityPos.y, 1e-6);
            Assert.AreEqual(-1.5f, unityPos.z, 1e-6);
        }

        [Test]
        public void DirectionalDirZIsMirrored()
        {
            var unityDir = SceneSetup.GltfToUnity(new[] { -0.3f, -0.6f, -0.7f });
            Assert.AreEqual(-0.3f, unityDir.x, 1e-6);
            Assert.AreEqual(-0.6f, unityDir.y, 1e-6);
            Assert.AreEqual(0.7f, unityDir.z, 1e-6);
        }

        [Test]
        public void UpVectorPreservedForYUp()
        {
            var unityUp = SceneSetup.GltfToUnity(new[] { 0f, 1f, 0f });
            Assert.AreEqual(0f, unityUp.x, 1e-6);
            Assert.AreEqual(1f, unityUp.y, 1e-6);
            Assert.AreEqual(0f, unityUp.z, 1e-6);
        }

        [Test]
        public void OriginPreserved()
        {
            var origin = SceneSetup.GltfToUnity(new[] { 0f, 0f, 0f });
            Assert.AreEqual(Vector3.zero, origin);
        }

        [Test]
        public void NonUnitLengthsArePreserved()
        {
            var unityVec = SceneSetup.GltfToUnity(new[] { 2.5f, -3.7f, 4.2f });
            Assert.AreEqual(2.5f, unityVec.x, 1e-6);
            Assert.AreEqual(-3.7f, unityVec.y, 1e-6);
            Assert.AreEqual(-4.2f, unityVec.z, 1e-6);
        }
    }
}
```

- [ ] **Step 7.2: Run test → expect compile failure (`SceneSetup` undefined)**

```bash
"$UNITY_BIN" -batchmode -projectPath adapters/univrm/UniVRMConformance \
  -runTests -testPlatform EditMode \
  -testFilter "Conformance.Tests.CoordinateConversionTest" \
  -logFile - 2>&1 | grep -E "(error|FAIL)" | head -5
```

Expected: `error CS0103: The name 'SceneSetup' does not exist in the current context`.

- [ ] **Step 7.3: Create initial `SceneSetup.cs` with just the conversion**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/SceneSetup.cs`:

```csharp
// Scene configuration for the conformance suite. Owns:
//   - coordinate conversion (glTF right-handed Y-up → Unity left-handed Y-up)
//   - per-test camera setup
//   - per-test directional light + ambient
//   - per-test post-processing handling (None passes through; others error)
//   - magenta sentinel clear color (255, 0, 255)
//
// Camera/light GameObjects are owned by callers; this class only sets
// values on already-spawned components.

using UnityEngine;
using UnityEngine.Rendering;

namespace Conformance
{
    public static class SceneSetup
    {
        public static Vector3 GltfToUnity(float[] v)
        {
            return new Vector3(v[0], v[1], -v[2]);
        }
    }
}
```

- [ ] **Step 7.4: Run test → expect pass (5 cases)**

```bash
"$UNITY_BIN" -batchmode -projectPath adapters/univrm/UniVRMConformance \
  -runTests -testPlatform EditMode \
  -testFilter "Conformance.Tests.CoordinateConversionTest" \
  -testResults /tmp/coord-results.xml \
  -logFile - 2>&1 | tee /tmp/coord.log
grep -E "Test (Passed|Failed)" /tmp/coord.log | head -10
```

Expected: 5 passed.

- [ ] **Step 7.5: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance
git commit -m "feat(adapters/univrm): SceneSetup.GltfToUnity + 5 coord-conversion tests"
```

---

## Task 8 — Camera setup

Adds `SceneSetup.ConfigureCamera(Camera, CameraDto, OutputDto)` — applies position, target via `LookAt`, FOV, magenta clear, `RenderTexture` assignment.

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/SceneSetup.cs`
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/CameraSetupTest.cs`

- [ ] **Step 8.1: Write the test**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/CameraSetupTest.cs`:

```csharp
using NUnit.Framework;
using UnityEngine;

namespace Conformance.Tests
{
    public class CameraSetupTest
    {
        [Test]
        public void ConfigureCameraSetsAllParams()
        {
            var go = new GameObject("TestCam");
            try
            {
                var cam = go.AddComponent<Camera>();
                var cameraDto = new Manifest.CameraDto
                {
                    position = new[] { 0f, 1.4f, 1.5f },
                    target   = new[] { 0f, 1.4f, 0f },
                    up       = new[] { 0f, 1f, 0f },
                    fov_degrees = 30f,
                };

                SceneSetup.ConfigureCamera(cam, cameraDto);

                Assert.AreEqual(new Vector3(0f, 1.4f, -1.5f), cam.transform.position);
                Assert.AreEqual(30f, cam.fieldOfView, 1e-6);
                Assert.AreEqual(CameraClearFlags.SolidColor, cam.clearFlags);
                Assert.AreEqual(new Color(1f, 0f, 1f, 1f), cam.backgroundColor);

                // After LookAt, forward should point from cam pos toward target.
                var expectedForward = (new Vector3(0f, 1.4f, 0f) - new Vector3(0f, 1.4f, -1.5f)).normalized;
                Assert.AreEqual(expectedForward.x, cam.transform.forward.x, 1e-4);
                Assert.AreEqual(expectedForward.y, cam.transform.forward.y, 1e-4);
                Assert.AreEqual(expectedForward.z, cam.transform.forward.z, 1e-4);
            }
            finally
            {
                Object.DestroyImmediate(go);
            }
        }
    }
}
```

- [ ] **Step 8.2: Run test → expect compile failure**

Same command pattern as Step 7.2; expected: `error CS0117: 'SceneSetup' does not contain a definition for 'ConfigureCamera'`.

- [ ] **Step 8.3: Add `ConfigureCamera`**

Add to `SceneSetup.cs`:

```csharp
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
```

Note: `RenderTexture` assignment is **not** here; `Capture.Render` (Task 10) owns RT lifecycle.

- [ ] **Step 8.4: Run test → expect pass**

- [ ] **Step 8.5: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance
git commit -m "feat(adapters/univrm): SceneSetup.ConfigureCamera + test"
```

---

## Task 9 — Lighting + post-processing setup

`SceneSetup.ConfigureLighting(Light, LightingDto)` + `SceneSetup.AssertPostProcessingSupported(PostProcessingDto)` (throws on unsupported tone mapping).

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/SceneSetup.cs`
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/LightingSetupTest.cs`
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/PostProcessingTest.cs`

- [ ] **Step 9.1: Write the lighting test**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/LightingSetupTest.cs`:

```csharp
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;

namespace Conformance.Tests
{
    public class LightingSetupTest
    {
        [Test]
        public void ConfigureLightingSetsDirectionalAndAmbient()
        {
            var go = new GameObject("Light");
            try
            {
                var light = go.AddComponent<Light>();
                var lightingDto = new Manifest.LightingDto
                {
                    directional = new Manifest.DirectionalDto
                    {
                        dir = new[] { -0.3f, -0.6f, -0.7f },
                        color = new[] { 1f, 1f, 1f },
                        intensity = 1f,
                    },
                    ambient = new Manifest.AmbientDto
                    {
                        color = new[] { 0.5f, 0.5f, 0.5f },
                        intensity = 0.3f,
                    },
                    cast_shadows = false,
                    receive_shadows = false,
                };

                SceneSetup.ConfigureLighting(light, lightingDto);

                Assert.AreEqual(LightType.Directional, light.type);
                Assert.AreEqual(1f, light.intensity, 1e-6);
                Assert.AreEqual(Color.white, light.color);
                Assert.AreEqual(LightShadows.None, light.shadows);

                // Forward should equal the Z-mirrored, normalized direction-of-travel.
                var expected = new Vector3(-0.3f, -0.6f, 0.7f).normalized;
                Assert.AreEqual(expected.x, light.transform.forward.x, 1e-4);
                Assert.AreEqual(expected.y, light.transform.forward.y, 1e-4);
                Assert.AreEqual(expected.z, light.transform.forward.z, 1e-4);

                // Ambient: flat with color × intensity.
                Assert.AreEqual(AmbientMode.Flat, RenderSettings.ambientMode);
                Assert.AreEqual(new Color(0.15f, 0.15f, 0.15f), RenderSettings.ambientLight);
            }
            finally
            {
                Object.DestroyImmediate(go);
            }
        }
    }
}
```

- [ ] **Step 9.2: Run → expect compile failure → add `ConfigureLighting` → run → pass**

Add to `SceneSetup.cs`:

```csharp
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
```

- [ ] **Step 9.3: Write the post-processing test**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/PostProcessingTest.cs`:

```csharp
using NUnit.Framework;

namespace Conformance.Tests
{
    public class PostProcessingTest
    {
        [Test]
        public void NoneIsSupported()
        {
            // Should not throw.
            SceneSetup.AssertPostProcessingSupported(new Manifest.PostProcessingDto
            {
                tone_mapping = "None",
                exposure = 1f,
            });
        }

        [Test]
        public void AcesIsRejected()
        {
            var ex = Assert.Throws<SceneSetup.UnsupportedFeatureException>(() =>
                SceneSetup.AssertPostProcessingSupported(new Manifest.PostProcessingDto
                {
                    tone_mapping = "Aces",
                    exposure = 1f,
                }));
            Assert.AreEqual("tone_mapping", ex.Feature);
            Assert.AreEqual("Aces", ex.Value);
            CollectionAssert.AreEqual(new[] { "None" }, ex.Supported);
        }
    }
}
```

- [ ] **Step 9.4: Run → fail → add types → pass**

Add to `SceneSetup.cs`:

```csharp
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
            if (p.tone_mapping != "None")
            {
                throw new UnsupportedFeatureException(
                    feature: "tone_mapping",
                    value: p.tone_mapping,
                    supported: new[] { "None" });
            }
        }
```

- [ ] **Step 9.5: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance
git commit -m "feat(adapters/univrm): SceneSetup lighting + post-processing handling"
```

---

## Task 10 — Capture: RenderTexture → PNG with color-space handling

Owns `RenderTexture` allocation/release, MSAA via `RT.antiAliasing`, color-space via `RT.sRGB`, `ReadPixels` lifecycle, `EncodeToPNG`, and reports `actual_color_space` in the per-test result.

**Files:**
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Capture.cs`
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/CaptureColorSpaceTest.cs`

- [ ] **Step 10.1: Write the test**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/CaptureColorSpaceTest.cs`:

```csharp
using System.IO;
using NUnit.Framework;
using UnityEngine;

namespace Conformance.Tests
{
    public class CaptureColorSpaceTest
    {
        [Test]
        public void RendersMagentaToSrgbPng()
        {
            var output = "/tmp/univrm-capture-srgb.png";
            if (File.Exists(output)) File.Delete(output);

            var cameraGo = new GameObject("Cam");
            try
            {
                var cam = cameraGo.AddComponent<Camera>();
                cam.clearFlags = CameraClearFlags.SolidColor;
                cam.backgroundColor = new Color(1f, 0f, 1f, 1f);

                var outputDto = new Manifest.OutputDto
                {
                    width = 128,
                    height = 128,
                    color_space = "Srgb",
                    msaa = 1,
                };

                var result = Capture.Render(cam, outputDto, output);

                Assert.AreEqual("Srgb", result.actualColorSpace);
                Assert.IsTrue(File.Exists(output));
                Assert.Greater(new FileInfo(output).Length, 100);
            }
            finally
            {
                Object.DestroyImmediate(cameraGo);
            }
        }

        [Test]
        public void RendersToLinearPngWhenRequested()
        {
            var output = "/tmp/univrm-capture-linear.png";
            if (File.Exists(output)) File.Delete(output);

            var cameraGo = new GameObject("Cam");
            try
            {
                var cam = cameraGo.AddComponent<Camera>();
                cam.clearFlags = CameraClearFlags.SolidColor;
                cam.backgroundColor = new Color(1f, 0f, 1f, 1f);

                var outputDto = new Manifest.OutputDto
                {
                    width = 64,
                    height = 64,
                    color_space = "Linear",
                    msaa = 1,
                };

                var result = Capture.Render(cam, outputDto, output);

                Assert.AreEqual("Linear", result.actualColorSpace);
                Assert.IsTrue(File.Exists(output));
            }
            finally
            {
                Object.DestroyImmediate(cameraGo);
            }
        }
    }
}
```

- [ ] **Step 10.2: Run → expect compile failure → create Capture.cs → run → pass**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Capture.cs`:

```csharp
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
                Directory.CreateDirectory(Path.GetDirectoryName(outputPath)!);
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
```

- [ ] **Step 10.3: Run, pass, commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance
git commit -m "feat(adapters/univrm): Capture.Render — RT lifecycle + PNG encode"
```

---

## Task 11 — Wire per-test render loop in `Conformance.RunBatch`

This is the integration point. Removes the Unimplemented loop; replaces it with per-test: load VRM, apply scene, render, write entry. Per-test failures append an error entry; the batch continues.

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs`
- Delete: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/Vrm10LoadSpike.cs` (subsumed by the production code)
- Delete: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/MagentaClearSpike.cs` (subsumed)
- Delete: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/MToonShaderSpike.cs` (subsumed by smoke test in Task 13)

- [ ] **Step 11.1: Rewrite `Conformance.cs` RunBatch body**

Overwrite the `RunBatch()` method in `adapters/univrm/UniVRMConformance/Assets/Conformance/Runtime/Conformance.cs`:

```csharp
// (keep the using directives at top of file from the existing stub,
//  add: using UniGLTF; using UniVRM10; using System.Diagnostics;)

namespace Conformance
{
    public static class Conformance
    {
        public static void RunBatch()
        {
            try
            {
                var args = ExtractAdapterArgs();
                if (args.Count < 2)
                {
                    Debug.LogError(
                        $"Conformance.RunBatch: expected 2 args (manifest, results); got {args.Count}");
                    EditorApplication.Exit(2);
                    return;
                }
                var manifestPath = args[0];
                var resultsPath = args[1];

                var manifestJson = File.ReadAllText(manifestPath);
                var manifest = JsonUtility.FromJson<Manifest.ManifestDto>(manifestJson);
                if (manifest == null || manifest.tests == null)
                {
                    Debug.LogError($"Conformance.RunBatch: failed to parse manifest at {manifestPath}");
                    EditorApplication.Exit(3);
                    return;
                }

                using var stream = new FileStream(
                    resultsPath, FileMode.Create, FileAccess.Write, FileShare.Read);

                WriteLine(stream, JsonUtility.ToJson(new Manifest.MetaDto
                {
                    _meta = true,
                    manifest_version = manifest.manifest_version,
                    renderer_name = manifest.renderer_name,
                    renderer_version = "v0.131.0",
                    unity_version = Application.unityVersion,
                    render_pipeline = "Built-in RP",
                    total_tests = manifest.tests.Length,
                }));

                foreach (var t in manifest.tests)
                {
                    var entry = RenderOne(manifest.output_dir, t);
                    WriteLine(stream, JsonUtility.ToJson(entry));
                }

                EditorApplication.Exit(0);
            }
            catch (Exception e)
            {
                Debug.LogError($"Conformance.RunBatch: unhandled exception: {e}");
                EditorApplication.Exit(1);
            }
        }

        // Render one test. Returns the EntryDto to append to the NDJSON.
        // Per-test failures produce a status="error" entry; they never
        // throw out of this method (batch must continue).
        private static Manifest.EntryDto RenderOne(string outputDir, Manifest.TestEntryDto t)
        {
            // Pre-flight reject of unsupported post-processing — fast and
            // doesn't waste a load.
            try
            {
                SceneSetup.AssertPostProcessingSupported(t.post_processing);
            }
            catch (SceneSetup.UnsupportedFeatureException ex)
            {
                return new Manifest.EntryDto
                {
                    test_id = t.test_id,
                    status = "error",
                    error = new Manifest.ErrorDto
                    {
                        code = -32602,
                        message = $"unsupported {ex.Feature}: {ex.Value}",
                        data = new Manifest.ErrorDataDto
                        {
                            feature = ex.Feature,
                            value = ex.Value,
                            supported = ex.Supported,
                        },
                    },
                };
            }

            GameObject vrmGo = null;
            GameObject lightGo = null;
            GameObject cameraGo = null;
            try
            {
                // Load.
                var loadTask = Vrm10.LoadPathAsync(
                    t.vrm_path,
                    canLoadVrm0X: false,
                    showMeshes: true,
                    awaitCaller: new ImmediateCaller(),
                    ct: System.Threading.CancellationToken.None);
                if (!loadTask.IsCompletedSuccessfully)
                {
                    return ErrorEntry(t.test_id, -32001, "LoadFailed", "L3", loadTask.Exception?.ToString());
                }
                var vrm = loadTask.Result;
                vrmGo = vrm.gameObject;

                // Camera + Light objects.
                cameraGo = new GameObject("Camera");
                var cam = cameraGo.AddComponent<Camera>();
                SceneSetup.ConfigureCamera(cam, t.camera);

                lightGo = new GameObject("Directional");
                var light = lightGo.AddComponent<Light>();
                SceneSetup.ConfigureLighting(light, t.lighting);

                // Capture.
                var outputPath = Path.Combine(outputDir, t.test_id + ".png");
                var captureResult = Capture.Render(cam, t.output, outputPath);

                return new Manifest.EntryDto
                {
                    test_id = t.test_id,
                    status = "ok",
                    output_path = captureResult.outputPath,
                    actual_color_space = captureResult.actualColorSpace,
                    render_seconds = captureResult.renderSeconds,
                };
            }
            catch (Exception e)
            {
                return ErrorEntry(t.test_id, -32002, "RenderFailed", "L3", e.ToString());
            }
            finally
            {
                if (cameraGo != null) Object.DestroyImmediate(cameraGo);
                if (lightGo != null) Object.DestroyImmediate(lightGo);
                if (vrmGo != null) Object.DestroyImmediate(vrmGo);
            }
        }

        private static Manifest.EntryDto ErrorEntry(string test_id, int code, string label, string phase, string detail)
        {
            // Truncate detail to keep results.ndjson lines reasonable.
            const int max = 1000;
            if (detail != null && detail.Length > max) detail = detail.Substring(0, max) + "…";
            return new Manifest.EntryDto
            {
                test_id = test_id,
                status = "error",
                error = new Manifest.ErrorDto
                {
                    code = code,
                    message = $"{label}: {detail ?? "no detail"}",
                    data = new Manifest.ErrorDataDto { phase = phase },
                },
            };
        }

        // ExtractAdapterArgs + WriteLine retained from L1+L2 stub — unchanged.
    }
}
```

- [ ] **Step 11.2: Delete the spike test files**

```bash
rm adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/Vrm10LoadSpike.cs \
   adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/MagentaClearSpike.cs \
   adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/MToonShaderSpike.cs
# Also delete the corresponding *.meta files if Unity generated them.
rm -f adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/*Spike.cs.meta
```

- [ ] **Step 11.3: Smoke-test via mini batch**

```bash
mkdir -p /tmp/univrm-l3-smoke
cargo run --release -p vrm-asset-generator -- emit-default \
  --id smoke_l3 \
  --output-dir /tmp/univrm-l3-smoke/

# Hand-craft a one-test manifest (the runner's normal path is exercised in Task 14):
cat > /tmp/univrm-l3-smoke/manifest.json <<EOF
{
  "manifest_version": 1,
  "output_dir": "/tmp/univrm-l3-smoke",
  "renderer_name": "univrm",
  "renderer_version": "v0.131.0",
  "tests": [{
    "test_id": "smoke_l3",
    "vrm_path": "/tmp/univrm-l3-smoke/smoke_l3.vrm",
    "spec_section": "VRMC_materials_mtoon",
    "camera": {"position":[0,1.4,1.5],"target":[0,1.4,0],"up":[0,1,0],"fov_degrees":30},
    "lighting": {"directional":{"dir":[-0.3,-0.6,-0.7],"color":[1,1,1],"intensity":1},
                 "ambient":{"color":[0.5,0.5,0.5],"intensity":0.3},
                 "cast_shadows":false,"receive_shadows":false},
    "post_processing": {"tone_mapping":"None","exposure":1.0},
    "output": {"width":1024,"height":1024,"color_space":"Srgb","msaa":4}
  }]
}
EOF

adapters/univrm/launcher.sh \
  /tmp/univrm-l3-smoke/manifest.json \
  /tmp/univrm-l3-smoke/results.ndjson
cat /tmp/univrm-l3-smoke/results.ndjson
ls -la /tmp/univrm-l3-smoke/smoke_l3.png
file /tmp/univrm-l3-smoke/smoke_l3.png
```

Expected:
- `results.ndjson` line 1 has `_meta:true` + Unity version
- `results.ndjson` line 2 has `status:"ok"`, `output_path` populated, `render_seconds` > 0
- PNG file ~1024×1024 RGBA, ~50-300 KB

- [ ] **Step 11.4: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance
git commit -m "$(cat <<'EOF'
feat(adapters/univrm): real per-test rendering in Conformance.RunBatch

Replaces the L1+L2 Unimplemented loop with the actual render path:
load via Vrm10.LoadPathAsync + ImmediateCaller, configure camera/light/
ambient via SceneSetup, capture via Capture.Render, append EntryDto.
Per-test exceptions become -32001 LoadFailed / -32002 RenderFailed
without aborting the batch.

Subsumes Spike 1/2/3 tests (deleted). Smoke-tested against a one-test
hand-crafted manifest; produces a 1024×1024 sRGB PNG and well-formed
results.ndjson.
EOF
)"
```

---

## Task 12 — ErrorEnvelopeTest

Asserts the error envelopes round-trip through `JsonUtility` with the codes/data the Rust runner expects to parse.

**Files:**
- Create: `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/ErrorEnvelopeTest.cs`

- [ ] **Step 12.1: Write the test**

Create `adapters/univrm/UniVRMConformance/Assets/Conformance/Tests/EditMode/ErrorEnvelopeTest.cs`:

```csharp
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
                        supported = new[] { "None" },
                    },
                },
            };
            var json = JsonUtility.ToJson(entry);
            StringAssert.Contains("\"code\":-32602", json);
            StringAssert.Contains("\"feature\":\"tone_mapping\"", json);
            StringAssert.Contains("\"value\":\"Aces\"", json);
            StringAssert.Contains("\"supported\":[\"None\"]", json);
        }

        [Test]
        public void LoadFailedAndRenderFailedShareEnvelope()
        {
            foreach (var (code, label) in new[] { (-32001, "LoadFailed"), (-32002, "RenderFailed") })
            {
                var entry = new Manifest.EntryDto
                {
                    test_id = "x",
                    status = "error",
                    error = new Manifest.ErrorDto
                    {
                        code = code,
                        message = label + ": detail",
                        data = new Manifest.ErrorDataDto { phase = "L3" },
                    },
                };
                var json = JsonUtility.ToJson(entry);
                StringAssert.Contains($"\"code\":{code}", json);
                StringAssert.Contains("\"phase\":\"L3\"", json);
            }
        }
    }
}
```

- [ ] **Step 12.2: Run → pass → commit**

```bash
"$UNITY_BIN" -batchmode -projectPath adapters/univrm/UniVRMConformance \
  -runTests -testPlatform EditMode \
  -testFilter "Conformance.Tests.ErrorEnvelopeTest" \
  -logFile - 2>&1 | grep "Test Passed\|Test Failed" | head -5

git add adapters/univrm/UniVRMConformance/Assets/Conformance
git commit -m "test(adapters/univrm): assert error envelope JSON shapes for -32000/-32001/-32002/-32602"
```

---

## Task 13 — `scripts/smoke-univrm.sh`

A local-only smoke script: generate `emit-default`, run through the runner's `execute-test-batch`, assert PNG produced + SSIM ≥ 0.75 against the existing three-vrm baseline for the same test_id.

**Files:**
- Create: `scripts/smoke-univrm.sh`

- [ ] **Step 13.1: Write the script**

Create `scripts/smoke-univrm.sh`:

```bash
#!/usr/bin/env bash
# Smoke test for the UniVRM adapter: generates a one-test corpus, runs
# it through the runner's execute-test-batch subcommand pointed at the
# real adapter, asserts a non-trivial PNG is produced and SSIM ≥ 0.75
# against three-vrm's baseline for the same test_id (assumes the
# three-vrm baseline is already in goldens-cache).
#
# Skip with SKIP_SMOKE=1 to short-circuit.
#
# Usage:
#   scripts/smoke-univrm.sh                 # runs full smoke
#   UNITY_BIN=/path/to/Unity ./scripts/smoke-univrm.sh

set -euo pipefail

if [ "${SKIP_SMOKE:-0}" = "1" ]; then
  echo "SKIP_SMOKE=1; exiting clean."
  exit 0
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Sanity: launcher exists, UNITY_BIN resolvable.
if [ ! -x adapters/univrm/launcher.sh ]; then
  echo "error: adapters/univrm/launcher.sh missing or not executable" >&2
  exit 1
fi
UNITY_BIN_PATH="${UNITY_BIN:-/Applications/Unity/Hub/Editor/6000.4.6f1/Unity.app/Contents/MacOS/Unity}"
if [ ! -x "$UNITY_BIN_PATH" ]; then
  echo "error: Unity binary not found at $UNITY_BIN_PATH" >&2
  echo "       set UNITY_BIN env or install Unity 6000.4.6f1" >&2
  exit 127
fi

SMOKE_DIR=/tmp/univrm-smoke
rm -rf "$SMOKE_DIR" && mkdir -p "$SMOKE_DIR/plans" "$SMOKE_DIR/out"

echo ">>> Generating one-test corpus (mtoon_default)"
cargo run --release -p vrm-asset-generator -- emit-default \
  --id smoke_mtoon_default \
  --output-dir "$SMOKE_DIR/plans/"

echo ">>> Running adapter via execute-test-batch"
cargo run --release -p vrm-runner -- execute-test-batch \
  --plans "$SMOKE_DIR/plans" \
  --adapter-bin adapters/univrm/launcher.sh \
  --output-dir "$SMOKE_DIR/out" \
  --renderer-name univrm \
  --json

echo ">>> Asserting output exists + non-trivial"
PNG="$SMOKE_DIR/out/smoke_mtoon_default.png"
if [ ! -f "$PNG" ]; then
  echo "FAIL: $PNG missing" >&2; exit 2
fi
size=$(stat -f%z "$PNG")
if [ "$size" -lt 10000 ]; then
  echo "FAIL: $PNG only $size bytes — render probably empty" >&2; exit 3
fi
echo "OK: PNG produced, $size bytes"

# Optional SSIM vs three-vrm baseline (only if baseline present).
BASELINE="goldens-cache/three-vrm/mtoon_default.png"
if [ -f "$BASELINE" ]; then
  echo ">>> SSIM vs three-vrm baseline"
  cargo run --release -p vrm-runner -- diff \
    --plan "$SMOKE_DIR/plans/smoke_mtoon_default.test.yaml" \
    --render "$PNG" \
    --reference "$BASELINE" \
    --renderer-name univrm \
    --json | tee "$SMOKE_DIR/diff.json"
  ssim=$(jq -r '.ssim' "$SMOKE_DIR/diff.json")
  echo "SSIM = $ssim"
  if awk "BEGIN{ exit !($ssim >= 0.75) }"; then
    echo "OK: SSIM $ssim ≥ 0.75"
  else
    echo "WARN: SSIM $ssim < 0.75 — UniVRM diverges from three-vrm baseline more than expected" >&2
    # Not exit-failing; this is informational pending corpus-wide bootstrap.
  fi
else
  echo "(no three-vrm baseline at $BASELINE; skipping SSIM check)"
fi

echo ">>> Smoke OK"
```

- [ ] **Step 13.2: Make executable and run**

```bash
chmod +x scripts/smoke-univrm.sh
scripts/smoke-univrm.sh
```

Expected: prints `>>> Smoke OK` at the end. If SSIM warning fires below 0.75, **investigate** but don't gate — the threshold is informational at this stage; corpus-wide data adjusts it.

- [ ] **Step 13.3: Commit**

```bash
git add scripts/smoke-univrm.sh
git commit -m "feat(scripts): smoke-univrm.sh — one-test E2E through the adapter"
```

---

## Task 14 — `scripts/bootstrap-goldens.sh` integration

Adds `RUN_UNIVRM=1` env flag (matching `RUN_THREE_VRM=1` convention) that drives the full 44-MToon-variant corpus through the UniVRM adapter and writes `goldens-cache/univrm/`.

**Files:**
- Modify: `scripts/bootstrap-goldens.sh`

- [ ] **Step 14.1: Add the UniVRM branch**

Find the existing `RUN_THREE_VRM` block in `scripts/bootstrap-goldens.sh` and add a parallel `RUN_UNIVRM` block after it. The structure should mirror `RUN_THREE_VRM` exactly: skip unless flag set, build a plans dir of all `*.test.yaml`+`*.vrm` pairs, invoke `cargo run -p vrm-runner -- execute-test-batch ...`, log the summary.

```bash
# (within scripts/bootstrap-goldens.sh — exact line numbers depend on
#  the current file; insert after the RUN_THREE_VRM block)

if [ "${RUN_UNIVRM:-0}" = "1" ]; then
  echo ">>> UniVRM (Unity, batched)"
  UNIVRM_OUT="$GOLDENS_DIR/univrm"
  mkdir -p "$UNIVRM_OUT"
  cargo run --release -p vrm-runner -- execute-test-batch \
    --plans "$ASSETS_DIR" \
    --adapter-bin adapters/univrm/launcher.sh \
    --output-dir "$UNIVRM_OUT" \
    --renderer-name univrm \
    --json | tee "$UNIVRM_OUT/run-summary.json"
fi
```

- [ ] **Step 14.2: Smoke run the full corpus**

```bash
scripts/bootstrap-goldens.sh RUN_UNIVRM=1 SKIP_VRM_METAL_KIT=1 SKIP_THREE_VRM=1
# (the SKIPs keep the bootstrap focused on UniVRM for this smoke;
#  full multi-renderer rerun is a separate step at the end of the plan)
ls -la goldens-cache/univrm/ | head
jq -r '.entries[] | "\(.status) \(.test_id)"' goldens-cache/univrm/local-manifest.json | head -20
```

Expected: 44 entries (the MToon corpus), most `ok`. Spring-bone tests (settle + swing, 36 tests) **must** produce `error` entries because L3 doesn't implement physics — they'll fail at load time (no `step_physics`/`reset_physics` op exists in the C# code yet). Document this in Step 14.3 — L4 closes that gap.

Wait: more precisely — the 36 spring-bone test_ids will appear in the plan dir and the runner will pass them in. Each will run our Phase 1 ops successfully (render the rest-pose), but **the rendered output will be wrong** because we won't have stepped physics. That's expected at L3 — the corpus will show valid PNGs for the spring-bone tests but the expected post-settle shapes won't match. We mark this as "L3 renders spring-bone tests in rest pose only" in the docs.

Decision: **render spring-bone tests in rest pose at L3** rather than rejecting them. Why: (a) they still produce a meaningful render (silhouette + materials are visible), (b) per-test result entries stay `ok`, the rendered PNG just won't match the expected post-physics pose, (c) this lines up with how three-vrm/vrm-metal-kit handle the same tests at their pre-L4 state. The L4 plan adds spring-bone stepping and re-runs the corpus.

- [ ] **Step 14.3: Commit**

```bash
git add scripts/bootstrap-goldens.sh
git commit -m "$(cat <<'EOF'
feat(scripts): bootstrap-goldens.sh RUN_UNIVRM=1 env flag

Adds the UniVRM branch parallel to RUN_THREE_VRM. Drives the full
80-test corpus through the UniVRM adapter via execute-test-batch.
Spring-bone tests render in rest pose at L3 (physics deferred to L4).
EOF
)"
```

---

## Task 15 — Top-level docs + findings entry

Update the three load-bearing docs that record adapter status, and add a `docs/findings.md` entry recording the first UniVRM-as-fourth-voter corpus run.

**Files:**
- Modify: `README.md` (adapter status table)
- Modify: `CLAUDE.md` (adapter-status bullet)
- Modify: `adapters/univrm/README.md` (status table — L3 row)
- Modify: `docs/findings.md` (new run section)

- [ ] **Step 15.1: README.md — bump univrm row**

In `README.md`, find the adapter-status table and update the `univrm` row from "L1+L2 scaffolded" to "L3 (Phase 1 ops real; spring-bone deferred to L4)".

- [ ] **Step 15.2: CLAUDE.md — adapter-status bullet**

In `CLAUDE.md`'s "Adapter status" section, update the `adapters/univrm/` bullet from "L1+L2 scaffold" to "L3 — Phase 1 ops real (44 MToon variants render through Unity + UniVRM v0.131.0 + Built-in RP). Spring-bone (Phase 2) deferred to L4. Requires Unity 6000.4.6f1 installed locally; CI does build-validate only."

- [ ] **Step 15.3: adapters/univrm/README.md — L3 row**

In `adapters/univrm/README.md` status table, change the L3 status from `deferred` to `shipped`. Add a one-paragraph "What L3 covers" section: load/camera/lighting/post-processing/render real; spring-bone tests render in rest pose (no physics stepping).

- [ ] **Step 15.4: docs/findings.md — record the first 4-voter run**

Append a new run section to `docs/findings.md`:

````markdown
## Run N: UniVRM joins as fourth renderer (L3)

**Trigger**: `docs/superpowers/plans/2026-05-13-adapter-univrm-L3.md` lands. UniVRM (Unity 6000.4.6f1 + UPM v0.131.0 + Built-in RP) renders the 44 MToon variants. Spring-bone tests render in rest pose (L4 deferred).

**Corpus stats** (`scripts/consensus-report.sh` after `scripts/bootstrap-goldens.sh RUN_UNIVRM=1`):

[Fill in: mean SSIM by pair, min pair, count of tests with all four renderers agreeing within 0.05 SSIM, count of tests where UniVRM is the lone outlier (which becomes the new "investigate three-vrm/VMK convention" list), count of tests where UniVRM disagrees with all three (= "UniVRM has a bug or our test plan is ambiguous" list).]

**Disambiguation reached on**:

[Fill in: which findings.md open issues UniVRM settles. Top of the list: `mtoon_outline_world_0p1` — does UniVRM produce the silhouette band the spec implies, or does it also flood like the other two? Whatever the answer, file the corresponding upstream issue or close the open one.]

**Still unresolved (UniVRM did not settle)**:

[Fill in: anything that remained ambiguous even with UniVRM data.]
````

The bracketed sections fill in after the first corpus run completes.

- [ ] **Step 15.5: Commit**

```bash
git add README.md CLAUDE.md adapters/univrm/README.md docs/findings.md
git commit -m "$(cat <<'EOF'
docs: UniVRM adapter L3 — Phase 1 ops real, fourth renderer in corpus

Updates README + CLAUDE.md adapter status to L3. adapters/univrm/README.md
gains the "what L3 covers" paragraph. docs/findings.md adds a new run
section recording the first four-renderer consensus baseline (numbers
filled in after bootstrap-goldens runs).
EOF
)"
```

---

## Final-pass checklist

Run these after Task 15 lands and the corpus has been bootstrapped:

- [ ] `cargo test --workspace` — all green
- [ ] `cargo fmt --all -- --check` — clean
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` — clean (no clippy fallout from the C# work, but the runner's manifest schema usage in execute_batch_tests.rs may need a touch)
- [ ] `scripts/smoke-univrm.sh` — green
- [ ] `scripts/bootstrap-goldens.sh RUN_UNIVRM=1` — produces 44 ok-status entries + 36 rest-pose entries
- [ ] `scripts/consensus-report.sh` — produces a 4-renderer matrix; outline + shadingShift test results compared against the three-renderer baseline
- [ ] `docs/findings.md` run-N section — filled in with actual numbers

If any of these fail, **do not claim L3 complete** — iterate on the failing item before the L4 plan opens.

---

## What this plan deliberately doesn't cover

1. **Phase 2 spring-bone physics** — `step_physics`, `reset_physics`, `animate_root_transform`, and the 36 swing/settle variants of the corpus. L3 renders spring-bone tests in rest pose. L4 (`2026-05-XX-adapter-univrm-L4.md`) adds VRMSpringBone manual stepping at 60 Hz mirroring the godot-vrm L4 pattern.

2. **URP / HDRP** — Built-in RP only. URP MToon is a port of the Built-in MToon; HDRP doesn't support MToon at all per UniVRM design. Adding URP later is a future-RFC scope expansion.

3. **CI rendering** — `.github/workflows/univrm.yml` does build-validate only (no `-runTests`-with-rendering, no RUN_UNIVRM=1 corpus). GitHub-hosted runners don't have the GPU + display config that `-batchmode` (without `-nographics`) needs for Metal. Corpus rendering remains local-only, same precedent as vrm-metal-kit.

4. **Reference-renderer status** — whether UniVRM becomes the new default `reference_renderer` in test plans (currently `vrm-metal-kit`) is a separate methodology decision, deferred to a `docs/findings.md` entry once corpus-wide UniVRM data exists.

5. **License management** — Unity Personal license lapses ~every 6 months; no automated test for the lapse path. Manual smoke test after each refresh cycle.

---

## References

- Design spec: [`docs/superpowers/specs/2026-05-12-adapter-univrm-design.md`](../specs/2026-05-12-adapter-univrm-design.md)
- L1+L2 scaffold plan: [`./2026-05-12-adapter-univrm-scaffold.md`](./2026-05-12-adapter-univrm-scaffold.md)
- RFC-0003 engine-idiom divergence: [`../../../rfcs/0003-engine-idiom-divergence.md`](../../../rfcs/0003-engine-idiom-divergence.md)
- godot-vrm L3 plan (precedent for adapter L3 structure): [`./2026-05-11-adapter-godot-vrm-L3.md`](./2026-05-11-adapter-godot-vrm-L3.md)
- UniVRM v0.131.0 source: `https://github.com/vrm-c/UniVRM/tree/v0.131.0/Packages/VRM10`
- `Vrm10.LoadPathAsync` signature: `https://github.com/vrm-c/UniVRM/blob/v0.131.0/Packages/VRM10/Runtime/IO/Vrm10.cs`
- `ImmediateCaller`: `https://github.com/vrm-c/UniVRM/blob/v0.131.0/Packages/UniGLTF/Runtime/UniGLTF/IO/AwaitCaller/ImmediateCaller.cs`
- VRMC_materials_mtoon-1.0 spec: `https://github.com/vrm-c/vrm-specification/blob/master/specification/VRMC_materials_mtoon-1.0/README.md`
