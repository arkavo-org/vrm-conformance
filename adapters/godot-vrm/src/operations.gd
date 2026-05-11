# Operation registry + dispatch for the godot-vrm adapter.
#
# L1 + L2 state: every Phase 1 op and every reserved op returns a structured
# `Unimplemented` error. L3 replaces the Phase 1 entries with real
# implementations driven by V-Sekai/godot-vrm + a hidden SubViewport.

class_name Operations

const PHASE_BY_METHOD := {
    "load_vrm": "L3 (godot-vrm integration deferred)",
    "set_camera": "L3 (godot-vrm integration deferred)",
    "set_lighting": "L3 (godot-vrm integration deferred)",
    "set_post_processing": "L3 (godot-vrm integration deferred)",
    "render": "L3 (godot-vrm integration deferred)",
    "dispose": "L3 (godot-vrm integration deferred)",
    "set_environment": "v1.x",
    "set_expression": "Phase 3",
    "set_humanoid_pose": "Phase 2",
    "set_root_transform": "Phase 2",
    "animate_root_transform": "Phase 2",
    "step_physics": "Phase 2",
    "reset_physics": "Phase 2",
}

# Returns the JSON-RPC response dict for one request. `id` is forwarded
# from the request unchanged. Unknown methods return -32601.
static func dispatch(id: Variant, method: String, _params: Variant) -> Dictionary:
    if PHASE_BY_METHOD.has(method):
        return {
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32000,
                "message": "Unimplemented",
                "data": { "phase": PHASE_BY_METHOD[method] },
            },
        }
    return {
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": -32601,
            "message": "method not found: " + method,
        },
    }
