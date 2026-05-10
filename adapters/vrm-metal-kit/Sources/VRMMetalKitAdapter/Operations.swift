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
    /// Phase labels match `docs/operation-contract.md`. Physics ops
    /// (`step_physics`, `reset_physics`, `animate_root_transform`) are
    /// implemented in three-vrm and the mock but stay deferred here until
    /// the L3-e spring-bone integration lands.
    static let reservedPhases: [String: String] = [
        "set_environment":         "v1.x",
        "set_expression":          "Phase 3",
        "set_humanoid_pose":       "Phase 2",
        "set_root_transform":      "Phase 2",
        "animate_root_transform":  "L3 (VRMMetalKit integration deferred)",
        "step_physics":            "L3 (VRMMetalKit integration deferred)",
        "reset_physics":           "L3 (VRMMetalKit integration deferred)",
    ]

    /// Phase label for the still-deferred Phase 2 ops.
    private static let l3Deferral = "L3 (VRMMetalKit integration deferred)"

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

        init(renderer: VRMRenderer, model: VRMModel) {
            self.renderer = renderer
            self.model = model
        }
    }

    private let device: MTLDevice?
    private let commandQueue: MTLCommandQueue?
    private var sessions: [String: Session] = [:]
    private var sessionCounter: Int = 0
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
        case "load_vrm":             return handleLoadVrm(params: params)
        case "set_camera":           return handleSetCamera(params: params)
        case "set_lighting":         return handleSetLighting(params: params)
        case "set_post_processing":  return handleSetPostProcessing(params: params)
        case "render":               return handleRender(params: params)
        case "dispose":              return handleDispose(params: params)
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
            var config = RendererConfig()
            config.sampleCount = 1     // single-sample for L3-c; MSAA later
            config.strict = .off
            let renderer = VRMRenderer(device: device, config: config)
            renderer.loadModel(model)

            stateLock.lock()
            sessionCounter += 1
            let id = "vrm-metal-kit-\(sessionCounter)"
            sessions[id] = Session(renderer: renderer, model: model)
            stateLock.unlock()
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
        // The operation contract's `directional.dir` is the direction the
        // light *travels* (light → scene). VRMMetalKit's `setLight` takes
        // the direction *toward* the light (scene → light); negate.
        if let dir = session.directionalDir,
           let color = session.directionalColor,
           let intensity = session.directionalIntensity {
            session.renderer.setLight(0, direction: -dir, color: color, intensity: intensity)
            session.renderer.disableLight(1)
            session.renderer.disableLight(2)
        }
        if let ambColor = session.ambientColor,
           let ambIntensity = session.ambientIntensity {
            session.renderer.setAmbientColor(ambColor * ambIntensity)
        }

        // ----- render targets -----
        // Magenta clear color: matches the mock-renderer + three-vrm
        // sentinel so the diff engine's bbox-relative property assertions
        // can find the avatar against a known background.
        let colorPixelFormat: MTLPixelFormat = (colorSpace == "Srgb") ? .rgba8Unorm_srgb : .rgba8Unorm
        let colorDesc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: colorPixelFormat,
            width: width, height: height, mipmapped: false
        )
        colorDesc.usage = [.renderTarget, .shaderRead]
        colorDesc.storageMode = .shared
        guard let colorTex = device.makeTexture(descriptor: colorDesc) else {
            return renderFailed("failed to create color texture (\(width)×\(height))")
        }

        let depthDesc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .depth32Float,
            width: width, height: height, mipmapped: false
        )
        depthDesc.usage = .renderTarget
        depthDesc.storageMode = .shared
        guard let depthTex = device.makeTexture(descriptor: depthDesc) else {
            return renderFailed("failed to create depth texture")
        }

        let rpd = MTLRenderPassDescriptor()
        rpd.colorAttachments[0].texture = colorTex
        rpd.colorAttachments[0].loadAction = .clear
        rpd.colorAttachments[0].storeAction = .store
        rpd.colorAttachments[0].clearColor = MTLClearColor(red: 1.0, green: 0.0, blue: 1.0, alpha: 1.0)
        rpd.depthAttachment.texture = depthTex
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
            session.renderer.drawOffscreenHeadless(
                to: colorTex,
                depth: depthTex,
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
        do {
            try writeTexturePng(colorTex, to: outputPath)
        } catch {
            return renderFailed("PNG export failed: \(error)")
        }

        return .ok(.object([
            "output_path": .string(outputPath),
            "actual_color_space": .string(colorSpace),
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
