extends RefCounted

const Operations := preload("res://src/operations.gd")

var _test_failure: String = ""

func _fail(msg: String) -> void:
    if _test_failure == "":
        _test_failure = msg

func _assert_eq(actual, expected, label: String) -> void:
    if actual != expected:
        _fail("%s: expected %s, got %s" % [label, str(expected), str(actual)])

func test_unknown_method_returns_minus_32601() -> void:
    var r: Dictionary = await Operations.dispatch(null, null, 7, "definitely_not_a_method", {})
    _assert_eq(r.get("id"), 7, "id echoed")
    _assert_eq(r.get("error", {}).get("code"), -32601, "error code")

func test_set_humanoid_pose_returns_phase_2() -> void:
    var r: Dictionary = await Operations.dispatch(null, null, 3, "set_humanoid_pose", {})
    _assert_eq(r.get("error", {}).get("data", {}).get("phase"), "Phase 2", "phase label")

func test_set_root_transform_returns_phase_2() -> void:
    var r: Dictionary = await Operations.dispatch(null, null, 8, "set_root_transform", {})
    _assert_eq(r.get("error", {}).get("data", {}).get("phase"), "Phase 2", "phase label")

func test_phase1_methods_include_physics_ops() -> void:
    var has_step: bool = Operations.PHASE1_METHODS.has("step_physics")
    var has_reset: bool = Operations.PHASE1_METHODS.has("reset_physics")
    var has_animate: bool = Operations.PHASE1_METHODS.has("animate_root_transform")
    _assert_eq(has_step, true, "step_physics in PHASE1")
    _assert_eq(has_reset, true, "reset_physics in PHASE1")
    _assert_eq(has_animate, true, "animate_root_transform in PHASE1")

func test_phase1_methods_include_dump_bone_positions() -> void:
    var has_dump: bool = Operations.PHASE1_METHODS.has("dump_bone_positions")
    _assert_eq(has_dump, true, "dump_bone_positions in PHASE1")

func test_set_environment_returns_v1x() -> void:
    var r: Dictionary = await Operations.dispatch(null, null, 4, "set_environment", {})
    _assert_eq(r.get("error", {}).get("data", {}).get("phase"), "v1.x", "phase label")

func test_set_expression_returns_phase_3() -> void:
    var r: Dictionary = await Operations.dispatch(null, null, 5, "set_expression", {})
    _assert_eq(r.get("error", {}).get("data", {}).get("phase"), "Phase 3", "phase label")

func test_vrma_ops_return_vrma_v1() -> void:
    var methods: Array = [
        "load_vrma",
        "apply_vrma_at_time",
        "dump_humanoid_pose",
        "dump_expression_weights",
        "dump_look_at_state",
    ]
    for method in methods:
        var r: Dictionary = await Operations.dispatch(null, null, 100, method, {})
        _assert_eq(r.get("error", {}).get("code"), -32000, "%s code" % method)
        _assert_eq(r.get("error", {}).get("data", {}).get("phase"), "vrma-v1", "%s phase" % method)

func test_id_is_echoed_on_unknown_method() -> void:
    var r1: Dictionary = await Operations.dispatch(null, null, "abc-123", "definitely_not_a_method", {})
    _assert_eq(r1.get("id"), "abc-123", "string id echoed on -32601 path")
    var r2: Dictionary = await Operations.dispatch(null, null, null, "definitely_not_a_method", {})
    _assert_eq(r2.get("id"), null, "null id echoed on -32601 path")
