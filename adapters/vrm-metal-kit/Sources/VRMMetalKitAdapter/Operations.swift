// Operation dispatcher.
//
// Maps method names to op handlers. Through L2 every Phase 1 op returned a
// structured `Unimplemented` error; L3-b promotes `load_vrm` and `dispose`
// to real handlers that drive VRMMetalKit. Other Phase 1 ops still return
// Unimplemented with the L3-deferral phase label until their respective
// L3 step lands. Unknown methods return `-32601`.
//
// Per the operation contract (`docs/operation-contract.md`):
//   -32000 Unimplemented   — declared but not implemented in this version
//   -32001 LoadFailed      — `.vrm` failed to load
//   -32601 method-not-found
//   -32602 invalid params

import Foundation
@preconcurrency import Metal
import VRMMetalKit

/// Stateful op state: holds the Metal device that survives the adapter
/// lifetime and the per-session registry. A single `Operations.shared`
/// instance backs the default dispatcher; tests can construct fresh
/// instances for isolation.
final class Operations: @unchecked Sendable {
    /// Default singleton wired into `JsonRpcServer`'s default dispatcher.
    static let shared = Operations()

    /// Static phase tables — preserved as static so tests can introspect
    /// the declaration without instantiating the class. L3-b changes the
    /// runtime dispatch for `load_vrm` / `dispose` even though those still
    /// appear in `phaseOneMethods` (they're still "Phase 1 contract ops",
    /// just no longer L3-deferred).
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
    /// the L3 spring-bone integration lands.
    static let reservedPhases: [String: String] = [
        "set_environment":         "v1.x",
        "set_expression":          "Phase 3",
        "set_humanoid_pose":       "Phase 2",
        "set_root_transform":      "Phase 2",
        "animate_root_transform":  "L3 (VRMMetalKit integration deferred)",
        "step_physics":            "L3 (VRMMetalKit integration deferred)",
        "reset_physics":           "L3 (VRMMetalKit integration deferred)",
    ]

    /// Phase label for Phase 1 ops that L3-b hasn't yet wired up. Distinct
    /// from the v1.x / Phase 2 / Phase 3 reservations so callers can tell
    /// "spec-deferred" from "this-adapter-deferred."
    private static let l3Deferral = "L3 (VRMMetalKit integration deferred)"

    /// Per-session state. Keyed by the session_id returned from `load_vrm`.
    private struct Session {
        let model: VRMModel
        // L3-c adds camera matrices, render target descriptors, etc.
    }

    private let device: MTLDevice?
    private var sessions: [String: Session] = [:]
    private var sessionCounter: Int = 0
    private let stateLock = NSLock()

    init() {
        // Eager Metal device acquisition: if the host has no GPU (shouldn't
        // happen on macOS, but cheap to guard) load_vrm will refuse with a
        // structured LoadFailed. Tests that only exercise error paths don't
        // touch the device, so a nil device is fine for them too.
        self.device = MTLCreateSystemDefaultDevice()
    }

    /// Static-flavored phase lookup, used by `JsonRpcServerTests` to assert
    /// what phase label a given method declaration claims. Real dispatch
    /// happens via `dispatch(method:params:)`.
    static func phase(for method: String) -> String? {
        if phaseOneMethods.contains(method) {
            return l3Deferral
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
        case "load_vrm":
            return handleLoadVrm(params: params)
        case "dispose":
            return handleDispose(params: params)
        default:
            if let phase = Operations.phase(for: method) {
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

    // MARK: - Handlers

    /// Load a `.vrm` file off disk and stash the resulting `VRMModel` in
    /// the session registry under a fresh `session_id`. Bridges sync→async
    /// via `DispatchSemaphore` since the JSON-RPC dispatcher is sync but
    /// `VRMModel.load(from:device:)` is async.
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
        let outcome = blockingLoad(url: url, device: device)
        switch outcome {
        case .failure(let err):
            return loadFailed("VRMModel.load failed: \(err)")
        case .success(let model):
            stateLock.lock()
            sessionCounter += 1
            let id = "vrm-metal-kit-\(sessionCounter)"
            sessions[id] = Session(model: model)
            stateLock.unlock()
            return .ok(.object(["session_id": .string(id)]))
        }
    }

    /// Remove a session from the registry. Idempotent — disposing an
    /// unknown id still returns ok, matching the mock-renderer's
    /// contract and the three-vrm adapter.
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

    // MARK: - Helpers

    /// Sync wrapper around `VRMModel.load`'s async API. The blocking is a
    /// deliberate choice: the JSON-RPC server is single-threaded, sessions
    /// load one at a time, and the call site needs the result before it
    /// can return.
    ///
    /// The result is boxed in a `@unchecked Sendable` reference type so
    /// Swift 6 strict concurrency doesn't reject the cross-Task write +
    /// post-semaphore read. The semaphore is the actual synchronization
    /// primitive: `sem.signal()` happens-before `sem.wait()` returns, so
    /// the write is visible. The compiler just can't prove it.
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

    private func loadFailed(_ reason: String) -> OpOutcome {
        .error(
            code: -32001,
            message: "LoadFailed",
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
