// Operation dispatcher.
//
// Maps method names to op handlers. Through L2 every Phase 1 op returned a
// structured `Unimplemented` error; L3-b promoted `load_vrm` and `dispose`;
// L3-c promotes `set_camera`, `set_lighting`, `set_post_processing`, and
// `render` so the adapter now produces real PNGs from VRMMetalKit. Phase 2
// physics ops still return Unimplemented (L3-e).
//
// Per the operation contract (`docs/operation-contract.md`):
//   -32000 Unimplemented   — declared but not implemented in this version
//   -32001 LoadFailed      — `.vrm` failed to load
//   -32002 RenderFailed    — render step failed (OOM, GPU error, etc)
//   -32601 method-not-found
//   -32602 invalid params

import CoreGraphics
import Foundation
import ImageIO
@preconcurrency import Metal
import simd
import UniformTypeIdentifiers
import VRMMetalKit

/// Stateful op state: holds the Metal device that survives the adapter
/// lifetime and the per-session registry. A single `Operations.shared`
/// instance backs the default dispatcher; tests can construct fresh
/// instances for isolation.
final class Operations: @unchecked Sendable {
    /// Default singleton wired into `JsonRpcServer`'s default dispatcher.
    static let shared = Operations()

    /// Phase 1 contract ops. L3-c implements every one of these against
    /// VRMMetalKit. The set is preserved so tests can introspect what the
    /// contract claims, even though every op now has a real handler.
    static let phaseOneMethods: Set<String> = [
        "load_vrm",
        "set_camera",
        "set_lighting",
        "set_post_processing",
        "render",
        "dispose",
    ]

    /// Reserved ops — declared by every adapter, return Unimplemented in v0.1.
    /// Phase labels match `docs/operation-contract.md`. L3-e promoted the
    /// three Phase 2 physics ops out of the deferral block; they now have
    /// real handlers backed by VRMMetalKit's spring-bone GPU system.
    /// `render_sequence` was promoted to a real handler in Phase 5 Task 1.
    /// The five VRMA ops (`load_vrma` / `apply_vrma_at_time` / `dump_*`)
    /// were promoted on top of VMK 0.16.0-rc.2 — the underlying library
    /// has shipped `VRMAnimationLoader.loadVRMA(from:model:)` and the
    /// retargeting closures since 0.15.1 (VMK#269 pose-normalisation fix).
    static let reservedPhases: [String: String] = [
        "set_environment":         "v1.x",
        "set_expression":          "Phase 3",
        "set_humanoid_pose":       "Phase 2",
        "set_root_transform":      "Phase 2",
    ]

    /// Phase label for the still-deferred Phase 2 ops.
    private static let l3Deferral = "L3 (VRMMetalKit integration deferred)"

    /// MSAA sample count for every render. Pinned to 4 per
    /// `docs/methodology.md` ("Outline antialiasing — v1.0 standardizes on
    /// MSAA 4x"). The render request's `msaa` field is accepted but does
    /// not vary behavior — matching the three-vrm adapter, which similarly
    /// configures antialiasing at canvas creation rather than per render.
    static let msaaSampleCount: Int = 4

    /// Per-session state. Held as a reference type so set_* handlers can
    /// mutate fields without read-modify-write through the registry.
    private final class Session: @unchecked Sendable {
        let renderer: VRMRenderer
        let model: VRMModel

        // Camera (filled by set_camera; consumed by render).
        // Projection is rebuilt at render time because aspect = width/height
        // is only known then.
        var cameraPosition: SIMD3<Float>?
        var cameraTarget: SIMD3<Float>?
        var cameraUp: SIMD3<Float>?
        var cameraFovDegrees: Float?

        // Lighting (filled by set_lighting; consumed by render).
        var directionalDir: SIMD3<Float>?
        var directionalColor: SIMD3<Float>?
        var directionalIntensity: Float?
        var ambientColor: SIMD3<Float>?
        var ambientIntensity: Float?
        var castShadows: Bool = false

        // Post-processing (filled by set_post_processing; consumed at render).
        // VRMMetalKit doesn't expose tone mapping directly; we accept the
        // value to satisfy the contract but only honor "None" (which is the
        // MToon test-plan default per docs/methodology.md).
        var toneMapping: String = "None"
        var exposure: Float = 1.0

        // VRMA state captured per-session. The head-local look-at point is
        // recorded by `apply_vrma_at_time` so the dump op can derive yaw/
        // pitch without depending on `VRMLookAtController`'s smoothed
        // internal state (which is zero before any render call).
        var lastLookAtHeadLocalPoint: SIMD3<Float>?

        init(renderer: VRMRenderer, model: VRMModel) {
            self.renderer = renderer
            self.model = model
        }
    }

    private let device: MTLDevice?
    private let commandQueue: MTLCommandQueue?
    private var sessions: [String: Session] = [:]
    private var sessionCounter: Int = 0
    // VRMA clip registry. `load_vrma` per the op contract carries no
    // session_id — handles are process-scoped, not session-scoped.
    // Retargeting in `VRMAnimationLoader.loadVRMA` is bound to a model at
    // load time, so a clip is implicitly tied to the session whose model
    // was used to retarget it. In practice the conformance corpus runs one
    // model per adapter invocation, which matches the three-vrm reference.
    private var vrmaClips: [UInt32: AnimationClip] = [:]
    private var nextVrmaHandle: UInt32 = 1
    private let stateLock = NSLock()

    init() {
        self.device = MTLCreateSystemDefaultDevice()
        self.commandQueue = self.device?.makeCommandQueue()
    }

    /// Static-flavored phase lookup, used by `JsonRpcServerTests` to assert
    /// what phase label a given method declaration claims. Real dispatch
    /// happens via `dispatch(method:params:)`.
    static func phase(for method: String) -> String? {
        if phaseOneMethods.contains(method) {
            // Through L3-c every Phase 1 op has a real handler; this hook
            // exists for symmetry and for the post-L3 inventory tests.
            return nil
        }
        return reservedPhases[method]
    }

    /// Convenience: route the default dispatcher through the shared
    /// singleton so `JsonRpcServer`'s default `dispatcher` argument keeps
    /// working without callers constructing an Operations instance.
    static func dispatch(_ method: String, _ params: JSONValue?) -> OpOutcome {
        shared.dispatch(method: method, params: params)
    }

    // MARK: - Dispatch

    func dispatch(method: String, params: JSONValue?) -> OpOutcome {
        switch method {
        case "load_vrm":                return handleLoadVrm(params: params)
        case "set_camera":              return handleSetCamera(params: params)
        case "set_lighting":            return handleSetLighting(params: params)
        case "set_post_processing":     return handleSetPostProcessing(params: params)
        case "render":                  return handleRender(params: params)
        case "step_physics":            return handleStepPhysics(params: params)
        case "reset_physics":           return handleResetPhysics(params: params)
        case "animate_root_transform":  return handleAnimateRootTransform(params: params)
        case "render_sequence":         return handleRenderSequence(params: params)
        case "dump_bone_positions":     return handleDumpBonePositions(params: params)
        case "load_vrma":               return handleLoadVrma(params: params)
        case "apply_vrma_at_time":      return handleApplyVrmaAtTime(params: params)
        case "dump_humanoid_pose":      return handleDumpHumanoidPose(params: params)
        case "dump_expression_weights": return handleDumpExpressionWeights(params: params)
        case "dump_look_at_state":      return handleDumpLookAtState(params: params)
        case "dispose":                 return handleDispose(params: params)
        default:
            if let phase = Operations.reservedPhases[method] {
                return .error(
                    code: -32000,
                    message: "Unimplemented",
                    data: .object(["phase": .string(phase)])
                )
            }
            return .error(
                code: -32601,
                message: "Method not found: \(method)",
                data: nil
            )
        }
    }

    // MARK: - load_vrm / dispose (L3-b)

    private func handleLoadVrm(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let path) = obj["path"]
        else {
            return invalidParams("missing path")
        }
        guard FileManager.default.fileExists(atPath: path) else {
            return loadFailed("file not found: \(path)")
        }
        guard let device = device else {
            return loadFailed("no Metal device available on this host")
        }

        let url = URL(fileURLWithPath: path)
        switch blockingLoad(url: url, device: device) {
        case .failure(let err):
            return loadFailed("VRMModel.load failed: \(err)")
        case .success(let model):
            // VRMRenderer is created per-session and holds the loaded model
            // for its lifetime. strict=.off so a malformed asset surfaces
            // as a render-time fallback instead of an immediate throw — the
            // diff engine will catch the visual difference downstream.
            //
            // sampleCount pinned to 4 per docs/methodology.md "Outline
            // antialiasing — v1.0 standardizes on MSAA 4x". The render
            // request's `msaa` field is read for protocol compliance but
            // does not vary behavior (matches three-vrm's antialias=true
            // canvas convention).
            var config = RendererConfig()
            config.sampleCount = Operations.msaaSampleCount
            config.strict = .off
            // Run spring-bone physics synchronously per frame. This is the
            // documented offline-render path: skinning sees the current
            // frame's physics rather than the previous frame's snapshot,
            // and (since VRMMetalKit landed VMK#283's renderer-side fix)
            // the spring-bone integrator switches to a fixed 60Hz timestep
            // instead of CACurrentMediaTime() pacing so repeated runs of
            // the same input produce byte-identical output.
            config.synchronousSpringBone = true
            // Bake the sRGB-encoded color attachment into the pipeline state
            // at load time. VRMMetalKit's pipeline objects are locked to
            // `config.colorPixelFormat` here; if the render-time target's
            // pixel format diverges, Metal writes raw linear bytes to the
            // target regardless of the descriptor's sRGB flag. The
            // methodology pins `color_space: srgb` for SSIM-grade comparison
            // (docs/methodology.md), so .rgba8Unorm_srgb is the right
            // default. A test requesting Linear output (diagnostic only per
            // methodology) still routes through this pipeline; the cost is
            // an extra OETF cycle, not a correctness issue.
            //
            // This bug was surfaced by VMK#213 (vrm-conformance run 12) —
            // VMK rendered shadingShift_0p8 center pixels at byte 118
            // (linear of 0.461) while three-vrm + UniVRM produced byte 181
            // (sRGB-encoded). Locking the format here, plus the
            // case-insensitive render-time comparison below, closes that
            // gap entirely.
            config.colorPixelFormat = .rgba8Unorm_srgb
            let renderer = VRMRenderer(device: device, config: config)
            renderer.loadModel(model)
            // Enable spring-bone physics during draws. No-op for models
            // without VRMC_springBone data (drawCore guards on
            // `model.springBone != nil`). VRMMetalKit's loadModel already
            // populated the spring-bone GPU buffers + ran 30 warmup steps;
            // we just need to flip the runtime toggle.
            renderer.enableSpringBone = true

            stateLock.lock()
            sessionCounter += 1
            let id = "vrm-metal-kit-\(sessionCounter)"
            sessions[id] = Session(renderer: renderer, model: model)
            stateLock.unlock()

            // Surface spring-bone interop status on stderr so smoke tests
            // can see whether VRMMetalKit picked up the VRMC_springBone
            // data. Cheap one-line check — disambiguates "physics op is
            // a no-op because no spring-bone data was present" from a
            // real handler bug. (springBoneBuffers.numBones is internal;
            // we count joints across springs as a public proxy.)
            //
            // NOTE: this counts the POST-EXPANSION spring + joint tally.
            // For VRM 1.0 assets a 1-spring/4-joint declaration is reported
            // here as 3 springs / 9 joints because
            // expandVRM0SpringBoneChains() runs unconditionally — filed
            // upstream as
            // https://github.com/arkavo-org/VRMMetalKit/issues/182
            let nSprings = model.springBone?.springs.count ?? 0
            let nJoints = model.springBone?.springs.reduce(0) { $0 + $1.joints.count } ?? 0
            FileHandle.standardError.write(Data(
                "vrm-metal-kit-adapter: loaded \(id) — spring-bone: \(nSprings) springs, \(nJoints) joints\n".utf8
            ))

            return .ok(.object(["session_id": .string(id)]))
        }
    }

    private func handleDispose(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let id) = obj["session_id"]
        else {
            return invalidParams("missing session_id")
        }
        stateLock.lock()
        sessions.removeValue(forKey: id)
        stateLock.unlock()
        return .ok(.object([:]))
    }

    // MARK: - set_camera / set_lighting / set_post_processing (L3-c)

    private func handleSetCamera(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let sessionId) = obj["session_id"]
        else {
            return invalidParams("missing session_id")
        }
        guard let session = lookupSession(sessionId) else {
            return invalidParams("unknown session_id: \(sessionId)")
        }
        guard let position = parseVec3(obj["position"]),
              let target   = parseVec3(obj["target"]),
              let up       = parseVec3(obj["up"]),
              let fov      = parseFloat(obj["fov_degrees"])
        else {
            return invalidParams("position/target/up must each be [number; 3]; fov_degrees must be a number")
        }
        session.cameraPosition = position
        session.cameraTarget = target
        session.cameraUp = up
        session.cameraFovDegrees = fov
        return .ok(.object([:]))
    }

    private func handleSetLighting(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let sessionId) = obj["session_id"]
        else {
            return invalidParams("missing session_id")
        }
        guard let session = lookupSession(sessionId) else {
            return invalidParams("unknown session_id: \(sessionId)")
        }
        guard case .object(let dirObj) = obj["directional"],
              let dir = parseVec3(dirObj["dir"]),
              let color = parseVec3(dirObj["color"]),
              let intensity = parseFloat(dirObj["intensity"])
        else {
            return invalidParams("directional.{dir,color,intensity} required")
        }
        guard case .object(let ambObj) = obj["ambient"],
              let ambColor = parseVec3(ambObj["color"]),
              let ambIntensity = parseFloat(ambObj["intensity"])
        else {
            return invalidParams("ambient.{color,intensity} required")
        }
        session.directionalDir = dir
        session.directionalColor = color
        session.directionalIntensity = intensity
        session.ambientColor = ambColor
        session.ambientIntensity = ambIntensity
        if case .bool(let cs) = obj["cast_shadows"] {
            session.castShadows = cs
        }
        return .ok(.object([:]))
    }

    private func handleSetPostProcessing(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let sessionId) = obj["session_id"]
        else {
            return invalidParams("missing session_id")
        }
        guard let session = lookupSession(sessionId) else {
            return invalidParams("unknown session_id: \(sessionId)")
        }
        if case .string(let tm) = obj["tone_mapping"] {
            session.toneMapping = tm
        }
        if let exposure = parseFloat(obj["exposure"]) {
            session.exposure = exposure
        }
        return .ok(.object([:]))
    }

    // MARK: - render (L3-c)

    private func handleRender(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let sessionId) = obj["session_id"]
        else {
            return invalidParams("missing session_id")
        }
        guard case .number(let widthD) = obj["width"],
              let width = Int(exactly: widthD), width > 0,
              case .number(let heightD) = obj["height"],
              let height = Int(exactly: heightD), height > 0,
              case .string(let outputPath) = obj["output_path"],
              case .string(let colorSpace) = obj["color_space"]
        else {
            return invalidParams("width/height/output_path/color_space required")
        }
        guard let session = lookupSession(sessionId) else {
            return invalidParams("unknown session_id: \(sessionId)")
        }
        guard let device = device, let commandQueue = commandQueue else {
            return renderFailed("no Metal device or command queue available")
        }

        // ----- camera -----
        let position = session.cameraPosition ?? SIMD3<Float>(0, 1.4, 1.5)
        let target = session.cameraTarget ?? SIMD3<Float>(0, 1.4, 0)
        let up = session.cameraUp ?? SIMD3<Float>(0, 1, 0)
        let fov = session.cameraFovDegrees ?? 30.0
        let aspect = Float(width) / Float(height)
        session.renderer.projectionMatrix = perspective(
            fovRadians: fov * .pi / 180.0,
            aspect: aspect,
            near: 0.01,
            far: 100.0
        )
        session.renderer.viewMatrix = lookAt(eye: position, center: target, up: up)

        // ----- lighting -----
        // VRMMetalKit's `setLight(direction:)` takes direction-of-travel
        // (light → scene) — same convention as the operation contract.
        // Pass `dir` through unchanged.
        //
        // Confirmed against VMK internals (per VMK maintainer 2026-05-13):
        //   - VRMUniforms.swift:28-31 default stores direction-of-travel;
        //   - setup3PointLighting (VRMRenderer.swift:382-384) doc-comments
        //     "Light direction points FROM light TO scene";
        //   - MToonShader.metal:606 computes NdotL = dot(normal, -lightDirection)
        //     — the shader negates internally, so the stored value must be
        //     direction-of-travel for standard lit shading.
        //
        // Historical: this line used `-dir` until 2026-05-13. The negation
        // produced an antipodally inverted lit pole (all three world-axis
        // signs flipped on the lit-pole position), which surfaced visibly
        // as a Y-mirrored shadow on tests like mtoon_shadingShift_0p8 —
        // reported upstream as VMK#204 before the adapter root cause was
        // identified.
        if let dir = session.directionalDir,
           let color = session.directionalColor,
           let intensity = session.directionalIntensity {
            session.renderer.setLight(0, direction: dir, color: color, intensity: intensity)
            session.renderer.disableLight(1)
            session.renderer.disableLight(2)
        }
        if let ambColor = session.ambientColor,
           let ambIntensity = session.ambientIntensity {
            session.renderer.setAmbientColor(ambColor * ambIntensity)
        }

        // ----- render targets (MSAA 4x) -----
        // VRMMetalKit's pipeline state objects are built around
        // config.sampleCount (set at load_vrm). To match, every render
        // pass allocates a 4x-MSAA color + depth (private storage) and a
        // single-sample resolve target that receives the resolved pixels
        // via .multisampleResolve. CPU read-back happens from the resolve
        // target.
        //
        // Magenta clear color: matches the mock-renderer + three-vrm
        // sentinel so the diff engine's bbox-relative property assertions
        // can find the avatar against a known background.
        // Case-insensitive: the wire format is `vrm_test_plan::ColorSpace`
        // serialized via `#[serde(rename_all = "lowercase")]` ⇒ "srgb" /
        // "linear". The original adapter compared against PascalCase
        // "Srgb" — always false — silently selecting .rgba8Unorm and
        // writing linear bytes to the target. See VMK#213.
        let colorPixelFormat: MTLPixelFormat = (colorSpace.lowercased() == "srgb") ? .rgba8Unorm_srgb : .rgba8Unorm
        let sampleCount = Operations.msaaSampleCount

        // Multisample color attachment (.private — GPU only, not CPU-readable).
        let msColorDesc = MTLTextureDescriptor()
        msColorDesc.textureType = .type2DMultisample
        msColorDesc.pixelFormat = colorPixelFormat
        msColorDesc.width = width
        msColorDesc.height = height
        msColorDesc.sampleCount = sampleCount
        msColorDesc.usage = [.renderTarget]
        msColorDesc.storageMode = .private
        guard let msColorTex = device.makeTexture(descriptor: msColorDesc) else {
            return renderFailed("failed to create multisample color texture (\(width)×\(height) @ \(sampleCount)x)")
        }

        // Single-sample resolve target — receives the resolved pixels and
        // is the CPU-readable surface we PNG-export from.
        let resolveDesc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: colorPixelFormat,
            width: width, height: height, mipmapped: false
        )
        resolveDesc.usage = [.renderTarget, .shaderRead]
        resolveDesc.storageMode = .shared
        guard let resolveTex = device.makeTexture(descriptor: resolveDesc) else {
            return renderFailed("failed to create resolve color texture")
        }

        // Multisample depth — must match the color attachment's sampleCount
        // (Metal requires identical sample counts across attachments).
        let depthDesc = MTLTextureDescriptor()
        depthDesc.textureType = .type2DMultisample
        depthDesc.pixelFormat = .depth32Float
        depthDesc.width = width
        depthDesc.height = height
        depthDesc.sampleCount = sampleCount
        depthDesc.usage = [.renderTarget]
        depthDesc.storageMode = .private
        guard let msDepthTex = device.makeTexture(descriptor: depthDesc) else {
            return renderFailed("failed to create multisample depth texture")
        }

        let rpd = MTLRenderPassDescriptor()
        rpd.colorAttachments[0].texture = msColorTex
        rpd.colorAttachments[0].resolveTexture = resolveTex
        rpd.colorAttachments[0].loadAction = .clear
        rpd.colorAttachments[0].storeAction = .multisampleResolve
        rpd.colorAttachments[0].clearColor = MTLClearColor(red: 1.0, green: 0.0, blue: 1.0, alpha: 1.0)
        rpd.depthAttachment.texture = msDepthTex
        rpd.depthAttachment.loadAction = .clear
        rpd.depthAttachment.storeAction = .dontCare
        rpd.depthAttachment.clearDepth = 1.0

        guard let commandBuffer = commandQueue.makeCommandBuffer() else {
            return renderFailed("failed to make command buffer")
        }

        // drawOffscreenHeadless is @MainActor-isolated. Top-level code in
        // main.swift runs synchronously on the main thread (Swift 6 docs),
        // so this dispatch path executes on MainActor at runtime — making
        // `assumeIsolated` sound. The call records GPU commands and returns
        // immediately; the actual wait happens via the semaphore below,
        // which doesn't need the main thread.
        //
        // Tests don't reach this point (they exercise only error paths in
        // handleRender), so the runtime-isolation assertion never fires
        // off MainActor.
        MainActor.assumeIsolated {
            // drawOffscreenHeadless's `to`/`depth` args are used only for
            // size detection (`colorTexture.width/.height`); the actual
            // render targets come from the renderPassDescriptor. Pass the
            // multisample textures; size matches the resolve target.
            session.renderer.drawOffscreenHeadless(
                to: msColorTex,
                depth: msDepthTex,
                commandBuffer: commandBuffer,
                renderPassDescriptor: rpd
            )
        }

        // Block until the GPU finishes. The completion handler runs on a
        // Metal-private queue, so signalling the semaphore from there does
        // not require the main thread (no deadlock).
        let sem = DispatchSemaphore(value: 0)
        commandBuffer.addCompletedHandler { _ in sem.signal() }
        commandBuffer.commit()
        sem.wait()

        if let err = commandBuffer.error {
            return renderFailed("GPU error: \(err)")
        }

        // ----- PNG export -----
        // The resolve target carries the resolved (anti-aliased) pixels in
        // .shared storage, so it's directly CPU-readable via getBytes.
        do {
            try writeTexturePng(resolveTex, to: outputPath)
        } catch {
            return renderFailed("PNG export failed: \(error)")
        }

        return .ok(.object([
            "output_path": .string(outputPath),
            "actual_color_space": .string(colorSpace),
        ]))
    }

    // MARK: - Physics ops (L3-e)

    /// Advance spring-bone physics by `count` steps. VRMMetalKit's public
    /// physics-stepping API (`warmupPhysics`) zeros velocity at the end —
    /// fine for "advance from settled state" semantics, which matches the
    /// mock-renderer's deterministic no-op handling. Test plans that need
    /// velocity-accumulating motion use `animate_root_transform` instead.
    ///
    /// `dt_seconds` is accepted for protocol compliance but ignored:
    /// VRMMetalKit's spring-bone simulation uses fixed 120Hz substeps (per
    /// the `.ultra` quality preset) regardless of the requested dt.
    private func handleStepPhysics(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let sessionId) = obj["session_id"]
        else {
            return invalidParams("missing session_id")
        }
        guard let session = lookupSession(sessionId) else {
            return invalidParams("unknown session_id: \(sessionId)")
        }
        let count: Int = {
            if case .number(let n) = obj["count"], let i = Int(exactly: n) { return max(0, i) }
            return 1
        }()
        if session.model.springBone == nil {
            // No-op for non-spring-bone models — matches the mock + three-vrm.
            return .ok(.object([:]))
        }
        MainActor.assumeIsolated {
            session.renderer.warmupPhysics(steps: count)
        }
        return .ok(.object([:]))
    }

    /// Reset spring-bone physics to a settled rest pose: zero velocity and
    /// run `settle_steps` silent physics steps so the chain hangs naturally
    /// before measurement. VRMMetalKit's `warmupPhysics(steps:)` already
    /// does both — it resets state at entry, runs N steps, and zeros
    /// velocity at exit so subsequent renders start from clean equilibrium.
    private func handleResetPhysics(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let sessionId) = obj["session_id"]
        else {
            return invalidParams("missing session_id")
        }
        guard let session = lookupSession(sessionId) else {
            return invalidParams("unknown session_id: \(sessionId)")
        }
        let settleSteps: Int = {
            if case .number(let n) = obj["settle_steps"], let i = Int(exactly: n) { return max(0, i) }
            return 30
        }()
        if session.model.springBone == nil {
            return .ok(.object([:]))
        }
        MainActor.assumeIsolated {
            session.renderer.warmupPhysics(steps: settleSteps)
        }
        return .ok(.object([:]))
    }

    /// Dump world-space joint positions for every spring chain (or one
    /// selected chain) after the current physics state.
    ///
    /// Access path: `session.model.springBone.springs[i]` → `VRMSpring`
    /// with `.name` and `.joints: [VRMSpringJoint]`. Each joint carries a
    /// `.node: Int` index into `session.model.nodes`. The node exposes
    /// `.worldPosition: SIMD3<Float>` — the same property used by
    /// `BoneTrajectoryDumper.recordFrame` in VRMMetalKit.
    ///
    /// Result shape:
    /// ```json
    /// {"springs":[{"name":"hair_L","joint_positions":[[x,y,z],...]},...]}
    /// ```
    ///
    /// `spring_index` (optional): when present, only that chain is returned;
    /// an out-of-range index yields an empty `springs` array (not an error).
    private func handleDumpBonePositions(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let sessionId) = obj["session_id"]
        else {
            return invalidParams("missing session_id")
        }
        guard let session = lookupSession(sessionId) else {
            return invalidParams("unknown session_id: \(sessionId)")
        }

        guard let springBone = session.model.springBone else {
            // Model has no spring chains — return an empty but valid result.
            return .ok(.object(["springs": .array([])]))
        }

        let allSprings = springBone.springs
        let springIndex: Int? = {
            if case .number(let n) = obj["spring_index"], let i = Int(exactly: n) { return i }
            return nil
        }()

        let chains: [VRMSpring]
        if let idx = springIndex {
            guard idx >= 0 && idx < allSprings.count else {
                return .ok(.object(["springs": .array([])]))
            }
            chains = [allSprings[idx]]
        } else {
            chains = allSprings
        }

        let nodes = session.model.nodes
        let dumped: [JSONValue] = chains.enumerated().map { (i, spring) in
            let name = spring.name ?? "spring_\(i)"
            let jointPositions: [JSONValue] = spring.joints.compactMap { joint in
                guard joint.node >= 0 && joint.node < nodes.count else { return nil }
                let p = nodes[joint.node].worldPosition
                return .array([.number(Double(p.x)), .number(Double(p.y)), .number(Double(p.z))])
            }
            return .object([
                "name": .string(name),
                "joint_positions": .array(jointPositions),
            ])
        }

        return .ok(.object(["springs": .array(dumped)]))
    }

    /// Linearly translate the avatar root over `duration_seconds` and run
    /// spring-bone physics per frame so the chain trails behind the motion.
    ///
    /// Per VRMMetalKit's HairFlutterTrajectoryTests pattern: physics
    /// stepping outside of `drawCore` requires issuing per-frame renders
    /// (drawCore is what ticks `SpringBoneComputeSystem.update`). We render
    /// each frame into a 64×64 throwaway target — the GPU commit overhead
    /// provides natural pacing for the renderer's CACurrentMediaTime-based
    /// deltaTime calculation, and the chain reads the model's per-frame
    /// root positions through VRMMetalKit's internal animatedPositions path.
    ///
    /// `characterVelocity` is set to the constant root velocity over the
    /// animation so the inertia-compensation kernel applies the right
    /// trailing force. Restored to zero at end so subsequent renders don't
    /// inherit phantom motion.
    private func handleAnimateRootTransform(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let sessionId) = obj["session_id"]
        else {
            return invalidParams("missing session_id")
        }
        guard let session = lookupSession(sessionId) else {
            return invalidParams("unknown session_id: \(sessionId)")
        }
        guard let startV = parseVec3(obj["translation_start"]),
              let endV = parseVec3(obj["translation_end"]),
              let duration = parseFloat(obj["duration_seconds"])
        else {
            return invalidParams("translation_start/translation_end/duration_seconds required")
        }
        let fps: Int = {
            if case .number(let n) = obj["fps"], let i = Int(exactly: n), i > 0 { return i }
            return 60
        }()
        if session.model.springBone == nil {
            // Non-spring-bone models have nothing to excite; treat as no-op
            // for protocol compliance.
            return .ok(.object([:]))
        }
        guard let device = device, let commandQueue = commandQueue else {
            return renderFailed("no Metal device or command queue available")
        }

        // Total frames in the animation. Always at least 1 so a zero-duration
        // request still produces a final-frame state without divide-by-zero.
        let totalFrames = max(1, Int((duration * Float(fps)).rounded()))
        let velocity: SIMD3<Float> = duration > 0
            ? (endV - startV) / duration
            : .zero

        // Find root nodes (parent == nil) and snapshot their original
        // translations. The animation overlays an offset; setting
        // `node.translation` outright would collapse multi-root avatars to
        // the same point.
        let rootNodes: [VRMNode] = session.model.nodes.filter { $0.parent == nil }
        let originalTranslations: [SIMD3<Float>] = rootNodes.map { $0.translation }

        // 64×64 throwaway color + depth (single-sample, .private). MSAA
        // adds setup cost without affecting the physics step, so the
        // animation loop uses a cheap single-sample target — the final
        // render() after animate_root_transform uses the full MSAA path.
        let colorDesc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm, width: 64, height: 64, mipmapped: false
        )
        colorDesc.usage = [.renderTarget]
        colorDesc.storageMode = .private
        let depthDesc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .depth32Float, width: 64, height: 64, mipmapped: false
        )
        depthDesc.usage = [.renderTarget]
        depthDesc.storageMode = .private
        guard let dummyColor = device.makeTexture(descriptor: colorDesc),
              let dummyDepth = device.makeTexture(descriptor: depthDesc)
        else {
            return renderFailed("animation: failed to create throwaway physics-step target")
        }

        // Renderer's pipelines were built for MSAA 4x at load_vrm. Since
        // our dummy targets are single-sample, we need a fresh single-sample
        // VRMRenderer for the animation loop. But spawning a second
        // VRMRenderer shares the same VRMModel — they coexist fine.
        var singleSampleConfig = RendererConfig()
        singleSampleConfig.sampleCount = 1
        singleSampleConfig.strict = .off
        // Match the session renderer's offline-render configuration so the
        // animation-driving renderer also runs spring-bone on a fixed 60Hz
        // timestep instead of wall-clock pacing (VMK#283).
        singleSampleConfig.synchronousSpringBone = true
        let physicsRenderer = VRMRenderer(device: device, config: singleSampleConfig)
        physicsRenderer.loadModel(session.model)
        physicsRenderer.enableSpringBone = true
        physicsRenderer.characterVelocity = velocity

        for i in 1...totalFrames {
            let t = Float(i) / Float(totalFrames)
            let offset = startV + (endV - startV) * t
            for (idx, root) in rootNodes.enumerated() {
                root.translation = originalTranslations[idx] + offset
                root.updateWorldTransform()
            }

            guard let cb = commandQueue.makeCommandBuffer() else {
                return renderFailed("animation: failed to make command buffer at frame \(i)")
            }
            let rpd = MTLRenderPassDescriptor()
            rpd.colorAttachments[0].texture = dummyColor
            rpd.colorAttachments[0].loadAction = .clear
            rpd.colorAttachments[0].storeAction = .store
            rpd.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 1)
            rpd.depthAttachment.texture = dummyDepth
            rpd.depthAttachment.loadAction = .clear
            rpd.depthAttachment.storeAction = .dontCare
            rpd.depthAttachment.clearDepth = 1.0

            MainActor.assumeIsolated {
                physicsRenderer.drawOffscreenHeadless(
                    to: dummyColor,
                    depth: dummyDepth,
                    commandBuffer: cb,
                    renderPassDescriptor: rpd
                )
            }
            let sem = DispatchSemaphore(value: 0)
            cb.addCompletedHandler { _ in sem.signal() }
            cb.commit()
            sem.wait()
            if let err = cb.error {
                return renderFailed("animation: GPU error at frame \(i): \(err)")
            }
        }

        // Clear inertia hint so subsequent renders don't see phantom motion.
        physicsRenderer.characterVelocity = .zero
        return .ok(.object([:]))
    }

    // MARK: - render_sequence (Phase 5 Task 1)

    /// Multi-frame capture with optional root-transform animation (RFC-0004).
    ///
    /// Each frame is rendered through the same MSAA 4× path as `handleRender`
    /// after applying the linear root-transform interpolation. Physics is
    /// driven implicitly by each frame's real draw call (VMK steps
    /// spring-bone during render).
    ///
    /// Phase 5 scope: PNG sequence + animate_root_transform only.
    /// `apply_vrma` and `output_format` ∈ {Mp4, Mov} are rejected as
    /// invalid_params; both ship as follow-ups.
    ///
    /// `blake3` on each emitted SequenceFrame is populated with a 64-zero
    /// sentinel. The runner re-hashes from PNG bytes after this method
    /// returns so the manifest gets real hashes (centralizes hashing in
    /// Rust where the diff engine already uses blake3).
    private func handleRenderSequence(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let sessionId) = obj["session_id"]
        else { return invalidParams("missing session_id") }

        guard case .number(let widthD) = obj["width"],
              let width = Int(exactly: widthD), width > 0,
              case .number(let heightD) = obj["height"],
              let height = Int(exactly: heightD), height > 0,
              case .string(let outputDir) = obj["output_dir"],
              case .number(let frameCountD) = obj["frame_count"],
              let frameCount = Int(exactly: frameCountD), frameCount > 0,
              case .number(let frameHz) = obj["frame_hz"],
              case .number(let physicsDt) = obj["physics_dt_seconds"],
              case .string(let colorSpace) = obj["color_space"],
              case .string(let outputFormat) = obj["output_format"]
        else {
            return invalidParams("missing or malformed required render_sequence fields")
        }

        // 60 Hz physics floor (methodology pin, RFC-0004 failure-modes).
        // Tolerance 1e-6 absorbs f32 round-trip noise: runners send the
        // value as f32, so 1.0_f32 / 60.0_f32 = 0.016666668 lands on the
        // wire and parses to a Double slightly above the true 1/60. Real
        // overages (e.g. 0.02) are still rejected — the tolerance is far
        // below any meaningful rate change.
        if physicsDt > 1.0 / 60.0 + 1e-6 {
            return invalidParams("physics_dt_seconds \(physicsDt) exceeds 60 Hz floor (1/60 ≈ 0.01667)")
        }

        // Mutual exclusion: animate_root_transform + apply_vrma both set ⇒ reject.
        let hasRootAnim = isPresent(obj["animate_root_transform"])
        let hasVrma = isPresent(obj["apply_vrma"])
        if hasRootAnim && hasVrma {
            return invalidParams("animate_root_transform and apply_vrma are mutually exclusive")
        }
        if hasVrma {
            return invalidParams("apply_vrma is not yet implemented in vrm-metal-kit (Phase 5 deferral)")
        }

        // Only PNG sequence is supported in this phase. Mp4/Mov mux is a
        // follow-up.
        if outputFormat != "png_sequence" {
            return invalidParams("output_format \"\(outputFormat)\" is not yet supported by vrm-metal-kit; only png_sequence")
        }

        guard let session = lookupSession(sessionId) else {
            return invalidParams("unknown session_id: \(sessionId)")
        }
        guard let device = device, let commandQueue = commandQueue else {
            return renderFailed("no Metal device or command queue available")
        }

        // Parse optional animate_root_transform; absent ⇒ no movement.
        var startV = SIMD3<Float>(0, 0, 0)
        var endV = SIMD3<Float>(0, 0, 0)
        if hasRootAnim, case .object(let anim) = obj["animate_root_transform"] {
            guard let s = parseVec3(anim["translation_start"]),
                  let e = parseVec3(anim["translation_end"])
            else {
                return invalidParams("animate_root_transform: translation_start/end required as [f32; 3]")
            }
            startV = s
            endV = e
        }

        // Snapshot root translations for restoration after the loop so
        // subsequent ops don't see drift.
        let rootNodes: [VRMNode] = session.model.nodes.filter { $0.parent == nil }
        let originalTranslations: [SIMD3<Float>] = rootNodes.map { $0.translation }

        // Camera + lighting — same as handleRender. Persisted once before
        // the loop; the renderer's matrices and lighting state are stable
        // across frames within a single render_sequence call.
        let position = session.cameraPosition ?? SIMD3<Float>(0, 1.4, 1.5)
        let target = session.cameraTarget ?? SIMD3<Float>(0, 1.4, 0)
        let up = session.cameraUp ?? SIMD3<Float>(0, 1, 0)
        let fov = session.cameraFovDegrees ?? 30.0
        let aspect = Float(width) / Float(height)
        session.renderer.projectionMatrix = perspective(
            fovRadians: fov * .pi / 180.0,
            aspect: aspect,
            near: 0.01,
            far: 100.0
        )
        session.renderer.viewMatrix = lookAt(eye: position, center: target, up: up)
        if let dir = session.directionalDir,
           let color = session.directionalColor,
           let intensity = session.directionalIntensity {
            session.renderer.setLight(0, direction: dir, color: color, intensity: intensity)
            session.renderer.disableLight(1)
            session.renderer.disableLight(2)
        }
        if let ambColor = session.ambientColor, let ambIntensity = session.ambientIntensity {
            session.renderer.setAmbientColor(ambColor * ambIntensity)
        }

        // Ensure output_dir exists.
        do {
            try FileManager.default.createDirectory(
                atPath: outputDir,
                withIntermediateDirectories: true,
                attributes: nil
            )
        } catch {
            return renderFailed("create output_dir \(outputDir): \(error)")
        }

        let colorPixelFormat: MTLPixelFormat =
            (colorSpace.lowercased() == "srgb") ? .rgba8Unorm_srgb : .rgba8Unorm
        let sampleCount = Operations.msaaSampleCount
        let zeroHash = "blake3:" + String(repeating: "0", count: 64)

        var frames: [JSONValue] = []

        for i in 0..<frameCount {
            // Interpolate root translation. For frameCount==1, t=0 (no animation).
            let t: Float = frameCount > 1 ? Float(i) / Float(frameCount - 1) : 0
            let offset = startV + (endV - startV) * t
            for (idx, root) in rootNodes.enumerated() {
                root.translation = originalTranslations[idx] + offset
                root.updateWorldTransform()
            }

            // Per-frame MSAA targets — allocating inside the loop keeps memory
            // pressure flat (resolve target's .shared bytes deallocate after
            // each PNG export).
            let msColorDesc = MTLTextureDescriptor()
            msColorDesc.textureType = .type2DMultisample
            msColorDesc.pixelFormat = colorPixelFormat
            msColorDesc.width = width
            msColorDesc.height = height
            msColorDesc.sampleCount = sampleCount
            msColorDesc.usage = [.renderTarget]
            msColorDesc.storageMode = .private
            guard let msColorTex = device.makeTexture(descriptor: msColorDesc) else {
                return renderFailed("frame \(i): failed to create MS color texture")
            }

            let resolveDesc = MTLTextureDescriptor.texture2DDescriptor(
                pixelFormat: colorPixelFormat,
                width: width, height: height, mipmapped: false
            )
            resolveDesc.usage = [.renderTarget, .shaderRead]
            resolveDesc.storageMode = .shared
            guard let resolveTex = device.makeTexture(descriptor: resolveDesc) else {
                return renderFailed("frame \(i): failed to create resolve texture")
            }

            let depthDesc = MTLTextureDescriptor()
            depthDesc.textureType = .type2DMultisample
            depthDesc.pixelFormat = .depth32Float
            depthDesc.width = width
            depthDesc.height = height
            depthDesc.sampleCount = sampleCount
            depthDesc.usage = [.renderTarget]
            depthDesc.storageMode = .private
            guard let msDepthTex = device.makeTexture(descriptor: depthDesc) else {
                return renderFailed("frame \(i): failed to create MS depth texture")
            }

            let rpd = MTLRenderPassDescriptor()
            rpd.colorAttachments[0].texture = msColorTex
            rpd.colorAttachments[0].resolveTexture = resolveTex
            rpd.colorAttachments[0].loadAction = .clear
            rpd.colorAttachments[0].storeAction = .multisampleResolve
            rpd.colorAttachments[0].clearColor = MTLClearColor(red: 1.0, green: 0.0, blue: 1.0, alpha: 1.0)
            rpd.depthAttachment.texture = msDepthTex
            rpd.depthAttachment.loadAction = .clear
            rpd.depthAttachment.storeAction = .dontCare
            rpd.depthAttachment.clearDepth = 1.0

            guard let commandBuffer = commandQueue.makeCommandBuffer() else {
                return renderFailed("frame \(i): failed to make command buffer")
            }

            MainActor.assumeIsolated {
                session.renderer.drawOffscreenHeadless(
                    to: msColorTex,
                    depth: msDepthTex,
                    commandBuffer: commandBuffer,
                    renderPassDescriptor: rpd
                )
            }
            let sem = DispatchSemaphore(value: 0)
            commandBuffer.addCompletedHandler { _ in sem.signal() }
            commandBuffer.commit()
            sem.wait()
            if let err = commandBuffer.error {
                return renderFailed("frame \(i): GPU error: \(err)")
            }

            // Export PNG.
            let framePath = "\(outputDir)/\(String(format: "%04d", i)).png"
            do {
                try writeTexturePng(resolveTex, to: framePath)
            } catch {
                return renderFailed("frame \(i): PNG export failed: \(error)")
            }

            frames.append(.object([
                "index": .number(Double(i)),
                "timestamp_seconds": .number(Double(i) / Double(frameHz)),
                "path": .string(framePath),
                "blake3": .string(zeroHash),
            ]))
        }

        // Restore original root translations so subsequent ops on this
        // session see the avatar at rest.
        for (idx, root) in rootNodes.enumerated() {
            root.translation = originalTranslations[idx]
            root.updateWorldTransform()
        }

        let durationSeconds = frameHz > 0 ? Double(frameCount) / frameHz : 0.0

        return .ok(.object([
            "frames": .array(frames),
            "duration_seconds": .number(durationSeconds),
            "actual_color_space": .string(colorSpace),
            "frame_hz_achieved": .number(frameHz),
        ]))
    }

    /// True iff the value is present and not JSONValue.null.
    private func isPresent(_ v: JSONValue?) -> Bool {
        guard let v = v else { return false }
        if case .null = v { return false }
        return true
    }

    // MARK: - VRMA ops (Phase 7 — VMK 0.16.0-rc.2)

    /// Conformance-relevant humanoid-bone subset, matching the three-vrm
    /// adapter's `HUMANOID_BONES` list (renderer-host.html). 19 bones cover
    /// the VRMA spec's required humanoid surface; finger and twist bones
    /// are not exercised by the cross-renderer pose diff.
    private static let referenceHumanoidBones: [String] = [
        "hips", "spine", "chest", "neck", "head",
        "leftShoulder", "leftUpperArm", "leftLowerArm", "leftHand",
        "rightShoulder", "rightUpperArm", "rightLowerArm", "rightHand",
        "leftUpperLeg", "leftLowerLeg", "leftFoot",
        "rightUpperLeg", "rightLowerLeg", "rightFoot",
    ]

    /// VRMA spec preset list (14 entries) matching three-vrm. `lookUp`/
    /// `lookDown`/`lookLeft`/`lookRight` are intentionally omitted — they
    /// are LookAt-driven and surface via `dump_look_at_state`, not via
    /// `dump_expression_weights` (per the ops-doc note in
    /// `crates/vrm-ops/src/tools.rs`).
    private static let referencePresetExpressions: [String] = [
        "happy", "angry", "sad", "relaxed", "surprised",
        "aa", "ih", "ou", "ee", "oh",
        "blink", "blinkLeft", "blinkRight", "neutral",
    ]

    private func handleLoadVrma(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let path) = obj["vrma_path"]
        else {
            return invalidParams("missing vrma_path")
        }
        guard FileManager.default.fileExists(atPath: path) else {
            return loadFailed("vrma file not found: \(path)")
        }

        // Retargeting needs a model — find the most recently created
        // session (the conformance runner loads exactly one model before
        // calling load_vrma).
        stateLock.lock()
        let session = sessions.values.first
        stateLock.unlock()
        guard let session = session else {
            return loadFailed("load_vrma called before any load_vrm — no model available for retargeting")
        }

        let url = URL(fileURLWithPath: path)
        let clip: AnimationClip
        do {
            clip = try VRMAnimationLoader.loadVRMA(from: url, model: session.model)
        } catch {
            return loadFailed("VRMAnimationLoader.loadVRMA failed: \(error)")
        }

        let humanoidBones = UInt32(clip.jointTracks.count)
        // Count unique expression names across morph and typed expression
        // tracks (the loader emits both for recognised presets; dedupe).
        var expressionNames = Set<String>()
        for t in clip.morphTracks { expressionNames.insert(t.key) }
        for t in clip.expressionTracks { expressionNames.insert(t.expression.rawValue) }
        let expressions = UInt32(expressionNames.count)
        let hasLookAt = clip.lookAtTargetSampler != nil

        stateLock.lock()
        let handle = nextVrmaHandle
        nextVrmaHandle &+= 1
        vrmaClips[handle] = clip
        stateLock.unlock()

        return .ok(.object([
            "vrma_handle": .number(Double(handle)),
            "channel_summary": .object([
                "humanoid_bones": .number(Double(humanoidBones)),
                "expressions": .number(Double(expressions)),
                "has_look_at": .bool(hasLookAt),
                "duration_seconds": .number(Double(clip.duration)),
            ]),
        ]))
    }

    private func handleApplyVrmaAtTime(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let sessionId) = obj["session_id"]
        else {
            return invalidParams("missing session_id")
        }
        guard case .number(let handleD) = obj["vrma_handle"],
              let handle = UInt32(exactly: handleD)
        else {
            return invalidParams("missing or malformed vrma_handle")
        }
        guard let time = parseFloat(obj["time_seconds"]) else {
            return invalidParams("missing time_seconds")
        }
        guard let session = lookupSession(sessionId) else {
            return invalidParams("unknown session_id: \(sessionId)")
        }
        stateLock.lock()
        let clip = vrmaClips[handle]
        stateLock.unlock()
        guard let clip = clip else {
            return invalidParams("unknown vrma_handle: \(handle)")
        }

        // Humanoid: write bone rotations and (hips only, per VRMA spec)
        // translation. The retargeting closures returned by the loader
        // already bake the rest-pose-to-rest-pose transform per
        // `VRMAnimationLoader.makeRotationSampler` (W_A·L_A⁻¹·A·W_A⁻¹ →
        // L_B·W_B⁻¹·Normalised·W_B).
        var humanoidBonesApplied: UInt32 = 0
        for track in clip.jointTracks {
            let (rot, trans, _) = track.sample(at: time)
            if let rot = rot {
                session.model.setLocalRotation(rot, for: track.bone)
                humanoidBonesApplied &+= 1
            }
            if let trans = trans, track.bone == .hips {
                session.model.setHipsTranslation(trans)
            }
        }

        // Expressions: prefer the typed `expressionTracks` for recognised
        // presets so we go through `setExpressionWeight(_:weight:)`'s preset
        // group-suppression logic. Fall through to `morphTracks` for the
        // custom and the (rare) unknown-preset case.
        var expressionsApplied: UInt32 = 0
        var presetNames = Set<String>()
        if let controller = session.renderer.expressionController {
            for track in clip.expressionTracks {
                controller.setExpressionWeight(track.expression, weight: track.sample(at: time))
                presetNames.insert(track.expression.rawValue)
                expressionsApplied &+= 1
            }
            for track in clip.morphTracks where !presetNames.contains(track.key) {
                let w = track.sample(at: time)
                if let preset = VRMExpressionPreset(rawValue: track.key) {
                    controller.setExpressionWeight(preset, weight: w)
                } else {
                    controller.setCustomExpressionWeight(track.key, weight: w)
                }
                expressionsApplied &+= 1
            }
        }

        // LookAt: record the head-local target point so dump_look_at_state
        // can derive yaw/pitch deterministically (no controller smoothing
        // contamination). Also push the target into the renderer's
        // VRMLookAtController so a downstream render reflects the gaze.
        var lookAtApplied = false
        if let sampler = clip.lookAtTargetSampler {
            let point = sampler(time)
            session.lastLookAtHeadLocalPoint = point
            session.renderer.lookAtController?.target = .headLocalPoint(point)
            lookAtApplied = true
        } else {
            session.lastLookAtHeadLocalPoint = nil
        }

        return .ok(.object([
            "channels_applied": .object([
                "humanoid_bones": .number(Double(humanoidBonesApplied)),
                "expressions": .number(Double(expressionsApplied)),
                "look_at": .bool(lookAtApplied),
            ]),
        ]))
    }

    private func handleDumpHumanoidPose(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let sessionId) = obj["session_id"]
        else {
            return invalidParams("missing session_id")
        }
        guard let session = lookupSession(sessionId) else {
            return invalidParams("unknown session_id: \(sessionId)")
        }

        var bonesArr: [JSONValue] = []
        var missingArr: [JSONValue] = []
        for boneName in Operations.referenceHumanoidBones {
            guard let bone = VRMHumanoidBone(rawValue: boneName) else {
                missingArr.append(.string(boneName))
                continue
            }
            guard let rot = session.model.getLocalRotation(for: bone) else {
                missingArr.append(.string(boneName))
                continue
            }
            let v = rot.vector  // simd_quatf exposes (x, y, z, w) via .vector
            bonesArr.append(.object([
                "name": .string(boneName),
                "local_rotation_quat": .array([
                    .number(Double(v.x)),
                    .number(Double(v.y)),
                    .number(Double(v.z)),
                    .number(Double(v.w)),
                ]),
            ]))
        }
        let hips = session.model.getHipsTranslation() ?? SIMD3<Float>(0, 0, 0)
        return .ok(.object([
            "bones": .array(bonesArr),
            "hips_translation": .array([
                .number(Double(hips.x)),
                .number(Double(hips.y)),
                .number(Double(hips.z)),
            ]),
            "bones_missing": .array(missingArr),
        ]))
    }

    private func handleDumpExpressionWeights(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let sessionId) = obj["session_id"]
        else {
            return invalidParams("missing session_id")
        }
        guard let session = lookupSession(sessionId) else {
            return invalidParams("unknown session_id: \(sessionId)")
        }
        let controller = session.renderer.expressionController
        // Sorted-key emit so the JSON output is deterministic and matches
        // the Rust-side BTreeMap ordering expected by `pose_diff`.
        var presets: [String: JSONValue] = [:]
        for presetName in Operations.referencePresetExpressions {
            let value: Float
            if let controller = controller, let preset = VRMExpressionPreset(rawValue: presetName) {
                value = controller.weight(for: preset)
            } else {
                value = 0.0
            }
            presets[presetName] = .number(Double(value))
        }
        var custom: [String: JSONValue] = [:]
        if let controller = controller {
            // VRMExpressionController has no public custom-name iterator;
            // enumerate via the model's expressions.custom registry
            // (populated at model load time from the .vrm).
            let customNames = session.model.expressions?.custom.keys.sorted() ?? []
            for name in customNames {
                let w = controller.weight(forCustom: name) ?? 0.0
                custom[name] = .number(Double(w))
            }
        }
        return .ok(.object([
            "presets": .object(presets),
            "custom": .object(custom),
        ]))
    }

    private func handleDumpLookAtState(params: JSONValue?) -> OpOutcome {
        guard case .object(let obj) = params,
              case .string(let sessionId) = obj["session_id"]
        else {
            return invalidParams("missing session_id")
        }
        guard let session = lookupSession(sessionId) else {
            return invalidParams("unknown session_id: \(sessionId)")
        }

        // Derive yaw/pitch from the last-applied head-local target point.
        // glTF/VRM forward is -Z in head-local space, Y up, X right.
        //   yaw   = rotation around Y (positive ⇒ avatar looks toward +X)
        //   pitch = rotation around X (positive ⇒ avatar looks upward)
        // Quaternion composes extrinsic ZXY per VRM 1.0 lookAt spec.
        var yawDeg: Float = 0
        var pitchDeg: Float = 0
        if let p = session.lastLookAtHeadLocalPoint {
            let len = simd_length(p)
            if len > 1e-6 {
                let d = p / len
                let pitchRad = asinf(max(-1.0, min(1.0, d.y)))
                let yawRad = atan2f(d.x, -d.z)
                pitchDeg = pitchRad * 180.0 / .pi
                yawDeg = yawRad * 180.0 / .pi
            }
        }
        // q = R_y(yaw) * R_x(pitch)  (extrinsic ZXY with roll=0 = YXZ-applied)
        let qYaw = simd_quatf(angle: yawDeg * .pi / 180.0, axis: SIMD3<Float>(0, 1, 0))
        let qPitch = simd_quatf(angle: pitchDeg * .pi / 180.0, axis: SIMD3<Float>(1, 0, 0))
        let q = (qYaw * qPitch).vector

        let appliedVia: String
        switch session.model.lookAt?.type {
        case .bone:
            appliedVia = "bone"
        case .expression:
            appliedVia = "expression"
        case .none:
            appliedVia = "off"
        }
        let offset = session.model.lookAt?.offsetFromHeadBone ?? SIMD3<Float>(0, 0, 0)

        return .ok(.object([
            "gaze_direction_quat": .array([
                .number(Double(q.x)),
                .number(Double(q.y)),
                .number(Double(q.z)),
                .number(Double(q.w)),
            ]),
            "yaw_deg": .number(Double(yawDeg)),
            "pitch_deg": .number(Double(pitchDeg)),
            "applied_via": .string(appliedVia),
            "offset_from_head_bone": .array([
                .number(Double(offset.x)),
                .number(Double(offset.y)),
                .number(Double(offset.z)),
            ]),
        ]))
    }

    // MARK: - Helpers

    private func lookupSession(_ id: String) -> Session? {
        stateLock.lock()
        defer { stateLock.unlock() }
        return sessions[id]
    }

    private func parseVec3(_ v: JSONValue?) -> SIMD3<Float>? {
        guard case .array(let a) = v, a.count == 3 else { return nil }
        var out: [Float] = []
        for x in a {
            guard case .number(let n) = x else { return nil }
            out.append(Float(n))
        }
        return SIMD3<Float>(out[0], out[1], out[2])
    }

    private func parseFloat(_ v: JSONValue?) -> Float? {
        if case .number(let n) = v { return Float(n) }
        return nil
    }

    /// Sync wrapper around `VRMModel.load`'s async API. The blocking is a
    /// deliberate choice: the JSON-RPC server is single-threaded, sessions
    /// load one at a time, and the call site needs the result before it
    /// can return.
    private func blockingLoad(url: URL, device: MTLDevice) -> Result<VRMModel, Error> {
        let box = ResultBox()
        let sem = DispatchSemaphore(value: 0)
        Task {
            do {
                let model = try await VRMModel.load(from: url, device: device)
                box.value = .success(model)
            } catch {
                box.value = .failure(error)
            }
            sem.signal()
        }
        sem.wait()
        return box.value!
    }

    private final class ResultBox: @unchecked Sendable {
        var value: Result<VRMModel, Error>?
    }

    // MARK: - PNG export

    /// Read RGBA bytes out of an MTLTexture and write a PNG to `path`.
    /// Adapted from VRMRender's `exportTexture` so the conformance adapter
    /// and the reference renderer produce identical PNG encodings.
    private func writeTexturePng(_ texture: MTLTexture, to path: String) throws {
        let width = texture.width
        let height = texture.height
        let bytesPerPixel = 4
        let bytesPerRow = width * bytesPerPixel
        let bytesPerImage = height * bytesPerRow

        var pixelData = Data(count: bytesPerImage)
        pixelData.withUnsafeMutableBytes { rawBuffer in
            guard let pointer = rawBuffer.baseAddress else { return }
            texture.getBytes(
                pointer,
                bytesPerRow: bytesPerRow,
                from: MTLRegionMake2D(0, 0, width, height),
                mipmapLevel: 0
            )
        }

        guard let provider = CGDataProvider(data: pixelData as CFData) else {
            throw RenderExportError.cgDataProviderFailed
        }
        let bitmapInfo: CGBitmapInfo
        switch texture.pixelFormat {
        case .rgba8Unorm, .rgba8Unorm_srgb:
            bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue)
        case .bgra8Unorm, .bgra8Unorm_srgb:
            bitmapInfo = CGBitmapInfo(
                rawValue: CGImageAlphaInfo.premultipliedLast.rawValue
                    | CGBitmapInfo.byteOrder32Little.rawValue
            )
        default:
            bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue)
        }

        guard let image = CGImage(
            width: width,
            height: height,
            bitsPerComponent: 8,
            bitsPerPixel: 32,
            bytesPerRow: bytesPerRow,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: bitmapInfo,
            provider: provider,
            decode: nil,
            shouldInterpolate: false,
            intent: .defaultIntent
        ) else {
            throw RenderExportError.cgImageFailed
        }

        let url = URL(fileURLWithPath: path)
        // Best-effort: create parent dirs if missing so callers don't have to.
        try? FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )

        guard let destination = CGImageDestinationCreateWithURL(
            url as CFURL,
            UTType.png.identifier as CFString,
            1, nil
        ) else {
            throw RenderExportError.destinationFailed
        }
        CGImageDestinationAddImage(destination, image, nil)
        if !CGImageDestinationFinalize(destination) {
            throw RenderExportError.finalizeFailed
        }
    }

    private enum RenderExportError: Error, CustomStringConvertible {
        case cgDataProviderFailed
        case cgImageFailed
        case destinationFailed
        case finalizeFailed
        var description: String {
            switch self {
            case .cgDataProviderFailed: return "CGDataProvider init failed"
            case .cgImageFailed:        return "CGImage init failed"
            case .destinationFailed:    return "CGImageDestination create failed"
            case .finalizeFailed:       return "CGImageDestination finalize failed"
            }
        }
    }

    private func loadFailed(_ reason: String) -> OpOutcome {
        .error(
            code: -32001,
            message: "LoadFailed",
            data: .object(["reason": .string(reason)])
        )
    }

    private func renderFailed(_ reason: String) -> OpOutcome {
        .error(
            code: -32002,
            message: "RenderFailed",
            data: .object(["reason": .string(reason)])
        )
    }

    private func invalidParams(_ reason: String) -> OpOutcome {
        .error(
            code: -32602,
            message: "Invalid params: \(reason)",
            data: nil
        )
    }
}
