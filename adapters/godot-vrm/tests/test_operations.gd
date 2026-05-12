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

func test_animate_root_transform_returns_phase_2() -> void:
    var r: Dictionary = await Operations.dispatch(null, null, 9, "animate_root_transform", {})
    _assert_eq(r.get("error", {}).get("data", {}).get("phase"), "Phase 2", "phase label")

func test_set_environment_returns_v1x() -> void:
    var r: Dictionary = await Operations.dispatch(null, null, 4, "set_environment", {})
    _assert_eq(r.get("error", {}).get("data", {}).get("phase"), "v1.x", "phase label")

func test_set_expression_returns_phase_3() -> void:
    var r: Dictionary = await Operations.dispatch(null, null, 5, "set_expression", {})
    _assert_eq(r.get("error", {}).get("data", {}).get("phase"), "Phase 3", "phase label")

func test_id_is_echoed_on_unknown_method() -> void:
    var r1: Dictionary = await Operations.dispatch(null, null, "abc-123", "definitely_not_a_method", {})
    _assert_eq(r1.get("id"), "abc-123", "string id echoed on -32601 path")
    var r2: Dictionary = await Operations.dispatch(null, null, null, "definitely_not_a_method", {})
    _assert_eq(r2.get("id"), null, "null id echoed on -32601 path")
