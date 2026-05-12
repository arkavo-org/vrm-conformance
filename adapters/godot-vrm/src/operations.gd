# Operation registry + dispatch for the godot-vrm adapter.
#
# L3 state: Phase 1 ops (load_vrm, set_camera, set_lighting,
# set_post_processing, render, dispose) dispatch to Session. Reserved ops
# return the standard -32000 Unimplemented with phase labels per
# docs/operation-contract.md.

class_name Operations

const Session := preload("res://src/session.gd")

const PHASE_BY_RESERVED_METHOD := {
    "set_environment": "v1.x",
    "set_expression": "Phase 3",
    "set_humanoid_pose": "Phase 2",
    "set_root_transform": "Phase 2",
}

# Phase 1 method names. dispatch() routes these to Session.<name>.
const PHASE1_METHODS := [
    "load_vrm", "set_camera", "set_lighting",
    "set_post_processing", "render", "dispose",
    "step_physics", "reset_physics", "animate_root_transform",
]

# Async to support `render` which awaits frames.
static func dispatch(tree: SceneTree, session: Session, id: Variant, method: String, params: Variant) -> Dictionary:
    if PHASE1_METHODS.has(method):
        var outcome: Dictionary
        match method:
            "load_vrm":
                outcome = session.load_vrm(tree.root, params if typeof(params) == TYPE_DICTIONARY else {})
            "set_camera":
                outcome = session.set_camera(params if typeof(params) == TYPE_DICTIONARY else {})
            "set_lighting":
                outcome = session.set_lighting(params if typeof(params) == TYPE_DICTIONARY else {})
            "set_post_processing":
                outcome = session.set_post_processing(params if typeof(params) == TYPE_DICTIONARY else {})
            "render":
                outcome = await session.render(tree, params if typeof(params) == TYPE_DICTIONARY else {})
            "dispose":
                outcome = session.dispose(params if typeof(params) == TYPE_DICTIONARY else {})
            "step_physics":
                outcome = session.step_physics(params if typeof(params) == TYPE_DICTIONARY else {})
            "reset_physics":
                outcome = session.reset_physics(params if typeof(params) == TYPE_DICTIONARY else {})
            "animate_root_transform":
                outcome = session.animate_root_transform(params if typeof(params) == TYPE_DICTIONARY else {})
            _:
                outcome = { "ok": false, "error": { "code": -32601, "message": "internal: PHASE1 method not routed: " + method } }
        if outcome.get("ok"):
            return { "jsonrpc": "2.0", "id": id, "result": outcome.get("result", {}) }
        return { "jsonrpc": "2.0", "id": id, "error": outcome.get("error") }

    if PHASE_BY_RESERVED_METHOD.has(method):
        return {
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32000,
                "message": "Unimplemented",
                "data": { "phase": PHASE_BY_RESERVED_METHOD[method] },
            },
        }

    return {
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": -32601, "message": "method not found: " + method },
    }
