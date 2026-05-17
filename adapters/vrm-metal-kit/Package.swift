// swift-tools-version: 6.2
import PackageDescription

// VRMMetalKit pins its platform floor at macOS 26 / iOS 26 (matching the
// Swift 6.3 / Xcode 26 toolchain). Our adapter follows the same floor so
// SPM can resolve the dependency without a `requires macos N, but depends
// on product requiring macos M` error.
//
// If we need to relax this — to support older macOS hosts for diff-only
// workflows — we'd have to vendor a subset of VRMMetalKit rather than
// re-platform the entire library.

let package = Package(
    name: "vrm-metal-kit-adapter",
    platforms: [.macOS(.v26)],
    products: [
        .executable(name: "vrm-metal-kit-adapter", targets: ["VRMMetalKitAdapter"]),
    ],
    dependencies: [
        // Pinned to a specific upstream revision so renderer regressions can be
        // bisected without library churn surprising us. Bump this revision when
        // a deliberate VRMMetalKit upgrade is part of the change.
        //
        // 0.15.0 (commit 5378ade) closes (all four were filed by this suite):
        //   - VMK#236 (VRMC_springBone collider parse silent-zero, scalar root
        //          cause): `parseVector3` returned nil for spec-typical mixed
        //          `[Double, Double, Int]` JSON arrays (e.g. `[0.02, -0.10, 0.0]`
        //          where the trailing whole-number `0.0` decodes as `Int(0)`).
        //          The nil fed the `?? SIMD3<Float>(0,0,0)` fallback at every
        //          collider site, so every VRMC_springBone collider sat at its
        //          owning bone's origin regardless of authored offset — which
        //          made the entire 24-variant collider sweep produce one
        //          byte-identical PNG (SHA prefix `f02fb44e3d2a`). 0.14.0 had
        //          fixed `[Double]`-vs-`[Float]` for *uniformly-double* arrays;
        //          this generalizes the fix to mixed Int/Double arrays.
        //          PR #258.
        //   - VMK#238 (MToon rimLightingMix=0 ≡ rimLightingMix=1, same root
        //          cause class): scalar `Float` factors were silently treated
        //          as Int by `AnyCodable`, so 0/1 boundary values funneled to
        //          a default. PR #254 generalizes the fix to every scalar
        //          factor parser site; PR #255 sweeps residual sites in
        //          `VRMExtensionParser`.
        //   - VMK#240 (spring-bone stiffness collapse under animation, root
        //          cause was at the warmup boundary, not the PBD math): the
        //          shader's `settlingStiffnessScale = 1 - smoothstep(0, 60,
        //          settlingFrames)` zeroes the stiffness contribution for any
        //          frame where `settlingFrames > 60`, and `warmupPhysics`
        //          never decremented the counter. Our 0.25 s animated swing
        //          window fell entirely inside that band, collapsing the
        //          `{0, 0.2, 0.8, 1}` stiffness sweep to one PNG hash. Fix
        //          consumes the counter inside `warmupPhysics`. PR #261.
        //   - VMK#213 (residual MToon shading-curve divergence after 0.13.5):
        //          PR #235 adds `LightNormalizationMode.radiometric` for
        //          spec-correct brightness on the shading-curve sweep.
        //   - VMK#228 (MToon front-face rim contribution regression lock):
        //          PR #234 regression test landed alongside #235.
        //   Behavioural change recorded in 0.15.0 release notes:
        //   - Spring-bone `warmupPhysics` now consumes `settlingFrames` —
        //     code that relied on the prior "warmup didn't tick the counter"
        //     behavior will see ~60 fewer frames of soft-start damping. Our
        //     test plans authored explicit `reset_physics(settle_steps=30)`
        //     so the behavioral change *unblocks* the stiffness sweep
        //     instead of regressing it. No conformance-plan edits needed.
        //   - GLTFMetalKit is a new sibling package; we do not depend on it.
        //   - `BoneParams` stride changed in the spring-bone uniform; only
        //     callers reaching directly into the uniform layout need updates,
        //     which we don't.
        //   Status going in (VMK tracker):
        //   - VMK#237 (extended_collider applied inconsistently) still open;
        //     PR #260 + #262 land phases 1-3 (plane, sphere, capsule parse
        //     and apply), but VMK#237 itself remains open pending end-to-end
        //     swing verification on capsule/sphere variants.
        //   - VMK#239 (shadingShift / shadingToony boundary collapse) still
        //     open in the VMK tracker; the boundary collapse pattern was
        //     partially addressed by the Int-vs-Double generalization, but
        //     #239 specifically (1.0/0.0 cast through `shadingShift` /
        //     `shadingToony` paths) hasn't been confirmed closed upstream.
        //     Re-render to verify.
        // 0.14.0 (commit f25a947) closes:
        //   - VMK#233 (spring-bone "zero settle": authored bind-direction is
        //          seeded into the kinematic reset instead of world -Y, and
        //          warmup-settled positions are applied to nodes on the first
        //          render frame so the bust chains don't pop on frame 0).
        //          Adds `VRMSpringBoneOverride` (set before `loadModel(_:)`)
        //          for rescuing badly-authored hair assets — `minGravityPower`,
        //          `maxStiffness`, `maxDragForce`, with optional joint-name
        //          predicate. We don't currently use the override; the kinematic
        //          fix lands automatically.
        //   - Collider parse silent-zero bug: VRM 1.0 sphere/capsule offsets,
        //          capsule tails, and plane normals were parsing as (0,0,0)
        //          because `JSONSerialization` returns `[Double]` but the
        //          parser cast to `[Float]`. Now accepts both. Hair was
        //          clipping through head/face on every VRM 1.0 asset with
        //          non-default colliders before this.
        //   - Load-time VRM 0.x → 1.0 coordinate conversion: the 180° Y
        //          rotation that used to live on `VRMRenderer` is now
        //          conjugated into VRM 0.x node TRS and inverse bind matrices
        //          at load. Physics, animation, and culling share a single
        //          coordinate space — closes a long-standing left/right limb
        //          handedness gap between formats.
        //   Behavioural change recorded in 0.14.0 release notes:
        //   - MToon shader now applies Half-Lambert remap (NdotL*0.5+0.5) for
        //     VRM 0.x assets via a new `vrmVersion` uniform; VRM 1.0 stays on
        //     spec-correct raw dot(N,L). Restores legacy VRM 0.x appearance
        //     that 0.13.3 (#183) had darkened when it removed Half-Lambert
        //     globally to fix a factor-only synthetic sphere. Our humanoid
        //     corpus is VRM 1.0, so the 1.0 path is unchanged for us; the
        //     0.x path now matches Unity-exported reference appearance.
        //   Conformance signal reported in the release notes (M4 Max):
        //     avatarA_bosom_zerosettle (VMK#233 verifier): 0.8396 vs three-vrm
        //     avatarA_bosom_swing:                          0.8351 vs three-vrm
        //     avatarA_bosom:                                0.7928 vs three-vrm
        //     avatarA_bosom_threequarter:                   0.7766 vs three-vrm
        //     avatarA_face:                                 0.6416 vs three-vrm
        //   VMK#228 (rim lift) and VMK#213 (shadingToony curve) remain open.
        // 0.13.7 (commit a6e2d6d) closes:
        //   - PR #231 (MToon: VRM 0.x `_ShadeToony`/`_ShadeShift` conversion
        //          now happens in VRM0MaterialProperty.toMToonMaterial()
        //          rather than the shader, so VRM 1.0 raw-NdotL toon-ramp
        //          path is no longer double-converting legacy materials.
        //          Also: defensive shader clamping for rimLightingMixFactor,
        //          shared `setupBrightToonLighting()` preset, Rec.709 luma
        //          AvatarSample_A regression test, AvatarSample_A.png
        //          regenerated. Shader hunk touches MToonShader.metal
        //          (+58/-31), VRMGeometry.swift (+5/-59), VRMRenderer.swift
        //          (+33/-15) — non-trivial despite the "test follow-ups"
        //          framing of the release notes. VMK#228 (rim lift) and
        //          VMK#213 (shadingToony curve) remain open going in.
        // 0.13.6 (commit a610223) closed:
        //   - #214 PR (pipeline cache: include pixel format + sample count
        //          in PSO cache key — adapter's config.colorPixelFormat
        //          write actually reaches the pipeline state object now)
        //   - #226 (MToon parametric-rim fresnel: move to world space —
        //          fresnel `viewDir·normalDir` term was using mismatched
        //          coordinate spaces, producing a constant position-offset
        //          rim band vs three-vrm + UniVRM. World-space normals
        //          match the spec's vector convention.)
        // 0.13.5 (commit c01ac8a) closed:
        //   - #205 (MToon shadingToonyFactor low values rendered as
        //          nearly-flat-lit; PR #207 adds /π Lambert normalization
        //          in MToonShader.metal so the lit/shaded transition curve
        //          matches three-vrm + UniVRM's BRDF_Lambert convention)
        //   - #206 (animate_root_transform no-op; PR #208 makes
        //          VRMNode.updateWorldTransform re-derive localMatrix
        //          from T/R/S so external translation writes finally
        //          reach worldMatrix — unblocks the entire swing-springbone
        //          surface)
        //   Behavioural changes recorded in 0.13.5 release notes:
        //   - setLightNormalizationMode(.manual(factor)) now multiplies on
        //     top of /π; scale factor by π for pre-0.13.5 brightness. (We
        //     don't use .manual; the default path gets the new
        //     normalization automatically.)
        //   - node.localMatrix is now owned by T/R/S. Direct assignment is
        //     transient. (Our adapter mutates node.translation, the new
        //     correct path.)
        // 0.13.4 (commit 4223876) closed:
        //   - #189 (GLTFParser BIN-chunk leak)
        //   - #190 (VRMA lookAt head-local space)
        //   - PR #188 (DocC catalog + 8 articles)
        // 0.13.3 (commit 83c9da1) closed:
        //   - #183 (flat-white MToon sphere — Half-Lambert remap saturated
        //          shadowStep=1 with shadingToonyFactor=0.9 + typical
        //          directional lighting; collapsed color to baseColor
        //          across the visible hemisphere)
        // 0.13.2 (commit d4bd52d) closed:
        //   - #185 (outline pass dispatching at world origin)
        // 0.13.1 (commit 9404287) closed:
        //   - #181 (non-skinned mesh dropped when skin present)
        //   - #182 (VRM 1.0 spring chain over-expansion)
        // All nine were first filed by this conformance suite.
        .package(
            url: "https://github.com/arkavo-org/VRMMetalKit",
            revision: "5378ade7e7d454e2c80ac5cd1821f2ce6feb1df6"
        ),
    ],
    targets: [
        .executableTarget(
            name: "VRMMetalKitAdapter",
            dependencies: [
                .product(name: "VRMMetalKit", package: "VRMMetalKit"),
            ]
        ),
        .testTarget(
            name: "VRMMetalKitAdapterTests",
            dependencies: ["VRMMetalKitAdapter"]
        ),
    ]
)
