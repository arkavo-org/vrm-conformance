// Operation dispatcher.
//
// Maps method names to op handlers. In L2 every Phase 1 op and every
// reserved op returns a structured `Unimplemented` error (JSON-RPC code
// `-32000`) with `data: { "phase": <label> }`. Unknown methods return
// `-32601` (Method not found), per the operation contract.
//
// L3 replaces the Phase 1 entries below with real VRMMetalKit-backed
// implementations; reserved ops keep returning Unimplemented until their
// respective phases.

import Foundation

enum Operations {
    /// Phase 1 ops (required by the contract; unimplemented in this scaffolding pass).
    static let phaseOneMethods: Set<String> = [
        "load_vrm",
        "set_camera",
        "set_lighting",
        "set_post_processing",
        "render",
        "dispose",
    ]

    /// Reserved ops — must be declared by every adapter, return Unimplemented in v0.1.
    /// Phase labels match `docs/operation-contract.md`.
    static let reservedPhases: [String: String] = [
        "set_environment":         "v1.x",
        "set_expression":          "Phase 3",
        "set_humanoid_pose":       "Phase 2",
        "set_root_transform":      "Phase 2",
        "animate_root_transform":  "Phase 2",
        "step_physics":            "Phase 2",
        "reset_physics":           "Phase 2",
    ]

    /// Returns the Unimplemented `phase` label for any known op, or nil for unknown.
    static func phase(for method: String) -> String? {
        if phaseOneMethods.contains(method) {
            // Phase 1 ops are not yet implemented in this scaffolding pass.
            // L3 replaces these with real handlers. The label is distinct from
            // the contract-reserved "v1.x" tag (which means HDRI / future spec
            // work) so consumers can tell the two apart.
            return "L3 (VRMMetalKit integration deferred)"
        }
        if let reserved = reservedPhases[method] {
            return reserved
        }
        return nil
    }

    /// Top-level dispatch entry. Wired into `JsonRpcServer` by default.
    static func dispatch(method: String, params: JSONValue?) -> OpOutcome {
        _ = params  // L3 reads params; L2 ignores them.

        if let phase = phase(for: method) {
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
