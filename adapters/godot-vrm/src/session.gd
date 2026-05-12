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

func _ok(result: Variant) -> Dictionary:
    return { "ok": true, "result": result }

func _err(code: int, message: String, data: Variant = null) -> Dictionary:
    var e: Dictionary = { "code": code, "message": message }
    if data != null:
        e["data"] = data
    return { "ok": false, "error": e }
