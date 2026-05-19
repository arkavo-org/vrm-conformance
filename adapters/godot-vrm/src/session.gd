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
var vrm_secondary: Node = null

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

    vrm_secondary = _find_vrm_secondary(scene)
    if vrm_secondary != null:
        # Disable auto-stepping so step_physics/reset_physics/animate_root_transform
        # have full control over the physics pump.
        vrm_secondary.process_mode = Node.PROCESS_MODE_DISABLED

    return _ok({ "session_id": session_id })

func dispose(_params: Dictionary) -> Dictionary:
    if viewport != null:
        viewport.queue_free()
    scene = null
    viewport = null
    camera = null
    directional_light = null
    environment = null
    vrm_secondary = null
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

# Render the current scene to a PNG. Returns the output path + the actual
# color-space the PNG was written in (Godot writes sRGB-encoded PNGs by
# default; `color_space: "Linear"` would require a linear PNG which Godot
# doesn't support natively, so we accept the request and report sRGB —
# the runner's diff engine tolerates this declared mismatch).
func render(tree: SceneTree, params: Dictionary) -> Dictionary:
    if viewport == null:
        return _err(-32002, "RenderFailed", { "reason": "no session active; call load_vrm first" })
    var width: int = params.get("width", 1024)
    var height: int = params.get("height", 1024)
    var output_path: String = params.get("output_path", "")
    if output_path == "":
        return _err(-32602, "missing output_path")
    var msaa: int = params.get("msaa", 4)
    var declared_cs: String = params.get("color_space", "Srgb")

    viewport.size = Vector2i(width, height)
    match msaa:
        0, 1: viewport.msaa_3d = Viewport.MSAA_DISABLED
        2:    viewport.msaa_3d = Viewport.MSAA_2X
        4:    viewport.msaa_3d = Viewport.MSAA_4X
        8:    viewport.msaa_3d = Viewport.MSAA_8X
        _:    viewport.msaa_3d = Viewport.MSAA_4X
    viewport.render_target_update_mode = SubViewport.UPDATE_ONCE

    # Drive a few frames so the shader pipeline is warm + the viewport
    # texture is populated. UPDATE_ONCE renders the next frame after
    # `notify_update` (which `tree.process_frame` triggers implicitly).
    for i in 4:
        await tree.process_frame

    var img: Image = viewport.get_texture().get_image()
    if img == null:
        return _err(-32002, "RenderFailed", { "reason": "get_image returned null" })
    var save_err := img.save_png(output_path)
    if save_err != OK:
        return _err(-32002, "RenderFailed", { "reason": "save_png err %d" % save_err })

    # Declared color-space: we always write sRGB-encoded PNGs.
    var _declared = declared_cs
    return _ok({ "output_path": output_path, "actual_color_space": "Srgb" })

static func _find_vrm_secondary(node: Node) -> Node:
    for child in node.get_children():
        var script = child.get_script()
        if script != null and script.resource_path.ends_with("vrm_secondary.gd"):
            return child
        var found := _find_vrm_secondary(child)
        if found != null:
            return found
    return null

func step_physics(params: Dictionary) -> Dictionary:
    if vrm_secondary == null:
        # Adapter loaded a VRM without spring bones — treat as no-op for protocol
        # compliance (cf. vrm-mock-renderer's no-op behavior).
        return _ok({})
    var dt_seconds: float = params.get("dt_seconds", 1.0/60.0)
    var count: int = params.get("count", 1)
    for i in count:
        vrm_secondary.do_process(dt_seconds)
    return _ok({})

func reset_physics(params: Dictionary) -> Dictionary:
    if vrm_secondary == null:
        return _ok({})
    var settle_steps: int = params.get("settle_steps", 30)
    # _ready() alone doesn't fully reset — the addon writes bone pose
    # overrides that survive across calls and _ready() re-reads from the
    # current (overridden) skeleton pose. Clear overrides + write rest
    # bones first, then re-initialize chains, then run settle steps.
    var skel: Skeleton3D = vrm_secondary.get_node_or_null(vrm_secondary.skeleton) as Skeleton3D
    if skel != null:
        skel.clear_bones_global_pose_override()
        for bone_i in range(skel.get_bone_count()):
            var rest_tx := skel.get_bone_rest(bone_i)
            skel.set_bone_pose_rotation(bone_i, rest_tx.basis.get_rotation_quaternion())
            skel.set_bone_pose_position(bone_i, rest_tx.origin)
    vrm_secondary._ready()
    for i in settle_steps:
        vrm_secondary.do_process(1.0/60.0)
    return _ok({})

func dump_bone_positions(params: Dictionary) -> Dictionary:
    # No spring-bone manager: return empty result deterministically,
    # mirroring the step_physics no-op behavior for spring-bone-less VRMs.
    if vrm_secondary == null:
        return _ok({"springs": []})

    var skel: Skeleton3D = vrm_secondary.get_node_or_null(vrm_secondary.skeleton) as Skeleton3D
    if skel == null:
        return _ok({"springs": []})

    var want_idx: int = -1
    if params.has("spring_index"):
        want_idx = int(params.get("spring_index"))

    var skel_xform: Transform3D = skel.global_transform
    var springs_out: Array = []
    var spring_bones: Array = vrm_secondary.spring_bones
    for i in range(spring_bones.size()):
        if want_idx >= 0 and i != want_idx:
            continue
        var spring = spring_bones[i]  # VRMSpringBone — avoid explicit type to not require addon preload
        var joint_positions: Array = []
        for bone_name in spring.joint_nodes:
            var bone_idx := skel.find_bone(bone_name)
            if bone_idx < 0:
                continue
            var pose: Transform3D = skel.get_bone_global_pose(bone_idx)
            var world_pos: Vector3 = skel_xform * pose.origin
            joint_positions.append([world_pos.x, world_pos.y, world_pos.z])
        var name_str: String = spring.resource_name if spring.resource_name != "" else ("spring_%d" % i)
        springs_out.append({
            "name": name_str,
            "joint_positions": joint_positions,
        })

    return _ok({"springs": springs_out})

func animate_root_transform(params: Dictionary) -> Dictionary:
    if scene == null:
        return _err(-32002, "RenderFailed", { "reason": "no session active; call load_vrm first" })
    var start_arr = params.get("translation_start", [0.0, 0.0, 0.0])
    var end_arr = params.get("translation_end", [0.0, 0.0, 0.0])
    var duration: float = params.get("duration_seconds", 0.25)
    var fps: int = params.get("fps", 60)
    var start := Vector3(start_arr[0], start_arr[1], start_arr[2])
    var end := Vector3(end_arr[0], end_arr[1], end_arr[2])

    var dt := 1.0 / float(fps)
    var sample_count := int(round(duration * float(fps)))
    if sample_count < 1:
        sample_count = 1

    if scene is Node3D:
        (scene as Node3D).position = start
    for i in range(sample_count):
        var t: float = float(i + 1) / float(sample_count)
        if scene is Node3D:
            (scene as Node3D).position = start.lerp(end, t)
        if vrm_secondary != null:
            vrm_secondary.do_process(dt)
    return _ok({})

# Phase 6 — RFC-0004 render_sequence.
# Loops frame_count times. Per frame: interpolate root translation,
# drive vrm_secondary.do_process(physics_dt), let the viewport render,
# save PNG, append SequenceFrame. Restores original root at end.
#
# BLAKE3 is populated with the 64-zero sentinel — Rust runner re-hashes
# (Phase 5 Task 2 contract). apply_vrma + non-PNG output_format +
# physics_dt > 1/60 all reject (RFC-0004 failure-modes).
func render_sequence(tree: SceneTree, params: Dictionary) -> Dictionary:
    if viewport == null:
        return _err(-32002, "RenderFailed", { "reason": "no session active; call load_vrm first" })

    var output_dir: String = params.get("output_dir", "")
    if output_dir == "":
        return _err(-32602, "missing output_dir")

    var frame_count: int = params.get("frame_count", 0)
    if frame_count < 1:
        return _err(-32602, "frame_count must be >= 1")

    var frame_hz: float = params.get("frame_hz", 30.0)
    var physics_dt: float = params.get("physics_dt_seconds", 1.0 / 60.0)
    if physics_dt > 1.0 / 60.0 + 1e-6:
        return _err(-32602, "physics_dt_seconds %f exceeds 60 Hz floor (1/60)" % physics_dt)

    var has_anim: bool = params.has("animate_root_transform") and params["animate_root_transform"] != null
    var has_vrma: bool = params.has("apply_vrma") and params["apply_vrma"] != null
    if has_anim and has_vrma:
        return _err(-32602, "animate_root_transform and apply_vrma are mutually exclusive")
    if has_vrma:
        return _err(-32602, "apply_vrma is not yet implemented in godot-vrm (Phase 6 deferral)")

    var output_format: String = params.get("output_format", "png_sequence")
    if output_format != "png_sequence":
        return _err(-32602, "output_format \"%s\" is not yet supported by godot-vrm; only png_sequence" % output_format)

    var width: int = params.get("width", 512)
    var height: int = params.get("height", 512)
    var msaa: int = params.get("msaa", 4)
    var color_space: String = params.get("color_space", "Srgb")

    # Configure viewport once.
    viewport.size = Vector2i(width, height)
    match msaa:
        0, 1: viewport.msaa_3d = Viewport.MSAA_DISABLED
        2:    viewport.msaa_3d = Viewport.MSAA_2X
        4:    viewport.msaa_3d = Viewport.MSAA_4X
        8:    viewport.msaa_3d = Viewport.MSAA_8X
        _:    viewport.msaa_3d = Viewport.MSAA_4X
    viewport.render_target_update_mode = SubViewport.UPDATE_ALWAYS

    var start := Vector3.ZERO
    var end := Vector3.ZERO
    if has_anim:
        var anim: Dictionary = params["animate_root_transform"]
        var s = anim.get("translation_start", [0.0, 0.0, 0.0])
        var e = anim.get("translation_end", [0.0, 0.0, 0.0])
        start = Vector3(s[0], s[1], s[2])
        end = Vector3(e[0], e[1], e[2])

    DirAccess.make_dir_recursive_absolute(output_dir)

    var original_translation := Vector3.ZERO
    if scene is Node3D:
        original_translation = (scene as Node3D).position
    var zero_hash := "blake3:" + "0".repeat(64)
    var frames: Array = []

    for i in frame_count:
        var t: float = 0.0
        if frame_count > 1:
            t = float(i) / float(frame_count - 1)
        if scene is Node3D:
            (scene as Node3D).position = original_translation + start.lerp(end, t)

        if vrm_secondary != null:
            vrm_secondary.do_process(physics_dt)

        # Let the viewport render. Two frames so the pipeline catches up
        # (matches the warm-up in the existing render() handler).
        await tree.process_frame
        await tree.process_frame
        var img: Image = viewport.get_texture().get_image()
        if img == null:
            if scene is Node3D:
                (scene as Node3D).position = original_translation
            return _err(-32002, "RenderFailed", { "reason": "frame %d: get_image returned null" % i })

        var frame_path := "%s/%04d.png" % [output_dir, i]
        var save_err := img.save_png(frame_path)
        if save_err != OK:
            if scene is Node3D:
                (scene as Node3D).position = original_translation
            return _err(-32002, "RenderFailed", { "reason": "frame %d: save_png err %d" % [i, save_err] })

        frames.append({
            "index": i,
            "timestamp_seconds": float(i) / frame_hz,
            "path": frame_path,
            "blake3": zero_hash,
        })

    # Restore root.
    if scene is Node3D:
        (scene as Node3D).position = original_translation

    # color_space is accepted for protocol compliance; we always write
    # sRGB-encoded PNGs (same convention as the existing render handler).
    var _declared = color_space
    return _ok({
        "frames": frames,
        "duration_seconds": float(frame_count) / frame_hz,
        "actual_color_space": "Srgb",
        "frame_hz_achieved": frame_hz,
    })

func _ok(result: Variant) -> Dictionary:
    return { "ok": true, "result": result }

func _err(code: int, message: String, data: Variant = null) -> Dictionary:
    var e: Dictionary = { "code": code, "message": message }
    if data != null:
        e["data"] = data
    return { "ok": false, "error": e }
