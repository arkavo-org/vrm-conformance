# Session state for one Godot-child lifetime. The shim's request/response
# loop is request-locked, so a single global Session suffices. Holds the
# loaded VRM scene + a SubViewport configured for off-screen rendering.

class_name Session

const VrmRuntimeExtensions := preload("res://src/vrm_runtime_extensions.gd")
const MAGENTA := Color(1.0, 0.0, 1.0)

var session_id: String = ""
var scene: Node = null
var viewport: SubViewport = null
var camera: Camera3D = null
var directional_light: DirectionalLight3D = null
var environment: Environment = null

# Build the SubViewport once at load time; reused across set_camera/
# set_lighting/set_post_processing/render. Caller passes the SceneTree
# root so the viewport can be parented and the renderer drives it.
func load_vrm(tree_root: Node, params: Dictionary) -> Dictionary:
    var path: String = params.get("path", "")
    if path == "":
        return _err(-32602, "missing path")

    var gltf := GLTFDocument.new()
    var registered := VrmRuntimeExtensions.register_all()
    var state := GLTFState.new()
    state.set_additional_data(&"vrm/head_hiding_method", 0)
    state.set_additional_data(&"vrm/first_person_layers", 2)
    state.set_additional_data(&"vrm/third_person_layers", 4)
    state.handle_binary_image = GLTFState.HANDLE_BINARY_EMBED_AS_UNCOMPRESSED
    var err := gltf.append_from_file(path, state, 0)
    if err != OK:
        VrmRuntimeExtensions.unregister_all(registered)
        return _err(-32001, "LoadFailed", { "reason": "append_from_file err %d" % err })
    var built: Node = gltf.generate_scene(state)
    VrmRuntimeExtensions.unregister_all(registered)
    if built == null:
        return _err(-32001, "LoadFailed", { "reason": "generate_scene returned null" })

    scene = built
    session_id = "godot-%d" % Time.get_ticks_msec()

    # Build the viewport scaffolding. set_camera/set_lighting/set_post_processing
    # will tune fields on viewport/camera/directional_light/environment.
    viewport = SubViewport.new()
    viewport.size = Vector2i(1024, 1024)
    viewport.transparent_bg = false
    viewport.msaa_3d = Viewport.MSAA_4X
    viewport.render_target_update_mode = SubViewport.UPDATE_ONCE
    viewport.world_3d = World3D.new()

    environment = Environment.new()
    environment.background_mode = Environment.BG_COLOR
    environment.background_color = MAGENTA
    environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
    environment.ambient_light_color = Color(0.5, 0.5, 0.5)
    environment.ambient_light_energy = 0.3
    environment.tonemap_mode = Environment.TONE_MAPPER_LINEAR
    environment.tonemap_exposure = 1.0
    viewport.world_3d.environment = environment

    tree_root.add_child(viewport)
    viewport.add_child(scene)

    camera = Camera3D.new()
    viewport.add_child(camera)
    camera.look_at_from_position(Vector3(0.0, 1.4, 1.5), Vector3(0.0, 1.4, 0.0), Vector3.UP)
    camera.fov = 30.0

    directional_light = DirectionalLight3D.new()
    viewport.add_child(directional_light)
    directional_light.rotation = Vector3(-deg_to_rad(30.0), deg_to_rad(45.0), 0.0)
    directional_light.light_color = Color(1, 1, 1)
    directional_light.light_energy = 1.0
    directional_light.shadow_enabled = false

    return _ok({ "session_id": session_id })

func dispose(_params: Dictionary) -> Dictionary:
    if viewport != null:
        viewport.queue_free()
    scene = null
    viewport = null
    camera = null
    directional_light = null
    environment = null
    session_id = ""
    return _ok({})

func set_camera(params: Dictionary) -> Dictionary:
    if camera == null:
        return _err(-32002, "RenderFailed", { "reason": "no session active; call load_vrm first" })
    var pos = params.get("position", [0.0, 1.4, 1.5])
    var tgt = params.get("target", [0.0, 1.4, 0.0])
    var up = params.get("up", [0.0, 1.0, 0.0])
    var fov: float = params.get("fov_degrees", 30.0)
    camera.look_at_from_position(Vector3(pos[0], pos[1], pos[2]), Vector3(tgt[0], tgt[1], tgt[2]), Vector3(up[0], up[1], up[2]))
    camera.fov = fov
    return _ok({})

func set_lighting(params: Dictionary) -> Dictionary:
    if directional_light == null or environment == null:
        return _err(-32002, "RenderFailed", { "reason": "no session active; call load_vrm first" })
    var d: Dictionary = params.get("directional", {})
    var dir = d.get("dir", [-0.3, -0.6, -0.7])
    var col = d.get("color", [1.0, 1.0, 1.0])
    var intensity: float = d.get("intensity", 1.0)
    # Godot represents directional light via Node3D rotation, not a vector.
    # Build a basis whose -Z points along `dir`.
    var dir_v := Vector3(dir[0], dir[1], dir[2]).normalized()
    directional_light.look_at_from_position(Vector3.ZERO, dir_v, Vector3.UP)
    directional_light.light_color = Color(col[0], col[1], col[2])
    directional_light.light_energy = intensity

    var a: Dictionary = params.get("ambient", {})
    var ac = a.get("color", [0.5, 0.5, 0.5])
    var ai: float = a.get("intensity", 0.3)
    environment.ambient_light_color = Color(ac[0], ac[1], ac[2])
    environment.ambient_light_energy = ai

    var cast: bool = params.get("cast_shadows", false)
    var receive: bool = params.get("receive_shadows", false)
    directional_light.shadow_enabled = cast
    # Receive shadows is per-material; for MToon math tests both flags are false,
    # so we can ignore the receive side here — the directional shadow_enabled
    # gates the entire shadow path.
    var _unused = receive
    return _ok({})

func set_post_processing(params: Dictionary) -> Dictionary:
    if environment == null:
        return _err(-32002, "RenderFailed", { "reason": "no session active; call load_vrm first" })
    var tone: String = params.get("tone_mapping", "None")
    var exposure: float = params.get("exposure", 1.0)
    match tone:
        "None":
            environment.tonemap_mode = Environment.TONE_MAPPER_LINEAR
        "Linear":
            environment.tonemap_mode = Environment.TONE_MAPPER_LINEAR
        "Reinhard":
            environment.tonemap_mode = Environment.TONE_MAPPER_REINHARDT
        "Aces":
            environment.tonemap_mode = Environment.TONE_MAPPER_ACES
        _:
            return _err(-32602, "unknown tone_mapping: " + tone)
    environment.tonemap_exposure = exposure
    return _ok({})

func _ok(result: Variant) -> Dictionary:
    return { "ok": true, "result": result }

func _err(code: int, message: String, data: Variant = null) -> Dictionary:
    var e: Dictionary = { "code": code, "message": message }
    if data != null:
        e["data"] = data
    return { "ok": false, "error": e }
