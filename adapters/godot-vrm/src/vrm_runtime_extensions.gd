# Registers all VRM 1.0 GLTFDocument extensions for runtime load.
#
# The vendored V-Sekai/godot-vrm addon classes (1.0/VRMC_*.gd) implement
# _import_preflight + _import_post but do NOT override _get_supported_extensions().
# Godot 4.6's GLTFDocument._parse_gltf_extensions requires that override or it
# rejects the extension with err 43 (ERR_PARSE_ERROR). This helper wraps each
# upstream class in a subclass that declares its extension name and registers
# all five.

class_name VrmRuntimeExtensions

class _VRMC_vrm extends "res://addons/vrm/1.0/VRMC_vrm.gd":
    func _get_supported_extensions() -> PackedStringArray:
        return PackedStringArray(["VRMC_vrm"])

class _VRMC_node_constraint extends "res://addons/vrm/1.0/VRMC_node_constraint.gd":
    func _get_supported_extensions() -> PackedStringArray:
        return PackedStringArray(["VRMC_node_constraint"])

class _VRMC_springBone extends "res://addons/vrm/1.0/VRMC_springBone.gd":
    func _get_supported_extensions() -> PackedStringArray:
        return PackedStringArray(["VRMC_springBone"])

class _VRMC_materials_mtoon extends "res://addons/vrm/1.0/VRMC_materials_mtoon.gd":
    func _get_supported_extensions() -> PackedStringArray:
        return PackedStringArray(["VRMC_materials_mtoon"])

class _VRMC_materials_hdr_emissiveMultiplier extends "res://addons/vrm/1.0/VRMC_materials_hdr_emissiveMultiplier.gd":
    func _get_supported_extensions() -> PackedStringArray:
        return PackedStringArray(["VRMC_materials_hdr_emissiveMultiplier"])

# Returns an array of registered extension instances; pass to unregister_all
# after the load completes (or fails). Uses the static GLTFDocument method
# so the registration is process-global across all GLTFDocument instances.
static func register_all() -> Array:
    var instances := [
        _VRMC_vrm.new(),
        _VRMC_node_constraint.new(),
        _VRMC_springBone.new(),
        _VRMC_materials_mtoon.new(),
        _VRMC_materials_hdr_emissiveMultiplier.new(),
    ]
    for inst in instances:
        GLTFDocument.register_gltf_document_extension(inst)
    return instances

static func unregister_all(instances: Array) -> void:
    for inst in instances:
        GLTFDocument.unregister_gltf_document_extension(inst)
