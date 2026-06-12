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
        // 0.16.0 (commit 392d949, released 2026-05-23 as **stable** —
        //   first non-pre-release in the cohort) consolidates rc.1
        //   through rc.4 plus one final spec-compliance fix (PR #298
        //   closing VMK#297) that landed after rc.4. Total scope vs
        //   0.15.2 (last stable):
        //   - 13 closures filed by this conformance suite: VMK#283,
        //          #286, #287, #288, #289, #290, #292, #293, #294,
        //          #297 + earlier 0.15.x closures. Plus VMK#239 and
        //          VMK#295 closed at upstream's discretion.
        //   - Stable corpus signal (this suite, 2026-05-23):
        //          - 632 / 632 plans render through this adapter
        //                  (100% stability — no Unimplemented, no
        //                  errors, no missing PNGs).
        //          - 247 / 263 conformance pass vs UniVRM consortium
        //                  reference (94%); the 19 below-threshold
        //                  failures are all known cross-renderer
        //                  methodology hazards (outline, matcap,
        //                  shaded-color texture aliasing) or
        //                  near-threshold ties — zero regression-class.
        //          - +42 absolute new passes over the rc.2 baseline
        //                  (205/206 on a 57-test smaller corpus).
        //   PR #298 (commit 53a68ea on the branch, squashed to 392d949)
        //   closes VMK#297 — the spec discrepancy this suite flagged in
        //   the rc.4 verification: VMK was writing lookAt expressions
        //   to custom-namespace `LookLeft/Right/Up/Down` (PascalCase)
        //   but the VRM 1.0 spec defines `lookLeft/Right/Up/Down`
        //   (lowercase) as preset expressions. The fix writes to BOTH
        //   namespaces (spec preset + legacy custom), so VRM 0.x assets
        //   keep working while spec-compliant VRM 1.0 assets finally
        //   get gaze applied. Verified locally: PR's 5 new unit tests
        //   pass; A/B vs rc.4 on 18-plan sample (MToon, spring-bone,
        //   VRMA lookAt) byte-identical; no behavioural drift outside
        //   the lookAt code path. End-to-end visual propagation on
        //   our synthetic lookAt corpus still requires suite-side
        //   asset extension (eye bones + lookAt preset expressions —
        //   a ~15-LoC follow-up in the asset generator).
        //   No code changes vs rc.4 in this stable cut beyond PR #298;
        //   our adapter wiring (`applyImmediately()` call added during
        //   rc.4 verification) is unchanged.
        // 0.16.0-rc.4 (commit 81ebce6, pre-released 2026-05-23) lands a
        //   single squashed PR #296 that closes all three suite-filed
        //   follow-ups against rc.3 plus a pre-existing rigid-follow
        //   regression that was surfaced during PR #291 development:
        //   - **PR #296 closes VMK#292** (swing-axis stiffness collapse on
        //          rc.3): rc.3's fixed-rate `synchronousSpringBone` timestep
        //          closed VMK#283 but introduced a settle/warmup interaction
        //          that left the conformance suite's swing-sweep stiffness
        //          axis collapsed to a single hash (`68b391e7764a2a9e`)
        //          across `stiffness_{0, 0.2, 0.8, 1}`. rc.4's `warmupPhysics`
        //          now drains settling frames so post-warmup animation runs
        //          with stiffness engaged. Verified: 9 swing axis variants
        //          produce 9 distinct hashes on rc.4 (see docs/findings.md
        //          "VMK 0.16.0-rc.4 verification").
        //   - **PR #296 closes VMK#293** (`occlusionTexture` silently
        //          dropped on MToon): rc.3's PR #291 closed
        //          `normalTexture.scale` (VMK#290) but the same
        //          `mtoon_pbr_textures_sweep` corpus also exercises
        //          `occlusionTexture.strength` — on rc.3 all three
        //          occlusion variants (baseline, default, strength_half)
        //          rendered byte-identical, because the texture binding
        //          itself was dropped. rc.4 wires the full path: parser
        //          populates texture data, uniform layout adds bindings,
        //          renderer activates texture slot 8, fragment shader
        //          applies the glTF formula `1.0 + strength * (ao - 1.0)`.
        //          Verified: 3 distinct hashes on rc.4.
        //   - **PR #296 closes VMK#294** (VRMA lookAt parsed but not
        //          propagated): rc.3's VMK#286 closure parsed yaw/pitch
        //          correctly but the parsed values never reached the
        //          rendered avatar's bones or expression weights. rc.4
        //          adds `VRMLookAtController.applyImmediately()` which
        //          resolves the queued gaze into eye-bone rotations
        //          (bone-driven) or `LookLeft/Right/Up/Down` custom
        //          expression weights (expression-driven), without
        //          waiting for the next frame-rate-dependent tick.
        //          **Adapter wiring update required**: this commit
        //          adds the `applyImmediately()` call in
        //          `Operations.swift::handleApplyVrmaAtTime` so the
        //          offline render path triggers propagation. Caveat:
        //          the suite's synthetic humanoid corpus lacks
        //          `leftEye`/`rightEye` bones and the lookAt custom
        //          expressions, so end-to-end visual propagation
        //          isn't yet observable on the conformance assets
        //          (extending the asset generator is a follow-up).
        //   - **PR #296 closes VMK#295** (center-node rigid follow
        //          CPU/GPU race): pre-existing
        //          `testCenterNodeTranslationDragsJointRigidly` failure
        //          surfaced during rc.3 follow-up work. Root cause:
        //          `applyCenterFrameDeltas` did the entire frame's
        //          center-node delta as a CPU-side memcpy *before* the
        //          substep loop, while the root bone was driven
        //          per-substep, leaving substep 1 with the child
        //          shifted 100% and the root only 1/N. The PBD
        //          distance constraint saw the chain stretched and
        //          yanked the child back. CPU-side per-substep fix
        //          can't work on the shared-command-buffer path
        //          (CPU writes to .storageModeShared land in one
        //          shot before GPU execution starts). rc.4 introduces
        //          a new Metal kernel `springBoneApplyCenterDelta`
        //          that applies per-substep deltas during GPU
        //          execution. Behavioural impact: spring-bone goldens
        //          for plans with non-trivial root translation will
        //          shift on rc.4; this affects most of the
        //          `swing_springbone_*` corpus.
        //   Behavioural changes on rc.4:
        //   - **All spring-bone goldens on plans using `animate_root_transform`
        //          will shift.** VMK#295's kernel change + VMK#292's warmup
        //          fix both alter integration output. Re-bootstrap of the
        //          swing/multichain corpora needed. Static settle plans
        //          (no root motion, no warmup interaction) are unaffected
        //          and remain byte-identical to rc.2/rc.3.
        //   - **`VRMLookAtController.applyImmediately()` is the offline-path
        //          contract** for invoking gaze propagation outside the
        //          live `update(deltaTime:)` smoothing tick. Adapters must
        //          call it after setting the controller target.
        // 0.16.0-rc.3 (commit 8cd3bc9, pre-released 2026-05-23) lands a
        //   single squashed PR #291 that closes six issues filed by this
        //   suite plus one long-standing open:
        //   - **PR #291 closes VMK#283** (animated swing non-determinism, take
        //          two): rc.2's PR #285 drain-only attempt did not close our
        //          reproducer (`swing_springbone_joints_16` still produced
        //          three distinct PNGs across five rc.2 runs on the same
        //          binary/input/hardware — see docs/findings.md "VMK
        //          0.16.0-rc.2 verification"). rc.3 replaces the
        //          wall-clock-paced `synchronousSpringBone` step with a
        //          fixed 60 Hz timestep whenever `simulationDeltaTime` is
        //          unset, eliminating timing variability between consecutive
        //          adapter invocations. This is the structural fix the
        //          rc.2 verification recommended; the drain alone left
        //          GPU/CPU scheduling jitter in the loop.
        //   - **PR #291 closes VMK#286** (VRMA lookAt rotation-channel
        //          gaze): `VRMAnimationLoader.loadVRMA(from:model:)`
        //          previously populated `clip.lookAtTargetSampler` only
        //          when the referenced node had a `translation` track.
        //          `@pixiv/three-vrm-animation`, the Pixiv VRMA sample
        //          set, and our `vrma_lookat_*` corpus all encode gaze
        //          as a `rotation` channel on the lookAt node — so the
        //          sampler stayed nil and `apply_vrma` was a silent
        //          no-op for gaze on every plan in that sweep. Fix adds
        //          `VRMAGLBBuilder.LookAtChannel` and parses both paths.
        //   - **PR #291 closes VMK#287** (MToon HDR emissive multiplier):
        //          `VRMC_materials_hdr_emissiveMultiplier-1.0.emissiveMultiplier`
        //          was being read but never applied to the rendered
        //          emission — `mtoon_emissive_multiplier_{0, 0.25, 0.5,
        //          1, 2, 5}` all rendered to the same PNG hash on rc.2.
        //          Closure means the multiplier reaches the shader as
        //          documented (alias of `KHR_materials_emissive_strength`).
        //   - **PR #291 closes VMK#288** (KHR_texture_transform on
        //          baseColorTexture): the eight `mtoon_uvxform_*` variants
        //          (offset/rotation/scale combinations) all rendered to
        //          one byte-identical PNG on rc.2; three-vrm produced
        //          eight distinct outputs. rc.3 wires the affine UV
        //          transform per the Khronos spec (`translation * rotation
        //          * scale`).
        //   - **PR #291 closes VMK#289** (outlineWidthMultiplyTexture
        //          degraded pipeline): the worst of the five rc.2
        //          MToon findings — setting `outlineWidthMultiplyTexture`
        //          activated a codepath that ignored per-vertex G-channel
        //          modulation **and** `outlineWidthFactor` **and**
        //          `outlineWidthMode` simultaneously, collapsing three
        //          materially different test variants to one PNG. Fix is
        //          "Outline pass texture binding and channel sampling
        //          fix" per the release notes — the binding now reaches
        //          the shader and the three input axes each have effect.
        //   - **PR #291 closes VMK#290** (normalTexture.scale ignored):
        //          glTF-core `normalTextureInfo.scale` was silently
        //          dropped on MToon materials — `mtoon_pbrtex_normal_default`
        //          (scale=1) and `mtoon_pbrtex_normal_scale_2x` (scale=2)
        //          rendered byte-identical on rc.2; UniVRM and three-vrm
        //          both produced distinct outputs. rc.3 threads the
        //          scale through the material pipeline.
        //   - **VMK#239 (shadingShift / shadingToony boundary collapse):**
        //          listed in the rc.3 release notes' closed-issues set
        //          but NOT in PR #291's commit message. Long-standing
        //          open since 0.15.0 (the Int/Double generalization
        //          partially addressed it but did not close
        //          `shadingShift=1.0`/`shadingToony=0.0` boundary cases).
        //          Verify empirically against `mtoon_shadingShift_*` and
        //          `mtoon_shadingToony_*` corpora — if boundary outputs
        //          have collapsed, the closure is real and likely a
        //          downstream effect of the MToon plumbing changes.
        //   Behavioural changes recorded in the rc.3 release notes:
        //   - **`synchronousSpringBone=true` now implies a fixed 60 Hz
        //          spring-bone timestep when `simulationDeltaTime` is
        //          unset.** Our adapter does not set `simulationDeltaTime`,
        //          so the entire swing-sweep corpus moves to the fixed-rate
        //          step on rc.3 — eliminates wall-clock variability and
        //          should restore byte-identical reproducibility on the
        //          surfaces that flickered under rc.1 / rc.2 / 0.15.2.
        //   - **MToon outline width now functional** via corrected texture
        //          binding (the #289 closure). Expect `mtoon_outline_*`
        //          variants that previously rendered identical to now
        //          diverge in line with the spec — this is a *deliberate*
        //          divergence vs rc.2 baselines; goldens for outline-related
        //          tests must be re-bootstrapped.
        //   - **Framework now builds for iOS, iOS Simulator, AND macOS**
        //          (previously macOS-only). Our adapter is unaffected
        //          (executable target stays macOS-only); broader product
        //          availability for downstream SPM consumers.
        // 0.16.0-rc.2 (commit 7f7d39b, pre-released 2026-05-22) adds two
        //   fixes on top of 0.16.0-rc.1:
        //   - **PR #285 closes VMK#283** (animated swing non-determinism):
        //          the self-committed `SpringBoneComputeSystem.update()`
        //          path (commandBuffer: nil — what our adapter uses) now
        //          drains the previous frame before overwriting
        //          `animatedRootPositionsBuffer` / `animatedRootPositionsPrevBuffer`.
        //          This closes the CPU/GPU race we filed against rc.1 after
        //          observing 5 runs of `swing_springbone_joints_16` produce
        //          3 distinct PNGs (blake3 `14b61fb5` ×2, `d5e06701` ×2,
        //          `1144c101` ×1; pairwise SSIM 0.9885–0.9897) on the same
        //          binary + same input + same hardware. 0.15.2 produced
        //          byte-identical output across all repetitions; rc.2 should
        //          restore that property. Behaviour-neutral for the
        //          shared-buffer renderer path and `synchronousSpringBone`
        //          path per the upstream PR description — both already
        //          drained before overwrite. This was a regression
        //          specifically on the path our conformance binary exercises.
        //   - **PR #281 closes VMK#280** (iOS metallib distribution):
        //          platform-specific precompiled metallibs ship in
        //          `Resources/` so iOS device/simulator builds load the
        //          FP16-defined slice without a local `make shaders` rebuild.
        //          No-op for our macOS adapter (we link the macOS metallib
        //          slice either way), but unblocks #279's mobile perf win
        //          for SPM consumers and fixes the iOS Simulator
        //          nil-pipeline error.
        //   No MToon or static spring-bone surface changes vs rc.1.
        //   Pre-release per upstream policy (-rc.N until Muse validates assets).
        // 0.16.0-rc.1 (commit 6a7084d, pre-released 2026-05-21) closes
        //   VMK#196/#237/#242/#243/#268/#273. RC verification (this suite,
        //   2026-05-21, docs/findings.md "VMK 0.16.0-rc.1 verification")
        //   found MToon (49) and static settle (82) byte-identical to
        //   0.15.2 and 190/191 conformance pass vs UniVRM, but flagged a
        //   reproducibility regression on the animated swing surface that
        //   was filed as VMK#283 and held the pin at 0.15.2. rc.2's PR
        //   #285 above is the closure of that hold.
        // 0.15.2 (commit de87578, released 2026-05-17) closes (PR #272):
        //   - **VRM 1.0 viseme weight coercion**: the expression parser cast
        //          `bind["weight"] as? Float` directly, but `JSONSerialization`
        //          decodes JSON numbers as `NSNumber` bridging to `Double`
        //          (or `Int` for whole-number literals like `1` / `0`). The
        //          `as? Float` cast silently failed for almost every bind,
        //          which hit `continue` and dropped the entry. Net effect:
        //          VRM 1.0 models loaded with `expressions.preset[.aa]` etc.
        //          populated but **empty `morphTargetBinds` arrays** —
        //          `setExpressionWeight(.aa, ...)` had nothing to deform.
        //          Visemes, blink, and emotion presets were all silently
        //          dead. This is the exact bug class as VMK#236 (collider
        //          parse silent-zero) and #238 (rim factor coercion), now
        //          applied to the expression-bind parser site.
        //   - **VRM 0.x `_ShadeTexture == _MainTex` washout**:
        //          `VRM0MaterialProperty.toMToonMaterial()` skipped
        //          `shadeMultiplyTexture` when `_ShadeTexture` and `_MainTex`
        //          pointed at the same texture index. Unity MToon and
        //          three-vrm's VRM0CompatPlugin always bind it — the 0.x
        //          export silently lost the shade input, leaving
        //          `shadeColorFactor=[1,1,1]` white as the only contribution.
        //          Fix binds unconditionally when `_ShadeTexture` is present.
        //   The viseme fix is the upstream landing of the bug class our
        //   newly-added viseme conformance coverage was built to surface:
        //   synthetic VRMs now carry POSITION morph targets bound to
        //   `aa/ih/ou/ee/oh`, and the VRMA expression sweep includes all
        //   five visemes. Re-render through VMK at this revision should
        //   produce visibly-deformed viseme outputs (SSIM divergence from
        //   any non-deforming baseline).
        // 0.15.1 (commit db5b90b, released 2026-05-17) closes:
        //   - VMK#269 (VRMA retargeting "zombie pose"): VRMAnimationLoader's
        //          makeRotationSampler used `L_B · L_A⁻¹ · A` which assumes
        //          the animation and target model share the same world rest
        //          orientation (W_A == W_B). VRMA_Locomotion_Pack/Idle.vrma
        //          is authored arms-forward (-Z); VRM 1.0 spec models extend
        //          arms along +X (T-pose), so the assumption fails and the
        //          per-frame rotation drags the model bone into the VRMA's
        //          pose space → both upper arms stuck forward. Fix ships
        //          the spec's pose-normalisation formula verbatim:
        //            Normalised = W_A · L_A⁻¹ · A.LocalRotation · W_A⁻¹
        //            B = L_B · W_B⁻¹ · Normalised · W_B
        //          The spec citation we forwarded at VMK#165 (closed in 0.15.1)
        //          and the conformance suite's phase 6 15-plan signal (0/15
        //          pass, worst per-bone divergence = exactly the authored
        //          angle) drove the closure.
        //   - VMK#270 (spring-bone twin-tails horizontal during rotation):
        //          we just filed this — fix ships in this release. The root
        //          cause matches the diagnostic we proposed: parent world
        //          rotation was being captured-once rather than read fresh
        //          each frame, so during rotation the stiffness restore
        //          direction stayed world-fixed.
        //   - VMK#264 (MToon discard_fragment defeats hardware A2C on MASK):
        //          opt-in MSAA alpha-to-coverage path landed.
        //   - VMK#265 (VRM 0.x _BlendMode=3 → transparentWithZWrite):
        //          conversion now explicit.
        //   Behavioural changes recorded in 0.15.1 release notes:
        //   - **Spring-bone gravity is ~12× stronger.** Asset gravityPower
        //          values may need reduction. The suite's existing
        //          springbone_gravity_{0..1} settle sweep WILL surface a
        //          ~12× change in tail rest position. This is a deliberate
        //          spec-conformance correction, not a regression on our
        //          end — but our tuned baseline plans may need re-rendering
        //          (which a re-bootstrap accomplishes).
        //   - **windAmplitude is now velocity-scale.** Hardcoded values
        //          need to be divided by ~60. We do not use windAmplitude
        //          in any test plan today (no wind axis in our sweep
        //          corpus), so this is a no-op for us. Would-be filed as
        //          a follow-up if we add a wind axis.
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
        // 0.18.0-rc.1 (commit aafc172, pre-released 2026-06-10, cut from
        // PR #334 "advanced concurrency + Metal") — perf + determinism RC:
        //   - **FP16 MToon shading on macOS** (−5.8% encoder time, fragment
        //          occupancy 21%→71% at 2048px; supersedes the FP32 macOS
        //          safe-default from #279). Fragment textures sample as
        //          `texture2d<half>`. Upstream claims pixel-clean against
        //          their MToon battery; this suite independently confirmed:
        //          370/370 A/B renders byte-identical to 0.17.2 with the
        //          changed FP16 metallib confirmed loaded (half4 returns
        //          are lossless for 8-bit content; math stays float).
        //   - **Spring-bone warmup determinism**: fixes an uninitialized
        //          `centerDeltaBuffer` read (could also write GPU memory out
        //          of bounds via a garbage bone range) and a CPU/GPU race
        //          where warmup rewrote shared buffers while steps were in
        //          flight. Independent loads of the same model now render
        //          bitwise-identically (`LoadDeterminismTests`). Same defect
        //          class as VMK#283 (rc-era swing non-determinism). Suite
        //          verification: joints_16 reproducer 5× byte-identical;
        //          170-plan spring-bone subset repeat byte-identical within
        //          BOTH versions (0.17.2 already stable on our render path;
        //          the fixed race lives on the load path the upstream
        //          `LoadDeterminismTests` witnesses).
        //   - On-disk `MTLBinaryArchive` pipeline persistence (opt-in via
        //          `RendererConfig.enablePipelineArchive`; default off — our
        //          adapter does not enable it). Known issue on macOS 27 beta
        //          (append-to-preloaded-archive), worked around upstream.
        //   - SIGSEGV fix for `%s` format specifiers fed Swift Strings.
        //   Regression verification for this pin: see docs/findings.md
        //   "VMK 0.18.0-rc.1 verification" (2026-06-09).
        // 0.17.2 (commit 3737e76, patch release 2026-06-08, closes #333) —
        // restores VRM 1.0 facial expressions. Behaviour change (no shader/
        // metallib change vs 0.17.1):
        //   - **VRM 1.0 morph binds were keyed by node, not mesh.** A 1.0
        //          expression `morphTargetBind.node` is a glTF *node* index,
        //          but the renderer and `VRMExpressionController` key morph
        //          weights by *mesh* index (0.x binds already carry the mesh
        //          index). The 1.0 loader stored the raw node index, so on any
        //          model whose face node index ≠ mesh index, every morph bind
        //          matched no primitive and the morph compute pass skipped it —
        //          blink, the five visemes, and every emotion preset silently
        //          produced no mesh deformation. The loader now resolves
        //          `node → nodes[node].mesh` into a resolved `meshIndex` while
        //          preserving the authored `node` for round-trip.
        //   Bone-driven look-at was unaffected (different path), which is why
        //   only *expressions* looked dead; VRM 0.x never hit it. Repro:
        //   `vroid_default_F_1_0` blink bind node=211 → mesh 0. This suite's
        //   new `vroid_default_F_expr_*` corpus (this commit) is the verifier —
        //   the synthetic humanoid corpus has no blink/happy/sad morphs and its
        //   visemes were silently frozen (node 19 ≠ mesh 0). Also adds
        //   `renderer.setExpression(_:weight:)` (additive). Rendering/
        //   before-after verification is local-only (macOS 26 / Xcode 26).
        // 0.17.1 (commit 421232b, patch release 2026-06-08, closes #332) —
        // corrects bone-driven eye look-at. Two behaviour changes to rendered
        // eye direction (no shader/metallib change vs 0.17.0):
        //   - **Head-local gaze resolution**: `updateTargetAngles` computed
        //          yaw/pitch in world space but wrote them as a *local*
        //          eye-bone rotation, so any turned head (body yawed at the
        //          root) drove the eyes off by the head's yaw. Targets now
        //          resolve through the head's inverse world matrix
        //          (`.headLocalPoint` was equally affected).
        //   - **Eye-bone rest composition**: `applyToBones` /
        //          `applyToAnimationState` overwrote the eye bones with a bare
        //          gaze quaternion, discarding the authored rest. VRoid rigs
        //          (`J_Adj_*_FaceEye`) carry a mirrored outward ~±22° eye rest;
        //          discarding it splayed the eyes wall-eyed at center and
        //          inverted gaze. Now composes `gaze * initialRotation`.
        //   This closes the long-deferred suite-side asset-coverage follow-up
        //   on `docs/upstream/VMK-vrma-lookat-renderer-propagation.md`: the new
        //   `vroid_default_F_gaze_*` corpus (this commit) drives gaze on the
        //   real VRoid avatar VMK names as the validation target — the
        //   synthetic humanoid corpus has no eye bones, which is why the
        //   `vrma_lookat_*` history could only ever verify the gaze *parse*.
        //   Rendering/before-after verification is local-only (macOS 26 /
        //   Xcode 26); CI build-validates the adapter but does not render.
        // 0.17.0 (commit 5cd0a95, **final** release 2026-06-07) — the
        // 1.0-candidate avatar-fidelity release. Consolidates rc.1…rc.5:
        //   - #324 spring-bone gravity at VRM spec scale (the 9.8× over-drive
        //          this suite reported; now `gravityDir · gravityPower`,
        //          matching UniVRM/three-vrm/godot). Verified at rc.4.
        //   - #326 VRM 0.x `gravityPower=0` respected — the `0→1.0`
        //          substitution this suite flagged is removed (authored
        //          inert chains stay inert, as in UniVRM/three-vrm).
        //   - #321 synthetic hand/arm colliders, #313 swept CCD, #309/#311/
        //          #312 synthetic collider augmentation, #316/#318 non-ultra
        //          `dtSub`, #322 render-order, #197 opt-in DQS (default-off).
        // First non-pre-release of the 0.17 line; SPM stable resolvers accept
        // it. Two intentional spring-bone behaviour changes vs 0.16.0 (gravity
        // ~9.8× less; authored gravityPower=0 inert) → spring-bone goldens
        // must be re-baselined. Bumped from rc.4 (b412db9) for the full
        // conformance run that gates this as the 1.0 candidate.
        .package(
            url: "https://github.com/arkavo-org/VRMMetalKit",
            // 0.20.1 (merge 39e65f0, released 2026-06-11, closes VMK#301
            // via PR #343) — skinned cull volume follows the skeleton
            // (hips-anchored) instead of an identity-matrix rest box pinned at
            // the load position. Corpus impact: NONE — 428/428 images render
            // byte-identical to 0.20.0 (the fix only changes cull decisions
            // for characters displaced from spawn, which no corpus frame
            // exercises). vs the previous 0.18.1 pin this also picks up the
            // 0.19/0.20 locomotion + vrmaprocess line; observed drift:
            // mtoon_pbrtex_occlusion_* (SSIM 0.905-0.964),
            // vrma_expression_preset_{oh,ih,aa,ee,ou} (0.907-0.946),
            // swing_springbone_* (~0.98) — pre-existing 0.18.1→0.20.0
            // changes, not the cull fix; goldens need re-baselining when
            // this pin lands.
            revision: "39e65f041a9786c79080f4afc6e4911f4bf4481b"
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
