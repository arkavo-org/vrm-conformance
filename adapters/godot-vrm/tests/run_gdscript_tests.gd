extends SceneTree

const TESTS_DIR := "res://tests/"

var _passed := 0
var _failed := 0
var _failures: Array[String] = []

func _init() -> void:
    var dir := DirAccess.open(TESTS_DIR)
    if dir == null:
        push_error("cannot open " + TESTS_DIR); quit(2); return
    dir.list_dir_begin()
    var names: Array[String] = []
    while true:
        var name := dir.get_next()
        if name == "": break
        if name.begins_with("test_") and name.ends_with(".gd"):
            names.append(name)
    names.sort()
    for name in names:
        _run_file(TESTS_DIR + name)
    print("\n%d passed, %d failed" % [_passed, _failed])
    for f in _failures:
        print("  FAIL: " + f)
    quit(0 if _failed == 0 else 1)

func _run_file(path: String) -> void:
    var script: GDScript = load(path)
    if script == null:
        _failed += 1; _failures.append(path + " (load failed)"); return
    var inst: Object = script.new()
    for m in inst.get_method_list():
        var mname: String = m["name"]
        if not mname.begins_with("test_"): continue
        inst.set("_test_failure", "")
        inst.call(mname)
        var captured: String = inst.get("_test_failure")
        if captured == "":
            _passed += 1
        else:
            _failed += 1
            _failures.append("%s::%s — %s" % [path, mname, captured])
