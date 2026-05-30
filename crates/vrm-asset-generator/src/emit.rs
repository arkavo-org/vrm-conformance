//! Top-level VRM 1.0 asset emission. Combines mesh, buffer, humanoid stub,
//! and VRM extensions into a single `.vrm` GLB on disk.

use crate::buffer::{pack_mesh, pack_mesh_with_morphs, pack_sphere_and_multichains};
use crate::glb::{write_glb, GlbDocument};
use crate::humanoid::minimal_skeleton;
use crate::mesh::{quad, sphere};
use crate::params::MToonParams;
use crate::vrm_ext::{base_material, viseme_preset_binds, vrmc_vrm};
use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use serde_json::{json, Value};

pub fn emit_vrm(params: &MToonParams, output: &Utf8Path) -> Result<()> {
    emit_vrm_with_custom_expressions(params, output, &[])
}

/// Like [`emit_vrm`] but pre-registers the named custom expressions on
/// the avatar's `VRMC_vrm.expressions.custom` map with empty
/// `morphTargetBinds`. Used by the VRMA expression sweep's custom-
/// expression variants so `VRMExpressionController` accepts
/// `setCustomExpressionWeight(name, …)` instead of silently no-op'ing
/// (per `VRMMorphTargets.swift:533` and equivalents in three-vrm /
/// godot-vrm / UniVRM). Bound morphs are intentionally empty: the test
/// signal is the controller's `weight(forCustom:)` being non-zero, not
/// a visible mesh deformation.
pub fn emit_vrm_with_custom_expressions(
    params: &MToonParams,
    output: &Utf8Path,
    custom_expression_names: &[&str],
) -> Result<()> {
    // 1) Mesh + buffer + five POSITION-only morph targets for visemes
    //    (aa, ih, ou, ee, oh). Each delta produces a visually distinct
    //    deformation pattern when the corresponding expression weight is
    //    driven to 1.0, so cross-renderer SSIM can falsify "weight accepted
    //    but mesh not deformed". Order MUST match `VISEME_PRESETS`.
    let mesh = sphere(0.3, 24, 48); // small radius so the sphere fits at avatar chest height
    let morphs = viseme_morph_deltas(&mesh.positions);
    let (packed, morph_accessors) = pack_mesh_with_morphs(&mesh, &morphs);

    // 2) Humanoid skeleton
    let skeleton = minimal_skeleton();
    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();
    let head_node = skeleton.bone_to_node["head"];

    // 3) Add a mesh-bearing node parented to head (so the sphere visualizes
    //    where the head is). Material 0 = our MToon material.
    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", params.id),
        "mesh": 0
    }));
    // Append mesh_node_index as a child of head.
    let head = &mut nodes[head_node];
    let mut head_children = head["children"].as_array().cloned().unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

    let targets: Vec<Value> = morph_accessors
        .iter()
        .map(|&idx| json!({ "POSITION": idx }))
        .collect();

    // Declare every extension the document actually carries. Spec-wise
    // `extensionsUsed` is informative (renderers ignore unknowns) but
    // validators enforce that every extension referenced in the JSON
    // appears here. The hdr_emissiveMultiplier entry is only added when
    // the material's effective multiplier diverges from the default 1.0,
    // matching the conditional emission in `base_material`.
    let mut extensions_used: Vec<&str> =
        vec!["KHR_materials_unlit", "VRMC_vrm", "VRMC_materials_mtoon"];
    let emits_emissive_multiplier = params.emissive_factor.iter().any(|&c| c != 0.0)
        && (params.emissive_multiplier - 1.0).abs() > f32::EPSILON;
    if emits_emissive_multiplier {
        extensions_used.push("VRMC_materials_hdr_emissiveMultiplier");
    }
    // KHR_texture_transform is referenced on every textureInfo whose
    // material carries the extension. We only emit it when the
    // transform is non-identity (see base_material), so the
    // extensionsUsed entry follows the same condition.
    let emits_texture_transform = params
        .texture_transform
        .map(|t| !t.is_identity())
        .unwrap_or(false);
    if emits_texture_transform {
        extensions_used.push("KHR_texture_transform");
    }

    // 4) Build the glTF JSON document
    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": extensions_used,
        "extensionsRequired": ["VRMC_vrm"],
        "scene": 0,
        "scenes": [
            { "nodes": [skeleton.root_node] }
        ],
        "nodes": nodes,
        "meshes": [
            {
                "name": format!("{}_geom", params.id),
                "primitives": [
                    {
                        "attributes": {
                            "POSITION": 0,
                            "NORMAL": 1,
                            "TEXCOORD_0": 2
                        },
                        "indices": 3,
                        "material": 0,
                        "mode": 4,
                        "targets": targets
                    }
                ]
            }
        ],
        "materials": [base_material(params)],
        "extensions": {
            "VRMC_vrm": vrmc_vrm(&params.id, &skeleton.bone_to_node, mesh_node_index)
        }
    });

    doc["extensions"]["VRMC_vrm"]["expressions"]["preset"] = viseme_preset_binds(mesh_node_index);

    // Attach the procedural quadrant-checkerboard texture (16x16 RGBA
    // PNG, encoded inline as a data URI) when any sweep variant has
    // requested a textured material. Stays out of the GLB binary
    // chunk to avoid touching the pack_mesh accessor maths — the
    // image lives in JSON. samplers[0] uses REPEAT wrap mode so
    // KHR_texture_transform variants with non-[0,1] UVs (offset > 0,
    // scale > 1) display the tiling.
    //
    // Triggered by any of the texture-needing params; the texture is
    // shared across binding points (baseColorTexture +
    // shadeMultiplyTexture + matcapTexture + shadingShiftTexture +
    // rimMultiplyTexture all reference index 0).
    let needs_texture = params.texture_transform.is_some()
        || params.shade_multiply_texture
        || params.matcap_texture
        || params.shading_shift_texture_scale.is_some()
        || params.rim_multiply_texture
        || params.outline_width_multiply_texture
        || params.normal_texture_scale.is_some()
        || params.occlusion_texture_strength.is_some();
    let needs_normal_map = params.normal_texture_scale.is_some();
    if needs_texture {
        let img = crate::texture::quadrant_checkerboard_16();
        let data_uri = crate::texture::image_as_data_uri(&img);
        let mut images = vec![json!({
            "name": format!("{}_checkerboard", params.id),
            "uri": data_uri,
            "mimeType": "image/png",
        })];
        let sampler = json!({
            "wrapS": 10497,    // REPEAT
            "wrapT": 10497,    // REPEAT
            "magFilter": 9729, // LINEAR
            "minFilter": 9729, // LINEAR (no mipmap to keep math testable)
        });
        let mut samplers = vec![sampler.clone()];
        let mut textures = vec![json!({ "source": 0, "sampler": 0 })];

        // glTF-core `normalTexture` needs a tangent-space normal map
        // (distinct RGB encoding from the color checkerboard), so we
        // append a second image/texture pair at index 1. Same REPEAT
        // sampler — normal maps don't need a special filter for our
        // test corpus.
        if needs_normal_map {
            let normal_img = crate::texture::quadrant_normal_map_16();
            let normal_uri = crate::texture::image_as_data_uri(&normal_img);
            images.push(json!({
                "name": format!("{}_normal_map", params.id),
                "uri": normal_uri,
                "mimeType": "image/png",
            }));
            samplers.push(sampler);
            textures.push(json!({ "source": 1, "sampler": 1 }));
        }

        doc["images"] = Value::Array(images);
        doc["samplers"] = Value::Array(samplers);
        doc["textures"] = Value::Array(textures);
    }

    // Apply the firstPerson.meshAnnotations[0].type override when the
    // caller wants something other than the canonical "auto" default
    // (which is what `vrmc_vrm` writes). The annotation node index is
    // already the mesh-bearing node so we only need to flip `type`.
    if let Some(t) = params.first_person_type {
        doc["extensions"]["VRMC_vrm"]["firstPerson"]["meshAnnotations"][0]["type"] =
            serde_json::json!(t.as_spec_str());
    }

    if !custom_expression_names.is_empty() {
        let mut custom_map = serde_json::Map::new();
        for name in custom_expression_names {
            custom_map.insert((*name).to_string(), json!({ "morphTargetBinds": [] }));
        }
        doc["extensions"]["VRMC_vrm"]["expressions"]["custom"] = Value::Object(custom_map);
    }

    // Splice in buffers/bufferViews/accessors from the packed mesh.
    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    // 5) Serialize and write GLB
    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, glb)?;
    Ok(())
}

/// POSITION-only morph deltas for the five viseme presets in
/// [`crate::vrm_ext::VISEME_PRESETS`] order. Each delta yields a visually
/// distinct deformation of the head-mounted sphere so the rendered images
/// disambiguate per viseme. NORMAL is omitted by design — shading on the
/// deformed sphere will be approximate but the silhouette change is the
/// signal the SSIM diff measures.
fn viseme_morph_deltas(positions: &[[f32; 3]]) -> Vec<Vec<[f32; 3]>> {
    let n = positions.len();
    let aa: Vec<[f32; 3]> = (0..n).map(|_| [0.04, 0.0, 0.0]).collect();
    let ih: Vec<[f32; 3]> = (0..n).map(|_| [0.0, -0.04, 0.0]).collect();
    let ou: Vec<[f32; 3]> = (0..n).map(|_| [0.0, 0.0, 0.04]).collect();
    let ee: Vec<[f32; 3]> = (0..n).map(|_| [-0.04, 0.0, 0.0]).collect();
    let oh: Vec<[f32; 3]> = positions
        .iter()
        .map(|p| [p[0] * 0.1, p[1] * 0.1, p[2] * 0.1])
        .collect();
    vec![aa, ih, ou, ee, oh]
}

use crate::sidecar::{build_default_test_plan, write_meta_json, write_test_yaml};
use crate::spring_bone::{ColliderAttach, ColliderShape, SpringBoneParams, SpringBoneSceneParams};
use crate::vrm_ext::{
    vrmc_spring_bone, vrmc_spring_bone_scene, vrmc_vrm_with_lookat_type, LookAtType,
};

/// Emit a `.vrm` GLB identical to `emit_vrm` except that the avatar's
/// `VRMC_vrm.lookAt.type` is set to the caller-supplied value.
/// Existing callers of `emit_vrm` are unaffected.
pub fn emit_vrm_with_lookat_type(
    params: &MToonParams,
    lookat_type: LookAtType,
    output: &Utf8Path,
) -> Result<()> {
    // 1) Mesh + buffer
    let mesh = sphere(0.3, 24, 48);
    let packed = pack_mesh(&mesh);

    // 2) Humanoid skeleton
    let skeleton = minimal_skeleton();
    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();
    let head_node = skeleton.bone_to_node["head"];

    // 3) Mesh-bearing node parented to head.
    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", params.id),
        "mesh": 0
    }));
    let head = &mut nodes[head_node];
    let mut head_children = head["children"].as_array().cloned().unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

    // 4) Build the glTF JSON document with the requested lookAt.type.
    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": ["KHR_materials_unlit", "VRMC_vrm", "VRMC_materials_mtoon"],
        "extensionsRequired": ["VRMC_vrm"],
        "scene": 0,
        "scenes": [
            { "nodes": [skeleton.root_node] }
        ],
        "nodes": nodes,
        "meshes": [
            {
                "name": format!("{}_geom", params.id),
                "primitives": [
                    {
                        "attributes": {
                            "POSITION": 0,
                            "NORMAL": 1,
                            "TEXCOORD_0": 2
                        },
                        "indices": 3,
                        "material": 0,
                        "mode": 4
                    }
                ]
            }
        ],
        "materials": [base_material(params)],
        "extensions": {
            "VRMC_vrm": vrmc_vrm_with_lookat_type(&params.id, &skeleton.bone_to_node, mesh_node_index, lookat_type)
        }
    });

    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, glb)?;
    Ok(())
}

/// Emit a `.vrm` GLB carrying a single open quad (no morphs) as the only
/// renderable, for the doubleSided back-face-culling spec test.
///
/// Unlike `emit_vrm` (closed sphere + viseme morphs), this is an open
/// single-quad surface whose front face points +Z. Paired with a camera on
/// the −Z side (see `build_doublesided_quad_test_plan`), the quad's BACK face
/// is in frame, so back-face culling becomes observable: `doubleSided=false`
/// culls it (all-background frame), `doubleSided=true` renders it. The minimal
/// humanoid skeleton is retained only to satisfy VRMC_vrm validation; the rest
/// pose is pure translation, so the quad's +Z normal survives into world space.
pub fn emit_vrm_doublesided_quad(params: &MToonParams, output: &Utf8Path) -> Result<()> {
    let mesh = quad(0.3);
    let packed = pack_mesh(&mesh);

    let skeleton = minimal_skeleton();
    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();
    let head_node = skeleton.bone_to_node["head"];

    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_quad", params.id),
        "mesh": 0
    }));
    let head = &mut nodes[head_node];
    let mut head_children = head["children"].as_array().cloned().unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": ["KHR_materials_unlit", "VRMC_vrm", "VRMC_materials_mtoon"],
        "extensionsRequired": ["VRMC_vrm"],
        "scene": 0,
        "scenes": [
            { "nodes": [skeleton.root_node] }
        ],
        "nodes": nodes,
        "meshes": [
            {
                "name": format!("{}_geom", params.id),
                "primitives": [
                    {
                        "attributes": {
                            "POSITION": 0,
                            "NORMAL": 1,
                            "TEXCOORD_0": 2
                        },
                        "indices": 3,
                        "material": 0,
                        "mode": 4
                    }
                ]
            }
        ],
        "materials": [base_material(params)],
        "extensions": {
            "VRMC_vrm": vrmc_vrm(&params.id, &skeleton.bone_to_node, mesh_node_index)
        }
    });

    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, glb)?;
    Ok(())
}

/// Returns `true` when the scene contains any extended-shape collider
/// (Plane, InsideSphere, InsideCapsule) or any spring with a joint angle
/// limit. When true, the glTF `extensionsUsed` must declare
/// `"VRMC_springBone_extended_collider"`.
fn scene_uses_extended_collider(scene: &SpringBoneSceneParams) -> bool {
    let has_extended_shape = scene.colliders.iter().any(|c| {
        matches!(
            &c.shape,
            ColliderShape::Plane { .. }
                | ColliderShape::InsideSphere { .. }
                | ColliderShape::InsideCapsule { .. }
        )
    });
    let has_angle_limit = scene
        .springs
        .iter()
        .any(|s| s.joint_angle_limit_deg.is_some());
    has_extended_shape || has_angle_limit
}

/// Emit the doubleSided quad triplet (`.vrm` + `.meta.json` + `.test.yaml`).
/// `cross_variant_sibling` names the opposite variant for the cross-variant
/// SSIM assertion; set it on the `false` variant only.
pub fn emit_with_sidecars_doublesided_quad(
    params: &MToonParams,
    stem: &Utf8Path,
    cross_variant_sibling: Option<&str>,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_doublesided_quad(params, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(params, None, &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = crate::sidecar::build_doublesided_quad_test_plan(
        params,
        &asset_relpath,
        cross_variant_sibling,
    );
    write_test_yaml(&plan, &yaml_path)?;
    Ok(())
}

/// Emit the doubleSided back-face-culling spec-test pair: two triplets,
/// `doublesided_quad_false` and `doublesided_quad_true`, identical except for
/// the `double_sided` flag and the `false` variant's cross_variant block.
/// Returns the emitted stems (without extension). UniVRM is the reference golden.
pub fn emit_doublesided_spec_test_pair(output_dir: &Utf8Path) -> Result<Vec<Utf8PathBuf>> {
    std::fs::create_dir_all(output_dir)?;

    let mut false_params = MToonParams::defaults("doublesided_quad_false");
    false_params.double_sided = false;
    let mut true_params = MToonParams::defaults("doublesided_quad_true");
    true_params.double_sided = true;

    let false_stem = output_dir.join("doublesided_quad_false");
    emit_with_sidecars_doublesided_quad(&false_params, &false_stem, Some("doublesided_quad_true"))?;

    let true_stem = output_dir.join("doublesided_quad_true");
    emit_with_sidecars_doublesided_quad(&true_params, &true_stem, None)?;

    Ok(vec![false_stem, true_stem])
}

/// Emits `<stem>.vrm`, `<stem>.meta.json`, and `<stem>.test.yaml` from a
/// single MToonParams value.
pub fn emit_with_sidecars(params: &MToonParams, stem: &Utf8Path) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm(params, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(params, None, &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = build_default_test_plan(params, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emit a VRM 0.x `.vrm` GLB with the full VRM extension block.
///
/// Produces a parseable GLB with `extensionsUsed: ["VRM"]` and a complete
/// `VRM` extension block (meta, humanoid, firstPerson, blendShapeMaster,
/// secondaryAnimation, materialProperties) assembled by
/// [`crate::vrm_ext_v0::emit_vrm_extension`].
/// No binary chunk: the minimal 0.x default asset has no mesh geometry.
///
/// `materials` is the list of MToon material variants to embed in
/// `materialProperties`. Pass `&[MToonParams::defaults(id)]` for the default
/// single-material case; pass a specific sweep variant's `MToonParams` for
/// parametric sweep emission.
pub fn emit_vrm_v0(id: &str, materials: &[MToonParams], output: &Utf8Path) -> Result<()> {
    use crate::expressions_v0::ExpressionsV0Params;

    let expressions = ExpressionsV0Params { groups: vec![] };
    emit_vrm_v0_with_expressions(id, materials, &expressions, output)
}

/// Like [`emit_vrm_v0`] but accepts caller-supplied `ExpressionsV0Params`
/// so the canonical normalization test pair can embed real morph-target
/// bindings in `blendShapeMaster.blendShapeGroups[]`.
pub fn emit_vrm_v0_with_expressions(
    id: &str,
    materials: &[MToonParams],
    expressions: &crate::expressions_v0::ExpressionsV0Params,
    output: &Utf8Path,
) -> Result<()> {
    let vrm_ext = crate::vrm_ext_v0::emit_vrm_extension(id, materials, expressions);

    let doc = serde_json::json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": ["VRM"],
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "name": id }],
        "extensions": {
            "VRM": vrm_ext
        }
    });

    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = crate::glb::write_glb(&crate::glb::GlbDocument {
        json: json_bytes,
        binary: Vec::new(),
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, glb)?;
    Ok(())
}

/// Emit a VRM 0.x asset triplet: `.vrm`, `.meta.json`, `.test.yaml`.
///
/// The `.vrm` is produced by [`emit_vrm_v0`] — a complete VRM 0.x GLB with
/// full extension blocks (meta, humanoid, firstPerson, blendShapeMaster,
/// secondaryAnimation, materialProperties). The sidecar files use
/// spec_version `0.x` so plans are clearly tagged.
pub fn emit_with_sidecars_v0(params: &MToonParams, stem: &Utf8Path) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_v0(&params.id, &[params.clone()], &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(params, None, &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let mut plan = build_default_test_plan(params, &asset_relpath);
    // Tag the plan as VRM 0.x so runners and validators know the spec target.
    crate::sidecar::tag_plan_vrm0(&mut plan);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Like [`emit_with_sidecars_v0`] but embeds caller-supplied
/// `ExpressionsV0Params` into the `blendShapeMaster.blendShapeGroups[]`
/// block. Used by `emit-expressions-preset-basic` for the v0 side of the
/// canonical normalization test pair.
pub fn emit_with_sidecars_v0_with_expressions(
    params: &MToonParams,
    expressions: &crate::expressions_v0::ExpressionsV0Params,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_v0_with_expressions(&params.id, &[params.clone()], expressions, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(params, None, &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let mut plan = build_default_test_plan(params, &asset_relpath);
    crate::sidecar::tag_plan_vrm0(&mut plan);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emit a VRM 1.0 asset triplet with caller-supplied `ExpressionsV1Params`
/// wired into `VRMC_vrm.expressions.preset`.
///
/// The emitted `.vrm` is structurally identical to [`emit_vrm`] (sphere mesh +
/// humanoid skeleton + five viseme morph-targets) except that the
/// `expressions.preset` block is built from `expr_params` instead of the
/// default viseme preset set. This is the v1 side of the canonical
/// normalization test pair (`expressions_preset_basic`).
///
/// Note: the mesh still carries five morph-target accessors (the viseme
/// geometry from `pack_mesh_with_morphs`). The expression preset binds in
/// `expr_params` reference `morph_target_index: 0` on the mesh node, which
/// IS a valid morph-target accessor index (the "aa" viseme delta). Option A
/// per task spec: the binding is what we test, not the geometry's semantics.
pub fn emit_with_sidecars_v1_with_expressions(
    params: &MToonParams,
    expr_params: &crate::vrm_ext::ExpressionsV1Params,
    stem: &Utf8Path,
) -> Result<()> {
    use crate::vrm_ext::preset_expression_binds_from_params;

    // Build the same geometry as emit_vrm: sphere + 5 viseme morph-targets.
    let mesh = sphere(0.3, 24, 48);
    let morphs = viseme_morph_deltas(&mesh.positions);
    let (packed, morph_accessors) = crate::buffer::pack_mesh_with_morphs(&mesh, &morphs);

    let skeleton = minimal_skeleton();
    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();
    let head_node = skeleton.bone_to_node["head"];

    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", params.id),
        "mesh": 0
    }));
    let head = &mut nodes[head_node];
    let mut head_children = head["children"].as_array().cloned().unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

    let targets: Vec<Value> = morph_accessors
        .iter()
        .map(|&idx| json!({ "POSITION": idx }))
        .collect();

    let extensions_used: Vec<&str> =
        vec!["KHR_materials_unlit", "VRMC_vrm", "VRMC_materials_mtoon"];

    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": extensions_used,
        "extensionsRequired": ["VRMC_vrm"],
        "scene": 0,
        "scenes": [
            { "nodes": [skeleton.root_node] }
        ],
        "nodes": nodes,
        "meshes": [
            {
                "name": format!("{}_geom", params.id),
                "primitives": [
                    {
                        "attributes": {
                            "POSITION": 0,
                            "NORMAL": 1,
                            "TEXCOORD_0": 2
                        },
                        "indices": 3,
                        "material": 0,
                        "mode": 4,
                        "targets": targets
                    }
                ]
            }
        ],
        "materials": [base_material(params)],
        "extensions": {
            "VRMC_vrm": vrmc_vrm(&params.id, &skeleton.bone_to_node, mesh_node_index)
        }
    });

    // Override expressions.preset with the caller-supplied binds, updating
    // the node index to the actual mesh node index (the sweep uses 0 as a
    // placeholder; the real mesh node is determined after skeleton layout).
    let mut resolved = expr_params.clone();
    for (_, bind) in &mut resolved.preset_binds {
        bind.node = mesh_node_index as u32;
    }
    doc["extensions"]["VRMC_vrm"]["expressions"]["preset"] =
        preset_expression_binds_from_params(&resolved);

    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    let vrm_path = stem.with_extension("vrm");
    if let Some(parent) = vrm_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;
    std::fs::write(&vrm_path, glb)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(params, None, &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = build_default_test_plan(params, &asset_relpath);
    // spec_version defaults to V1 (the back-compat default per TestPlan schema);
    // explicit here for clarity.
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emit a `.skipped.json` marker file for a NotApplicable sweep variant.
///
/// Instead of a `.vrm` asset, writes a small JSON file that records why the
/// variant is not applicable for the target spec version. The runner and site
/// consume this marker to skip rendering and diff for this variant without
/// treating the absence of a `.vrm` as a missing-asset error.
///
/// Convention: `<id>.skipped.json` co-located with the Applicable assets in
/// the same output directory.
pub fn emit_not_applicable_marker(
    id: &str,
    reason: crate::NotApplicableReason,
    output_dir: &Utf8Path,
) -> std::io::Result<()> {
    let marker = serde_json::json!({
        "kind": "NotApplicable",
        "reason": format!("{reason:?}"),
        "test_id": id,
    });
    let path = output_dir.join(format!("{id}.skipped.json"));
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&marker).map_err(std::io::Error::other)?,
    )?;
    Ok(())
}

/// World position of chain joint `i` (0-based) given the chain root world,
/// the unit `chain_axis`, and `segment_length_m`. Joint 0 sits one segment
/// from the root (head).
fn chain_joint_world(root: [f32; 3], axis: [f32; 3], seg: f32, i: u32) -> [f32; 3] {
    let step = (i + 1) as f32 * seg;
    [
        root[0] + axis[0] * step,
        root[1] + axis[1] * step,
        root[2] + axis[2] * step,
    ]
}

/// Column-major glTF Mat4 that is a pure inverse translation of `p`.
///
/// Uses `0.0 - p[i]` rather than the unary negation `-p[i]` so that zero
/// inputs produce `+0.0` instead of `-0.0` (IEEE 754: `0.0 - 0.0 = +0.0`).
/// This preserves byte-identity with the pre-axis-feature code path, which
/// hardcoded `0.0` literals for the X/Z translation entries.
fn inv_translation_mat4(p: [f32; 3]) -> [f32; 16] {
    #[rustfmt::skip]
    let m = [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0 - p[0], 0.0 - p[1], 0.0 - p[2], 1.0,
    ];
    m
}

#[cfg(test)]
mod chain_helper_tests {
    use super::*;

    #[test]
    fn ibm_default_axis_matches_legacy_y_only() {
        let head = crate::humanoid::rest_pose_world_position("head");
        let seg = 0.05;
        for i in 0..4u32 {
            let p = chain_joint_world(head, [0.0, -1.0, 0.0], seg, i);
            let m = inv_translation_mat4(p);
            let jy = head[1] - ((i + 1) as f32) * seg;
            assert!((m[13] - (-jy)).abs() < 1e-6, "element 13");
            assert!(
                m[12].abs() < 1e-6 && m[14].abs() < 1e-6,
                "X/Z translation zero"
            );
        }
    }
}

/// Emit a `.vrm` GLB containing MToon material data, a VRMC_springBone
/// chain attached to the head bone, **and a cylinder mesh skinned to
/// the chain joints** so spring-bone physics is visible in pixel space.
///
/// Three renderables:
///
/// 1. A head-mounted sphere (the existing MToon material reference shape,
///    parented to the head node, not skinned).
/// 2. A vertical cylinder hanging from the head, weighted to the chain
///    joints. When a renderer's spring-bone physics moves a joint, the
///    cylinder bends with it.
/// 3. The VRMC_springBone extension declaring the chain itself.
///
/// Wiring was previously deferred because vrm-metal-kit at our prior pin
/// dropped non-skinned meshes when any skin was present
/// ([VRMMetalKit#181](https://github.com/arkavo-org/VRMMetalKit/issues/181));
/// the 0.13.1 release closes that, so the chain-skinned mesh now coexists
/// with the head-mounted sphere across all renderers.
pub fn emit_vrm_with_spring_bone(
    mtoon: &MToonParams,
    spring_bone: &SpringBoneParams,
    output: &Utf8Path,
) -> Result<()> {
    debug_assert!(
        !(spring_bone.explicit_tail
            && (spring_bone.stiffness_per_joint.is_some()
                || spring_bone.drag_force_per_joint.is_some()
                || spring_bone.gravity_power_per_joint.is_some()
                || spring_bone.hit_radius_per_joint.is_some())),
        "explicit_tail with per-joint taper arrays is unsupported (joint-count mismatch)"
    );

    let mesh = sphere(0.3, 24, 48);

    let mut skeleton = minimal_skeleton();
    let head_node = skeleton.bone_to_node["head"];
    let chain_nodes = crate::humanoid::append_spring_chain_axis(
        &mut skeleton,
        head_node,
        spring_bone.joint_count,
        spring_bone.segment_length_m,
        spring_bone.chain_axis,
    );

    let head_world = crate::humanoid::rest_pose_world_position("head");
    // Joint 0 (chain top) = head + axis * segment_length.
    let chain_top = chain_joint_world(
        head_world,
        spring_bone.chain_axis,
        spring_bone.segment_length_m,
        0,
    );

    let chain_mesh = crate::chain_mesh::build_chain_cylinder(
        spring_bone.joint_count,
        spring_bone.segment_length_m,
        0.025,
        chain_top,
        spring_bone.chain_axis,
        12,
    );

    // Inverse-bind matrices: joint i bind-pose world = head + axis*(i+1)*seg.
    let inv_bind: Vec<[f32; 16]> = (0..spring_bone.joint_count)
        .map(|i| {
            let p = chain_joint_world(
                head_world,
                spring_bone.chain_axis,
                spring_bone.segment_length_m,
                i,
            );
            inv_translation_mat4(p)
        })
        .collect();

    let packed = crate::buffer::pack_sphere_and_chain(&mesh, &chain_mesh, &inv_bind);

    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();

    // Sphere mesh node — child of head, identical to emit_vrm's wiring.
    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", mtoon.id),
        "mesh": 0
    }));
    let head = &mut nodes[head_node];
    let mut head_children = head["children"].as_array().cloned().unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

    // Chain-skinned node — child of hips so the scene stays single-rooted.
    // Its own transform is ignored at draw time; skin.joints +
    // inverseBindMatrices fully determine vertex positions.
    let chain_mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_chain_mesh", mtoon.id),
        "mesh": 1,
        "skin": 0
    }));
    let hips = &mut nodes[skeleton.root_node];
    let mut hips_children = hips["children"].as_array().cloned().unwrap_or_default();
    hips_children.push(json!(chain_mesh_node_index));
    hips["children"] = Value::Array(hips_children);

    // Optional explicit 7 cm tail (VRM 1.0 parity twin with 0.x synthesized-tail).
    // This is a sim-only joint — NOT mesh-weighted (skin.joints stays chain_nodes only).
    // The 7 cm constant is from the VRM spec (VRMC_springBone-1.0/README.md:137-153),
    // independent of segment_length_m. Fully gated behind explicit_tail so the default
    // (false) path remains byte-identical.
    let spring_joint_nodes: Vec<usize> = if spring_bone.explicit_tail {
        let end_idx = nodes.len();
        nodes.push(json!({
            "name": "spring_joint_end",
            "translation": [
                spring_bone.chain_axis[0] * 0.07_f32,
                spring_bone.chain_axis[1] * 0.07_f32,
                spring_bone.chain_axis[2] * 0.07_f32,
            ],
        }));
        let leaf = *chain_nodes.last().unwrap();
        let mut leaf_children = nodes[leaf]
            .get("children")
            .and_then(|c| c.as_array())
            .cloned()
            .unwrap_or_default();
        leaf_children.push(json!(end_idx));
        nodes[leaf]["children"] = Value::Array(leaf_children);
        let mut v = chain_nodes.clone();
        v.push(end_idx);
        v
    } else {
        chain_nodes.clone()
    };

    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": [
            "KHR_materials_unlit",
            "VRMC_vrm",
            "VRMC_materials_mtoon",
            "VRMC_springBone"
        ],
        "extensionsRequired": ["VRMC_vrm"],
        "scene": 0,
        "scenes": [{ "nodes": [skeleton.root_node] }],
        "nodes": nodes,
        "meshes": [
            // mesh 0: head-mounted sphere
            {
                "name": format!("{}_geom", mtoon.id),
                "primitives": [{
                    "attributes": {
                        "POSITION": 0,
                        "NORMAL": 1,
                        "TEXCOORD_0": 2
                    },
                    "indices": 3,
                    "material": 0,
                    "mode": 4
                }]
            },
            // mesh 1: chain cylinder (skinned to spring-bone joints)
            {
                "name": format!("{}_chain_geom", mtoon.id),
                "primitives": [{
                    "attributes": {
                        "POSITION": 4,
                        "NORMAL": 5,
                        "TEXCOORD_0": 6,
                        "JOINTS_0": 8,
                        "WEIGHTS_0": 9
                    },
                    "indices": 7,
                    "material": 0,
                    "mode": 4
                }]
            }
        ],
        "skins": [{
            "joints": chain_nodes,
            "inverseBindMatrices": 10,
            "skeleton": chain_nodes[0]
        }],
        "materials": [base_material(mtoon)],
        "extensions": {
            "VRMC_vrm": vrmc_vrm_with_chain_mesh(
                &mtoon.id, &skeleton.bone_to_node, mesh_node_index, chain_mesh_node_index
            ),
            "VRMC_springBone": vrmc_spring_bone(&spring_joint_nodes, spring_bone),
        }
    });

    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, glb)?;
    Ok(())
}

/// VRMC_vrm helper that annotates BOTH the head mesh and the chain mesh
/// in `firstPerson.meshAnnotations`. Both use `type: "both"` so they're
/// unconditionally visible regardless of camera mode; the conformance
/// test plans are always third-person renders.
fn vrmc_vrm_with_chain_mesh(
    meta_name: &str,
    bone_to_node: &std::collections::BTreeMap<String, usize>,
    sphere_mesh_node: usize,
    chain_mesh_node: usize,
) -> Value {
    let mut ext = vrmc_vrm(meta_name, bone_to_node, sphere_mesh_node);
    ext["firstPerson"]["meshAnnotations"] = json!([
        { "node": sphere_mesh_node, "type": "both" },
        { "node": chain_mesh_node,  "type": "both" }
    ]);
    ext
}

/// Emits `<stem>.vrm` (MToon + spring-bone), `<stem>.meta.json` (with
/// spring-bone params), and `<stem>.test.yaml` from one MToonParams +
/// one SpringBoneParams pair.
pub fn emit_with_sidecars_spring_bone(
    mtoon: &MToonParams,
    spring_bone: &SpringBoneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone(mtoon, spring_bone, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = crate::sidecar::build_spring_bone_test_plan(mtoon, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emit a VRM **0.x** `.vrm` GLB carrying a `secondaryAnimation` spring-bone
/// chain (no sidecars).
///
/// The geometry is **identical** to [`emit_vrm_with_spring_bone`] — sphere mesh +
/// humanoid skeleton + spring chain nodes + skinned chain cylinder + inverse-bind
/// matrices via `pack_sphere_and_chain` — so the physics is visually observable
/// in pixel space. Only the material / extension layer changes:
///
/// - `extensionsUsed`: `["KHR_materials_unlit", "VRM"]`. Both primitives
///   reference `material: 0` — an unlit-only glTF material with MToon carried
///   in `VRM.materialProperties` (not in the glTF material's `extensions`
///   block, which would require `VRMC_materials_mtoon` in `extensionsUsed`
///   and violate the 0.x asset contract). `KHR_materials_unlit` is declared
///   because the glTF material uses it; `VRM` covers the 0.x extension block.
/// - `extensions`: `{ "VRM": … }` assembled by
///   [`crate::vrm_ext_v0::emit_vrm_extension_with_secondary`] with
///   `secondaryAnimation` built by
///   [`crate::spring_bone_v0::build_secondary_animation`].
/// - No `VRMC_vrm`, no `VRMC_springBone`, no `extensionsRequired` (0.x assets
///   have none by spec).
pub fn emit_vrm_with_spring_bone_v0(
    mtoon: &MToonParams,
    spring: &SpringBoneParams,
    output: &Utf8Path,
) -> Result<()> {
    // ── 1. Geometry assembly (mirror emit_vrm_with_spring_bone exactly) ──────
    let mesh = sphere(0.3, 24, 48);

    let mut skeleton = crate::humanoid::minimal_skeleton();
    let head_node = skeleton.bone_to_node["head"];
    let chain_nodes = crate::humanoid::append_spring_chain_axis(
        &mut skeleton,
        head_node,
        spring.joint_count,
        spring.segment_length_m,
        spring.chain_axis,
    );

    let head_world = crate::humanoid::rest_pose_world_position("head");
    // Joint 0 (chain top) = head + axis * segment_length.
    let chain_top = chain_joint_world(head_world, spring.chain_axis, spring.segment_length_m, 0);

    let chain_mesh = crate::chain_mesh::build_chain_cylinder(
        spring.joint_count,
        spring.segment_length_m,
        0.025,
        chain_top,
        spring.chain_axis,
        12,
    );

    // Inverse-bind matrices: joint i bind-pose world = head + axis*(i+1)*seg.
    let inv_bind: Vec<[f32; 16]> = (0..spring.joint_count)
        .map(|i| {
            let p = chain_joint_world(head_world, spring.chain_axis, spring.segment_length_m, i);
            inv_translation_mat4(p)
        })
        .collect();

    let packed = crate::buffer::pack_sphere_and_chain(&mesh, &chain_mesh, &inv_bind);

    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();

    // Sphere mesh node — child of head (identical to v1 wiring).
    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", mtoon.id),
        "mesh": 0
    }));
    let head = &mut nodes[head_node];
    let mut head_children = head["children"].as_array().cloned().unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

    // Chain-skinned node — child of hips so the scene stays single-rooted.
    let chain_mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_chain_mesh", mtoon.id),
        "mesh": 1,
        "skin": 0
    }));
    let hips = &mut nodes[skeleton.root_node];
    let mut hips_children = hips["children"].as_array().cloned().unwrap_or_default();
    hips_children.push(json!(chain_mesh_node_index));
    hips["children"] = Value::Array(hips_children);

    // ── 2. VRM 0.x extension block ───────────────────────────────────────────
    let empty_expressions = crate::expressions_v0::ExpressionsV0Params { groups: vec![] };
    let secondary = crate::spring_bone_v0::build_secondary_animation(spring, chain_nodes[0]);
    let vrm_ext = crate::vrm_ext_v0::emit_vrm_extension_with_secondary(
        &mtoon.id,
        &[mtoon.clone()],
        &empty_expressions,
        Some(secondary),
        &skeleton.bone_to_node,
    );

    // ── 3. glTF-level material (v0-compatible: KHR_materials_unlit only) ────
    //
    // `base_material` from vrm_ext.rs always embeds `VRMC_materials_mtoon` in
    // the material's `extensions` block. That extension must then appear in
    // `extensionsUsed`, which would violate the 0.x asset contract. Instead we
    // emit a minimal `KHR_materials_unlit` material here — the MToon parameters
    // are carried by `VRM.materialProperties` (via `emit_vrm_extension_with_secondary`
    // → `mtoon_v0::emit_material_property`) and the glTF material is only needed
    // so the mesh primitives have a valid `material` index.
    let v0_material = json!({
        "name": mtoon.id,
        "pbrMetallicRoughness": {
            "baseColorFactor": mtoon.base_color_factor,
            "metallicFactor": 0.0,
            "roughnessFactor": 0.9
        },
        "alphaMode": "OPAQUE",
        "doubleSided": mtoon.double_sided,
        "extensions": {
            "KHR_materials_unlit": {}
        }
    });

    // ── 4. glTF document (0.x: no extensionsRequired; KHR_materials_unlit
    //       is declared because our v0_material uses it; VRM for the ext) ────
    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": ["KHR_materials_unlit", "VRM"],
        "scene": 0,
        "scenes": [{ "nodes": [skeleton.root_node] }],
        "nodes": nodes,
        "meshes": [
            {
                "name": format!("{}_geom", mtoon.id),
                "primitives": [{
                    "attributes": {
                        "POSITION": 0,
                        "NORMAL": 1,
                        "TEXCOORD_0": 2
                    },
                    "indices": 3,
                    "material": 0,
                    "mode": 4
                }]
            },
            {
                "name": format!("{}_chain_geom", mtoon.id),
                "primitives": [{
                    "attributes": {
                        "POSITION": 4,
                        "NORMAL": 5,
                        "TEXCOORD_0": 6,
                        "JOINTS_0": 8,
                        "WEIGHTS_0": 9
                    },
                    "indices": 7,
                    "material": 0,
                    "mode": 4
                }]
            }
        ],
        "skins": [{
            "joints": chain_nodes,
            "inverseBindMatrices": 10,
            "skeleton": chain_nodes[0]
        }],
        "materials": [v0_material],
        "extensions": {
            "VRM": vrm_ext
        }
    });

    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    // ── 5. Write GLB ─────────────────────────────────────────────────────────
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;
    std::fs::write(output, glb)?;

    Ok(())
}

/// Emit a VRM **0.x** asset triplet carrying a `secondaryAnimation` spring-bone
/// chain: `<stem>.vrm`, `<stem>.meta.json`, `<stem>.test.yaml` (settle plan).
///
/// Calls [`emit_vrm_with_spring_bone_v0`] for the `.vrm`, then writes the
/// `.meta.json` and a settle `.test.yaml` tagged `spec_version: "0.x"` with
/// `physics: { settle_steps: 30 }` (via `build_spring_bone_test_plan`).
pub fn emit_with_sidecars_spring_bone_v0(
    mtoon: &MToonParams,
    spring: &SpringBoneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_v0(mtoon, spring, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(mtoon, Some(spring), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let mut plan = crate::sidecar::build_spring_bone_test_plan(mtoon, &asset_relpath);
    crate::sidecar::tag_plan_vrm0(&mut plan);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Same VRM body as [`emit_with_sidecars_spring_bone_v0`], but the emitted
/// `.test.yaml` carries an additional `animation.root_transform` block (swing
/// plan). Mirrors [`emit_with_sidecars_spring_bone_swing`] for the 0.x corpus.
///
/// The `.vrm` is produced by [`emit_vrm_with_spring_bone_v0`] — geometry and
/// extension layout are identical to the settle variant. Only the test plan
/// differs: `spec_version: "0.x"` and `animation.root_transform` present
/// (via `build_spring_bone_swing_test_plan`).
pub fn emit_with_sidecars_spring_bone_v0_swing(
    mtoon: &MToonParams,
    spring: &SpringBoneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_v0(mtoon, spring, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(mtoon, Some(spring), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let mut plan = crate::sidecar::build_spring_bone_swing_test_plan(mtoon, &asset_relpath);
    crate::sidecar::tag_plan_vrm0(&mut plan);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emit a VRM **0.x** `.vrm` GLB carrying a `secondaryAnimation` spring-bone
/// chain with **sphere colliders** (no sidecars).
///
/// Geometry is identical to [`emit_vrm_with_spring_bone_colliders`] (the v1
/// collider emit) — sphere mesh + humanoid skeleton + spring chain nodes +
/// skinned chain cylinder + inverse-bind matrices. Only the material/extension
/// layer differs:
///
/// - `extensionsUsed`: `["KHR_materials_unlit", "VRM"]`.
/// - Material: `v0_material` (unlit-only, no `VRMC_materials_mtoon`).
/// - `secondaryAnimation` built by
///   [`crate::spring_bone_v0::build_secondary_animation_with_colliders`] with
///   sphere-only resolved colliders (non-sphere colliders in `scene.colliders`
///   are silently skipped — they have no 0.x form).
/// - No `VRMC_vrm`, no `VRMC_springBone`, no `extensionsRequired`.
///
/// Collider attach resolution mirrors the v1 path exactly:
/// - `ColliderAttach::Head` → `head_node`.
/// - `ColliderAttach::NewIntermediateNode{y_offset, z_offset}` → a new glTF
///   node inserted as a child of head at `[0, y_offset, z_offset]`.
pub fn emit_vrm_with_spring_bone_colliders_v0(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    output: &Utf8Path,
) -> Result<()> {
    let spring = &scene.springs[0];
    let mesh = sphere(0.3, 24, 48);

    let mut skeleton = crate::humanoid::minimal_skeleton();
    let head_node = skeleton.bone_to_node["head"];
    let chain_nodes = crate::humanoid::append_spring_chain_axis(
        &mut skeleton,
        head_node,
        spring.joint_count,
        spring.segment_length_m,
        spring.chain_axis,
    );

    let head_world = crate::humanoid::rest_pose_world_position("head");
    // Joint 0 (chain top) = head + axis * segment_length.
    let chain_top = chain_joint_world(head_world, spring.chain_axis, spring.segment_length_m, 0);

    let chain_mesh = crate::chain_mesh::build_chain_cylinder(
        spring.joint_count,
        spring.segment_length_m,
        /* radius */ 0.025,
        chain_top,
        spring.chain_axis,
        /* ring_segments */ 12,
    );

    // Inverse-bind matrices: joint i bind-pose world = head + axis*(i+1)*seg.
    let inv_bind: Vec<[f32; 16]> = (0..spring.joint_count)
        .map(|i| {
            let p = chain_joint_world(head_world, spring.chain_axis, spring.segment_length_m, i);
            inv_translation_mat4(p)
        })
        .collect();

    let packed = crate::buffer::pack_sphere_and_chain(&mesh, &chain_mesh, &inv_bind);

    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();

    // Sphere mesh node — child of head (identical to v1 wiring).
    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", mtoon.id),
        "mesh": 0
    }));
    let head = &mut nodes[head_node];
    let mut head_children = head["children"].as_array().cloned().unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

    // Chain-skinned node — child of hips so the scene stays single-rooted.
    let chain_mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_chain_mesh", mtoon.id),
        "mesh": 1,
        "skin": 0
    }));
    let hips = &mut nodes[skeleton.root_node];
    let mut hips_children = hips["children"].as_array().cloned().unwrap_or_default();
    hips_children.push(json!(chain_mesh_node_index));
    hips["children"] = Value::Array(hips_children);

    // Resolve collider attach nodes (mirrors v1 path exactly) and build the
    // V0SphereCollider list for sphere-shape colliders only.
    let mut resolved_sphere_colliders: Vec<crate::spring_bone_v0::V0SphereCollider> =
        Vec::with_capacity(scene.colliders.len());

    for collider in &scene.colliders {
        // Filter to Sphere only — other shapes have no 0.x form.
        let radius = match &collider.shape {
            ColliderShape::Sphere { radius } => *radius,
            _ => continue, // skip Capsule, InsideSphere, InsideCapsule, Plane
        };

        let attach_node = match &collider.attach {
            ColliderAttach::Head => head_node,
            ColliderAttach::NewIntermediateNode { y_offset, z_offset } => {
                let new_node_idx = nodes.len();
                nodes.push(json!({
                    "name": format!("{}_collider_node_{}", mtoon.id, new_node_idx),
                    "translation": [0.0, y_offset, z_offset],
                }));
                // Parent under head.
                let head_ref = &mut nodes[head_node];
                let mut hc = head_ref["children"].as_array().cloned().unwrap_or_default();
                hc.push(json!(new_node_idx));
                head_ref["children"] = Value::Array(hc);
                new_node_idx
            }
        };

        resolved_sphere_colliders.push(crate::spring_bone_v0::V0SphereCollider {
            node: attach_node,
            offset: collider.offset,
            radius,
        });
    }

    // Build the VRM 0.x extension block with sphere colliders.
    let empty_expressions = crate::expressions_v0::ExpressionsV0Params { groups: vec![] };
    let secondary = crate::spring_bone_v0::build_secondary_animation_with_colliders(
        spring,
        chain_nodes[0],
        &resolved_sphere_colliders,
    );
    let vrm_ext = crate::vrm_ext_v0::emit_vrm_extension_with_secondary(
        &mtoon.id,
        &[mtoon.clone()],
        &empty_expressions,
        Some(secondary),
        &skeleton.bone_to_node,
    );

    // v0-compatible glTF material: KHR_materials_unlit only, no VRMC_materials_mtoon.
    let v0_material = json!({
        "name": mtoon.id,
        "pbrMetallicRoughness": {
            "baseColorFactor": mtoon.base_color_factor,
            "metallicFactor": 0.0,
            "roughnessFactor": 0.9
        },
        "alphaMode": "OPAQUE",
        "doubleSided": mtoon.double_sided,
        "extensions": {
            "KHR_materials_unlit": {}
        }
    });

    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": ["KHR_materials_unlit", "VRM"],
        "scene": 0,
        "scenes": [{ "nodes": [skeleton.root_node] }],
        "nodes": nodes,
        "meshes": [
            {
                "name": format!("{}_geom", mtoon.id),
                "primitives": [{
                    "attributes": {
                        "POSITION": 0,
                        "NORMAL": 1,
                        "TEXCOORD_0": 2
                    },
                    "indices": 3,
                    "material": 0,
                    "mode": 4
                }]
            },
            {
                "name": format!("{}_chain_geom", mtoon.id),
                "primitives": [{
                    "attributes": {
                        "POSITION": 4,
                        "NORMAL": 5,
                        "TEXCOORD_0": 6,
                        "JOINTS_0": 8,
                        "WEIGHTS_0": 9
                    },
                    "indices": 7,
                    "material": 0,
                    "mode": 4
                }]
            }
        ],
        "skins": [{
            "joints": chain_nodes,
            "inverseBindMatrices": 10,
            "skeleton": chain_nodes[0]
        }],
        "materials": [v0_material],
        "extensions": {
            "VRM": vrm_ext
        }
    });

    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;
    std::fs::write(output, glb)?;

    Ok(())
}

/// Emits `<stem>.vrm` (MToon + spring-bone with sphere colliders, VRM 0.x),
/// `<stem>.meta.json`, and `<stem>.test.yaml` (settle variant, 60-step settle,
/// `spec_version: "0.x"`).
pub fn emit_with_sidecars_spring_bone_colliders_v0(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_colliders_v0(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring = &scene.springs[0];
    write_meta_json(mtoon, Some(spring), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let mut plan =
        crate::sidecar::build_spring_bone_collider_test_plan(mtoon, scene, &asset_relpath);
    crate::sidecar::tag_plan_vrm0(&mut plan);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Same as [`emit_with_sidecars_spring_bone_colliders_v0`] but the test plan
/// carries an `animation.root_transform` block (swing variant, `spec_version:
/// "0.x"`).
pub fn emit_with_sidecars_spring_bone_colliders_v0_swing(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_colliders_v0(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring = &scene.springs[0];
    write_meta_json(mtoon, Some(spring), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let mut plan =
        crate::sidecar::build_spring_bone_collider_swing_test_plan(mtoon, scene, &asset_relpath);
    crate::sidecar::tag_plan_vrm0(&mut plan);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Same VRM body as `emit_with_sidecars_spring_bone`, but the emitted
/// `.test.yaml` carries an additional `animation.root_transform` block.
/// The runner will settle the chain, then translate the root sideways
/// 15 cm over 0.25 s before rendering — capturing the chain mid-swing
/// rather than at the static settle. See
/// `build_spring_bone_swing_test_plan` for the rationale on the numbers.
pub fn emit_with_sidecars_spring_bone_swing(
    mtoon: &MToonParams,
    spring_bone: &SpringBoneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone(mtoon, spring_bone, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = crate::sidecar::build_spring_bone_swing_test_plan(mtoon, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Same VRM body as `emit_with_sidecars_spring_bone_swing`, but the
/// `.test.yaml` carries a `render_sequence:` block (sequence-mode) instead
/// of `animation: { root_transform }` (single-frame mode). Used by the
/// `emit-sequence-sweep` CLI subcommand.
pub fn emit_with_sidecars_spring_bone_swing_sequence(
    mtoon: &MToonParams,
    spring_bone: &SpringBoneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone(mtoon, spring_bone, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = crate::sidecar::build_spring_bone_swing_sequence_test_plan(mtoon, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// 0.x sequence-mode spring-bone triplet: same `.vrm` as the v0 settle path
/// (no animation in the asset), but the `.test.yaml` carries a `render_sequence`
/// block. Mirrors `emit_with_sidecars_spring_bone_swing_sequence` over the v0 emit.
pub fn emit_with_sidecars_spring_bone_v0_sequence(
    mtoon: &MToonParams,
    spring: &SpringBoneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_v0(mtoon, spring, &vrm_path)?;
    let meta_path = stem.with_extension("meta.json");
    write_meta_json(mtoon, Some(spring), &vrm_path, &meta_path)?;
    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let mut plan =
        crate::sidecar::build_spring_bone_swing_sequence_test_plan(mtoon, &asset_relpath);
    crate::sidecar::tag_plan_vrm0(&mut plan);
    write_test_yaml(&plan, &yaml_path)?;
    Ok(())
}

/// Emit a `.vrm` GLB identical in structure to `emit_vrm_with_spring_bone`
/// but with VRMC_springBone colliders declared in the extension.
///
/// For `ColliderAttach::Head`, the collider node is the head node index.
/// For `ColliderAttach::NewIntermediateNode { y_offset, z_offset }`, a new
/// glTF node is inserted as a child of head at the given local offset and
/// its index is used.
///
/// NOTE: factor shared chain setup with `emit_vrm_with_spring_bone` in phase 6
/// multi-chain refactor; for phase 2 we accept the duplication.
pub fn emit_vrm_with_spring_bone_colliders(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    output: &Utf8Path,
) -> Result<()> {
    let spring_bone = &scene.springs[0];
    let mesh = sphere(0.3, 24, 48);

    let mut skeleton = crate::humanoid::minimal_skeleton();
    let head_node = skeleton.bone_to_node["head"];
    let chain_nodes = crate::humanoid::append_spring_chain_axis(
        &mut skeleton,
        head_node,
        spring_bone.joint_count,
        spring_bone.segment_length_m,
        spring_bone.chain_axis,
    );

    let head_world = crate::humanoid::rest_pose_world_position("head");
    // Joint 0 (chain top) = head + axis * segment_length.
    let chain_top = chain_joint_world(
        head_world,
        spring_bone.chain_axis,
        spring_bone.segment_length_m,
        0,
    );

    let chain_mesh = crate::chain_mesh::build_chain_cylinder(
        spring_bone.joint_count,
        spring_bone.segment_length_m,
        /* radius */ 0.025,
        chain_top,
        spring_bone.chain_axis,
        /* ring_segments */ 12,
    );

    // Inverse-bind matrices: joint i bind-pose world = head + axis*(i+1)*seg.
    let inv_bind: Vec<[f32; 16]> = (0..spring_bone.joint_count)
        .map(|i| {
            let p = chain_joint_world(
                head_world,
                spring_bone.chain_axis,
                spring_bone.segment_length_m,
                i,
            );
            inv_translation_mat4(p)
        })
        .collect();

    let packed = crate::buffer::pack_sphere_and_chain(&mesh, &chain_mesh, &inv_bind);

    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();

    // Sphere mesh node — child of head.
    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", mtoon.id),
        "mesh": 0
    }));
    let head = &mut nodes[head_node];
    let mut head_children = head["children"].as_array().cloned().unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

    // Chain-skinned node — child of hips.
    let chain_mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_chain_mesh", mtoon.id),
        "mesh": 1,
        "skin": 0
    }));
    let hips = &mut nodes[skeleton.root_node];
    let mut hips_children = hips["children"].as_array().cloned().unwrap_or_default();
    hips_children.push(json!(chain_mesh_node_index));
    hips["children"] = Value::Array(hips_children);

    // Resolve collider attach nodes. For Head attach, use head_node.
    // For NewIntermediateNode, add a new glTF node parented under head.
    let mut collider_attach_nodes: Vec<usize> = Vec::with_capacity(scene.colliders.len());
    for collider in &scene.colliders {
        match &collider.attach {
            ColliderAttach::Head => {
                collider_attach_nodes.push(head_node);
            }
            ColliderAttach::NewIntermediateNode { y_offset, z_offset } => {
                let new_node_idx = nodes.len();
                nodes.push(json!({
                    "name": format!("{}_collider_node_{}", mtoon.id, new_node_idx),
                    "translation": [0.0, y_offset, z_offset],
                }));
                // Parent under head
                let head_node_ref = &mut nodes[head_node];
                let mut hc = head_node_ref["children"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                hc.push(json!(new_node_idx));
                head_node_ref["children"] = Value::Array(hc);
                collider_attach_nodes.push(new_node_idx);
            }
        }
    }

    // Build extensionsUsed: always declare springBone; add extended collider
    // extension only when the scene actually uses it.
    let mut extensions_used = vec![
        "KHR_materials_unlit",
        "VRMC_vrm",
        "VRMC_materials_mtoon",
        "VRMC_springBone",
    ];
    if scene_uses_extended_collider(scene) {
        extensions_used.push("VRMC_springBone_extended_collider");
    }

    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": extensions_used,
        "extensionsRequired": ["VRMC_vrm"],
        "scene": 0,
        "scenes": [{ "nodes": [skeleton.root_node] }],
        "nodes": nodes,
        "meshes": [
            {
                "name": format!("{}_geom", mtoon.id),
                "primitives": [{
                    "attributes": {
                        "POSITION": 0,
                        "NORMAL": 1,
                        "TEXCOORD_0": 2
                    },
                    "indices": 3,
                    "material": 0,
                    "mode": 4
                }]
            },
            {
                "name": format!("{}_chain_geom", mtoon.id),
                "primitives": [{
                    "attributes": {
                        "POSITION": 4,
                        "NORMAL": 5,
                        "TEXCOORD_0": 6,
                        "JOINTS_0": 8,
                        "WEIGHTS_0": 9
                    },
                    "indices": 7,
                    "material": 0,
                    "mode": 4
                }]
            }
        ],
        "skins": [{
            "joints": chain_nodes,
            "inverseBindMatrices": 10,
            "skeleton": chain_nodes[0]
        }],
        "materials": [base_material(mtoon)],
        "extensions": {
            "VRMC_vrm": vrmc_vrm_with_chain_mesh(
                &mtoon.id, &skeleton.bone_to_node, mesh_node_index, chain_mesh_node_index
            ),
            "VRMC_springBone": vrmc_spring_bone_scene(&chain_nodes, scene, &collider_attach_nodes),
        }
    });

    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, glb)?;
    Ok(())
}

/// Emits `<stem>.vrm` (MToon + spring-bone with colliders), `<stem>.meta.json`,
/// and `<stem>.test.yaml` (settle variant, 60-step settle).
pub fn emit_with_sidecars_spring_bone_colliders(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_colliders(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring_bone = &scene.springs[0];
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = crate::sidecar::build_spring_bone_collider_test_plan(mtoon, scene, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Same as `emit_with_sidecars_spring_bone_colliders` but the test plan
/// carries an `animation.root_transform` block (swing variant).
pub fn emit_with_sidecars_spring_bone_colliders_swing(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_colliders(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring_bone = &scene.springs[0];
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan =
        crate::sidecar::build_spring_bone_collider_swing_test_plan(mtoon, scene, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emits `<stem>.vrm` (MToon + spring-bone with extended colliders),
/// `<stem>.meta.json`, and `<stem>.test.yaml` (settle variant, 60-step settle).
/// Uses `VRMC_springBone_extended_collider` extension shapes/angle limits.
pub fn emit_with_sidecars_spring_bone_extended(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_colliders(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring_bone = &scene.springs[0];
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = crate::sidecar::build_spring_bone_extended_test_plan(mtoon, scene, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Same as `emit_with_sidecars_spring_bone_extended` but the test plan
/// carries an `animation.root_transform` block (swing variant).
pub fn emit_with_sidecars_spring_bone_extended_swing(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_colliders(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring_bone = &scene.springs[0];
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan =
        crate::sidecar::build_spring_bone_extended_swing_test_plan(mtoon, scene, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emit a `.vrm` GLB with N parallel spring-bone chains.
///
/// Each chain gets its own intermediate node radially spaced around the head bone
/// (angles 0°, 360°/N, 2·360°/N, …, in the XZ plane at a fixed radial distance of
/// 0.05 m from the head axis). Each chain hangs straight down from its intermediate
/// node and is skinned by its own cylinder mesh.
///
/// Structure:
/// - 1 sphere mesh (head-mounted, parented to head — the MToon reference shape)
/// - N chain cylinder meshes (each skinned to its chain's joints)
/// - N skins (one per chain)
/// - VRMC_springBone extension with N springs entries
pub fn emit_vrm_with_spring_bone_multichain(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    output: &Utf8Path,
) -> Result<()> {
    let n_chains = scene.springs.len();
    assert!(n_chains >= 1, "multichain emit needs at least 1 chain");

    let mesh = sphere(0.3, 24, 48);

    let mut skeleton = crate::humanoid::minimal_skeleton();
    let head_node = skeleton.bone_to_node["head"];
    let head_world = crate::humanoid::rest_pose_world_position("head");

    // Radial spacing: chains are placed at radius CHAIN_RADIAL_M from the head axis.
    // For n_chains=1 this puts the single chain at angle 0 (same as the single-chain emit).
    const CHAIN_RADIAL_M: f32 = 0.05;

    let mut chain_joint_nodes: Vec<Vec<usize>> = Vec::with_capacity(n_chains);
    let mut chain_meshes: Vec<crate::chain_mesh::SkinnedMeshData> = Vec::with_capacity(n_chains);
    let mut inv_binds: Vec<Vec<[f32; 16]>> = Vec::with_capacity(n_chains);

    for (c_idx, spring_params) in scene.springs.iter().enumerate() {
        // Radial angle for this chain.
        let angle = (c_idx as f32) * 2.0 * std::f32::consts::PI / (n_chains as f32);
        let (sin_a, cos_a) = angle.sin_cos();
        let rx = CHAIN_RADIAL_M * sin_a;
        let rz = CHAIN_RADIAL_M * cos_a;

        // Intermediate node: child of head, offset radially so each chain hangs
        // from a distinct XZ position.
        let nodes = skeleton.nodes_json.as_array_mut().unwrap();
        let inter_idx = nodes.len();
        nodes.push(json!({
            "name": format!("{}_chain{}_inter", mtoon.id, c_idx),
            "translation": [rx, 0.0, rz],
        }));
        // Wire intermediate under head.
        let head_ref = nodes.get_mut(head_node).unwrap();
        let mut hc = head_ref["children"].as_array().cloned().unwrap_or_default();
        hc.push(json!(inter_idx));
        head_ref["children"] = Value::Array(hc);

        // Append chain joints as children of the intermediate node.
        let chain_nodes = crate::humanoid::append_spring_chain_axis(
            &mut skeleton,
            inter_idx,
            spring_params.joint_count,
            spring_params.segment_length_m,
            spring_params.chain_axis,
        );

        // Chain cylinder: top_world is head_world + axis * segment_length.
        // Uses head_world (not the intermediate node world) so the default -Y
        // path produces byte-identical output: the head bone has x=z=0, and
        // the existing IBM zeroed those entries for the same reason.
        let chain_top = chain_joint_world(
            head_world,
            spring_params.chain_axis,
            spring_params.segment_length_m,
            0,
        );
        let chain_mesh = crate::chain_mesh::build_chain_cylinder(
            spring_params.joint_count,
            spring_params.segment_length_m,
            0.025,
            chain_top,
            spring_params.chain_axis,
            12,
        );

        // Inverse-bind matrices: joint i bind-pose world = head + axis*(i+1)*seg.
        // See note on chain_top above for why head_world is the root.
        let ibm: Vec<[f32; 16]> = (0..spring_params.joint_count)
            .map(|i| {
                let p = chain_joint_world(
                    head_world,
                    spring_params.chain_axis,
                    spring_params.segment_length_m,
                    i,
                );
                inv_translation_mat4(p)
            })
            .collect();

        chain_joint_nodes.push(chain_nodes);
        chain_meshes.push(chain_mesh);
        inv_binds.push(ibm);
    }

    // Pack all geometry into a single buffer.
    let chains_for_pack: Vec<(&crate::chain_mesh::SkinnedMeshData, &[[f32; 16]])> = chain_meshes
        .iter()
        .zip(inv_binds.iter())
        .map(|(cm, ibm)| (cm, ibm.as_slice()))
        .collect();
    let packed = pack_sphere_and_multichains(&mesh, &chains_for_pack);

    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();

    // Sphere mesh node: child of head.
    let sphere_mesh_node = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", mtoon.id),
        "mesh": 0
    }));
    let head_ref = &mut nodes[head_node];
    let mut hc = head_ref["children"].as_array().cloned().unwrap_or_default();
    hc.push(json!(sphere_mesh_node));
    head_ref["children"] = Value::Array(hc);

    // Chain mesh nodes: child of hips (so scene stays single-rooted).
    // Mesh index 1..=N, skin index 0..N-1.
    let mut chain_mesh_nodes: Vec<usize> = Vec::with_capacity(n_chains);
    for c_idx in 0..n_chains {
        let chain_mesh_node = nodes.len();
        nodes.push(json!({
            "name": format!("{}_chain{}_mesh", mtoon.id, c_idx),
            "mesh": 1 + c_idx,
            "skin": c_idx
        }));
        let hips_ref = &mut nodes[skeleton.root_node];
        let mut hips_children = hips_ref["children"].as_array().cloned().unwrap_or_default();
        hips_children.push(json!(chain_mesh_node));
        hips_ref["children"] = Value::Array(hips_children);
        chain_mesh_nodes.push(chain_mesh_node);
    }

    // Resolve collider attach nodes.
    let mut collider_attach_nodes: Vec<usize> = Vec::with_capacity(scene.colliders.len());
    for collider in &scene.colliders {
        match &collider.attach {
            ColliderAttach::Head => {
                collider_attach_nodes.push(head_node);
            }
            ColliderAttach::NewIntermediateNode { y_offset, z_offset } => {
                let new_node_idx = nodes.len();
                nodes.push(json!({
                    "name": format!("{}_collider_node_{}", mtoon.id, new_node_idx),
                    "translation": [0.0, y_offset, z_offset],
                }));
                let head_ref = &mut nodes[head_node];
                let mut hc = head_ref["children"].as_array().cloned().unwrap_or_default();
                hc.push(json!(new_node_idx));
                head_ref["children"] = Value::Array(hc);
                collider_attach_nodes.push(new_node_idx);
            }
        }
    }

    // extensionsUsed.
    let mut extensions_used = vec![
        "KHR_materials_unlit",
        "VRMC_vrm",
        "VRMC_materials_mtoon",
        "VRMC_springBone",
    ];
    if scene_uses_extended_collider(scene) {
        extensions_used.push("VRMC_springBone_extended_collider");
    }

    // Meshes: sphere (index 0) + N chain cylinders (index 1..N).
    // Accessor base for chain i: 4 + i*7.
    let mut meshes: Vec<Value> = vec![json!({
        "name": format!("{}_geom", mtoon.id),
        "primitives": [{
            "attributes": { "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2 },
            "indices": 3,
            "material": 0,
            "mode": 4
        }]
    })];
    for c_idx in 0..n_chains {
        let base = 4 + c_idx * 7;
        meshes.push(json!({
            "name": format!("{}_chain{}_geom", mtoon.id, c_idx),
            "primitives": [{
                "attributes": {
                    "POSITION": base,
                    "NORMAL": base + 1,
                    "TEXCOORD_0": base + 2,
                    "JOINTS_0": base + 4,
                    "WEIGHTS_0": base + 5
                },
                "indices": base + 3,
                "material": 0,
                "mode": 4
            }]
        }));
    }

    // Skins: one per chain.  inverseBindMatrices accessor index = 4 + i*7 + 6.
    let skins: Vec<Value> = (0..n_chains)
        .map(|c_idx| {
            let ibm_acc = 4 + c_idx * 7 + 6;
            json!({
                "joints": chain_joint_nodes[c_idx],
                "inverseBindMatrices": ibm_acc,
                "skeleton": chain_joint_nodes[c_idx][0]
            })
        })
        .collect();

    // VRMC_vrm firstPerson: annotate sphere + all chain mesh nodes.
    let mut all_mesh_nodes: Vec<Value> = vec![json!({ "node": sphere_mesh_node, "type": "both" })];
    for &cmn in &chain_mesh_nodes {
        all_mesh_nodes.push(json!({ "node": cmn, "type": "both" }));
    }
    let mut vrm_ext = vrmc_vrm(&mtoon.id, &skeleton.bone_to_node, sphere_mesh_node);
    vrm_ext["firstPerson"]["meshAnnotations"] = Value::Array(all_mesh_nodes);

    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": extensions_used,
        "extensionsRequired": ["VRMC_vrm"],
        "scene": 0,
        "scenes": [{ "nodes": [skeleton.root_node] }],
        "nodes": nodes,
        "meshes": meshes,
        "skins": skins,
        "materials": [base_material(mtoon)],
        "extensions": {
            "VRMC_vrm": vrm_ext,
            "VRMC_springBone": crate::vrm_ext::vrmc_spring_bone_scene_multichain(
                &chain_joint_nodes,
                scene,
                &collider_attach_nodes,
            ),
        }
    });

    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, glb)?;
    Ok(())
}

/// Emit a VRM **0.x** `.vrm` GLB with N parallel spring-bone chains
/// (`secondaryAnimation.boneGroups`).
///
/// The geometry is **identical** to [`emit_vrm_with_spring_bone_multichain`] —
/// sphere mesh + humanoid skeleton + N intermediate nodes (radially placed under
/// head) + N chain node trees + N skinned cylinder meshes + N skins packed via
/// `pack_sphere_and_multichains`. Only the material/extension layer changes:
///
/// - `extensionsUsed`: `["KHR_materials_unlit", "VRM"]`.
/// - Material: `v0_material` (unlit-only, no `VRMC_materials_mtoon`).
/// - `extensions.VRM` assembled by
///   [`crate::vrm_ext_v0::emit_vrm_extension_with_secondary`] with
///   `secondaryAnimation` built by
///   [`crate::spring_bone_v0::build_secondary_animation_multi`].
/// - No `VRMC_vrm`, no `VRMC_springBone`, no `extensionsRequired`.
pub fn emit_vrm_with_spring_bone_multichain_v0(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    output: &Utf8Path,
) -> Result<()> {
    let n_chains = scene.springs.len();
    assert!(n_chains >= 1, "multichain v0 emit needs at least 1 chain");

    let mesh = sphere(0.3, 24, 48);

    let mut skeleton = crate::humanoid::minimal_skeleton();
    let head_node = skeleton.bone_to_node["head"];
    let head_world = crate::humanoid::rest_pose_world_position("head");

    const CHAIN_RADIAL_M: f32 = 0.05;

    let mut chain_joint_nodes: Vec<Vec<usize>> = Vec::with_capacity(n_chains);
    let mut chain_meshes: Vec<crate::chain_mesh::SkinnedMeshData> = Vec::with_capacity(n_chains);
    let mut inv_binds: Vec<Vec<[f32; 16]>> = Vec::with_capacity(n_chains);

    for (c_idx, spring_params) in scene.springs.iter().enumerate() {
        let angle = (c_idx as f32) * 2.0 * std::f32::consts::PI / (n_chains as f32);
        let (sin_a, cos_a) = angle.sin_cos();
        let rx = CHAIN_RADIAL_M * sin_a;
        let rz = CHAIN_RADIAL_M * cos_a;

        let nodes = skeleton.nodes_json.as_array_mut().unwrap();
        let inter_idx = nodes.len();
        nodes.push(json!({
            "name": format!("{}_chain{}_inter", mtoon.id, c_idx),
            "translation": [rx, 0.0, rz],
        }));
        let head_ref = nodes.get_mut(head_node).unwrap();
        let mut hc = head_ref["children"].as_array().cloned().unwrap_or_default();
        hc.push(json!(inter_idx));
        head_ref["children"] = Value::Array(hc);

        let chain_nodes = crate::humanoid::append_spring_chain_axis(
            &mut skeleton,
            inter_idx,
            spring_params.joint_count,
            spring_params.segment_length_m,
            spring_params.chain_axis,
        );

        // Chain cylinder: top_world is head_world + axis * segment_length.
        // Uses head_world (not the intermediate node world) so the default -Y
        // path produces byte-identical output: the head bone has x=z=0, and
        // the existing IBM zeroed those entries for the same reason.
        let chain_top = chain_joint_world(
            head_world,
            spring_params.chain_axis,
            spring_params.segment_length_m,
            0,
        );
        let chain_mesh = crate::chain_mesh::build_chain_cylinder(
            spring_params.joint_count,
            spring_params.segment_length_m,
            0.025,
            chain_top,
            spring_params.chain_axis,
            12,
        );

        // Inverse-bind matrices: joint i bind-pose world = head + axis*(i+1)*seg.
        // See note on chain_top above for why head_world is the root.
        let ibm: Vec<[f32; 16]> = (0..spring_params.joint_count)
            .map(|i| {
                let p = chain_joint_world(
                    head_world,
                    spring_params.chain_axis,
                    spring_params.segment_length_m,
                    i,
                );
                inv_translation_mat4(p)
            })
            .collect();

        chain_joint_nodes.push(chain_nodes);
        chain_meshes.push(chain_mesh);
        inv_binds.push(ibm);
    }

    let chains_for_pack: Vec<(&crate::chain_mesh::SkinnedMeshData, &[[f32; 16]])> = chain_meshes
        .iter()
        .zip(inv_binds.iter())
        .map(|(cm, ibm)| (cm, ibm.as_slice()))
        .collect();
    let packed = pack_sphere_and_multichains(&mesh, &chains_for_pack);

    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();

    // Sphere mesh node — child of head.
    let sphere_mesh_node = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", mtoon.id),
        "mesh": 0
    }));
    let head_ref = &mut nodes[head_node];
    let mut hc = head_ref["children"].as_array().cloned().unwrap_or_default();
    hc.push(json!(sphere_mesh_node));
    head_ref["children"] = Value::Array(hc);

    // Chain mesh nodes — children of hips (scene stays single-rooted).
    for c_idx in 0..n_chains {
        let chain_mesh_node = nodes.len();
        nodes.push(json!({
            "name": format!("{}_chain{}_mesh", mtoon.id, c_idx),
            "mesh": 1 + c_idx,
            "skin": c_idx
        }));
        let hips_ref = &mut nodes[skeleton.root_node];
        let mut hips_children = hips_ref["children"].as_array().cloned().unwrap_or_default();
        hips_children.push(json!(chain_mesh_node));
        hips_ref["children"] = Value::Array(hips_children);
    }

    // Collect the per-chain first-bone node indices (root of each chain).
    let per_chain_first_nodes: Vec<usize> =
        chain_joint_nodes.iter().map(|chain| chain[0]).collect();

    // Meshes: sphere (index 0) + N chain cylinders (index 1..N).
    let mut meshes: Vec<Value> = vec![json!({
        "name": format!("{}_geom", mtoon.id),
        "primitives": [{
            "attributes": { "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2 },
            "indices": 3,
            "material": 0,
            "mode": 4
        }]
    })];
    for c_idx in 0..n_chains {
        let base = 4 + c_idx * 7;
        meshes.push(json!({
            "name": format!("{}_chain{}_geom", mtoon.id, c_idx),
            "primitives": [{
                "attributes": {
                    "POSITION": base,
                    "NORMAL": base + 1,
                    "TEXCOORD_0": base + 2,
                    "JOINTS_0": base + 4,
                    "WEIGHTS_0": base + 5
                },
                "indices": base + 3,
                "material": 0,
                "mode": 4
            }]
        }));
    }

    // Skins: one per chain. inverseBindMatrices accessor index = 4 + i*7 + 6.
    let skins: Vec<Value> = (0..n_chains)
        .map(|c_idx| {
            let ibm_acc = 4 + c_idx * 7 + 6;
            json!({
                "joints": chain_joint_nodes[c_idx],
                "inverseBindMatrices": ibm_acc,
                "skeleton": chain_joint_nodes[c_idx][0]
            })
        })
        .collect();

    // VRM 0.x extension block.
    let empty_expressions = crate::expressions_v0::ExpressionsV0Params { groups: vec![] };
    let secondary = crate::spring_bone_v0::build_secondary_animation_multi(
        &scene.springs,
        &per_chain_first_nodes,
    );
    let vrm_ext = crate::vrm_ext_v0::emit_vrm_extension_with_secondary(
        &mtoon.id,
        &[mtoon.clone()],
        &empty_expressions,
        Some(secondary),
        &skeleton.bone_to_node,
    );

    // v0-compatible glTF material: KHR_materials_unlit only, no VRMC_materials_mtoon.
    let v0_material = json!({
        "name": mtoon.id,
        "pbrMetallicRoughness": {
            "baseColorFactor": mtoon.base_color_factor,
            "metallicFactor": 0.0,
            "roughnessFactor": 0.9
        },
        "alphaMode": "OPAQUE",
        "doubleSided": mtoon.double_sided,
        "extensions": {
            "KHR_materials_unlit": {}
        }
    });

    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": ["KHR_materials_unlit", "VRM"],
        "scene": 0,
        "scenes": [{ "nodes": [skeleton.root_node] }],
        "nodes": nodes,
        "meshes": meshes,
        "skins": skins,
        "materials": [v0_material],
        "extensions": {
            "VRM": vrm_ext
        }
    });

    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;
    std::fs::write(output, glb)?;
    Ok(())
}

/// Emits `<stem>.vrm` (VRM 0.x multi-chain spring-bone), `<stem>.meta.json`,
/// and `<stem>.test.yaml` (settle variant, `spec_version: "0.x"`).
pub fn emit_with_sidecars_spring_bone_multichain_v0(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_multichain_v0(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring_bone = &scene.springs[0];
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let mut plan =
        crate::sidecar::build_spring_bone_multichain_test_plan(mtoon, scene, &asset_relpath);
    crate::sidecar::tag_plan_vrm0(&mut plan);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Same as [`emit_with_sidecars_spring_bone_multichain_v0`] but the `.test.yaml`
/// carries an `animation.root_transform` block (swing variant, `spec_version: "0.x"`).
pub fn emit_with_sidecars_spring_bone_multichain_v0_swing(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_multichain_v0(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring_bone = &scene.springs[0];
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let mut plan =
        crate::sidecar::build_spring_bone_multichain_swing_test_plan(mtoon, scene, &asset_relpath);
    crate::sidecar::tag_plan_vrm0(&mut plan);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emits `<stem>.vrm` (MToon + multi-chain spring-bone), `<stem>.meta.json`,
/// and `<stem>.test.yaml` (settle variant).
pub fn emit_with_sidecars_spring_bone_multichain(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_multichain(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring_bone = &scene.springs[0];
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = crate::sidecar::build_spring_bone_multichain_test_plan(mtoon, scene, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Same as `emit_with_sidecars_spring_bone_multichain` but the test plan
/// carries an `animation.root_transform` block (swing variant).
pub fn emit_with_sidecars_spring_bone_multichain_swing(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_multichain(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring_bone = &scene.springs[0];
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan =
        crate::sidecar::build_spring_bone_multichain_swing_test_plan(mtoon, scene, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emit a humanoid-sweep triplet: .vrm + .vrma + .test.yaml.
///
/// The .vrm is the canonical minimal humanoid rig (same as emit_default).
/// The .vrma carries a single-bone rotation animation; the .test.yaml
/// declares `animation.vrma` so the runner drives the VRMA op sequence
/// against any adapter.
pub fn emit_vrma_humanoid_triplet(
    output_dir: &Utf8Path,
    params: &crate::vrma_params::VrmaHumanoidParams,
) -> Result<()> {
    use crate::vrma_emit::{
        add_humanoid_bone_rotation_channel, build_empty_vrma, finalize_vrma_scenes,
        register_all_humanoid_bones, write_vrma_glb,
    };
    use crate::vrma_params::RotationAxis;

    std::fs::create_dir_all(output_dir)?;

    // 1. Emit the .vrm (canonical default avatar — the .vrma carries the test signal).
    let vrm_relpath = format!("{}.vrm", params.id);
    let vrm_path = output_dir.join(&vrm_relpath);
    let mtoon_defaults = crate::params::MToonParams::defaults(&params.id);
    emit_vrm(&mtoon_defaults, &vrm_path)?;

    // 2. Emit the .vrma.
    let skel = crate::humanoid::minimal_skeleton();
    let node_idx = *skel
        .bone_to_node
        .get(&params.bone_name)
        .unwrap_or_else(|| panic!("bone {} not in canonical skeleton", params.bone_name));

    let mut doc = build_empty_vrma();
    // Populate doc.nodes with the canonical skeleton's nodes (so node
    // indices in humanBones resolve).
    doc["nodes"] = skel.nodes_json.clone();

    // Declare all bones in humanBones so UniVRM 0.131.0 can build a valid
    // Unity HumanAvatar (requires at minimum hips + limb bones). Without
    // this the Avatar is invalid, AssignBonesFromAnimator returns false,
    // BoxMan is null, and TransferOwnership panics.
    register_all_humanoid_bones(&mut doc, &skel.bone_to_node);

    let mut buffer = Vec::<u8>::new();
    let half_rad = params.angle_deg.to_radians() / 2.0;
    let sin_h = half_rad.sin();
    let target_quat = match params.axis {
        RotationAxis::X => [sin_h, 0.0, 0.0, half_rad.cos()],
        RotationAxis::Y => [0.0, sin_h, 0.0, half_rad.cos()],
        RotationAxis::Z => [0.0, 0.0, sin_h, half_rad.cos()],
    };
    let keyframes = [
        (0.0_f32, [0.0_f32, 0.0, 0.0, 1.0]),
        (params.duration_s, target_quat),
    ];
    add_humanoid_bone_rotation_channel(
        &mut doc,
        &mut buffer,
        node_idx,
        &params.bone_name,
        &keyframes,
    );

    // Populate scenes[0].nodes with root-level nodes before serialising.
    // Required by UniVRM 0.131.0 VrmAnimationImporter which unconditionally
    // accesses scenes[0] at LoadAsync line 245.
    finalize_vrma_scenes(&mut doc);

    let vrma_relpath = format!("{}.vrma", params.id);
    let vrma_path = output_dir.join(&vrma_relpath);
    let vrma_bytes = write_vrma_glb(&doc, &buffer)?;
    std::fs::write(&vrma_path, &vrma_bytes)?;

    // 3. Emit the .test.yaml.
    let plan = crate::sidecar::build_vrma_humanoid_test_plan(params, &vrm_relpath, &vrma_relpath);
    let yaml_path = output_dir.join(format!("{}.test.yaml", params.id));
    crate::sidecar::write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emit a VRMA expression sweep triplet: .vrm + .vrma + .test.yaml.
///
/// The .vrm is the canonical minimal humanoid rig.
/// The .vrma carries a single-expression weight ramp: 0 → 1 → 0 over `duration_s`.
/// The .test.yaml declares `animation.vrma` with `apply_at_time = duration_s / 2`
/// so the runner samples at peak weight.
pub fn emit_vrma_expression_triplet(
    output_dir: &Utf8Path,
    params: &crate::vrma_params::VrmaExpressionParams,
) -> Result<()> {
    use crate::vrma_emit::{
        add_expression_weight_channel, build_empty_vrma, finalize_vrma_scenes, write_vrma_glb,
        ExpressionKind,
    };

    std::fs::create_dir_all(output_dir)?;

    // 1. .vrm avatar. For preset-expression variants we use the canonical
    //    default avatar (which already registers the 5 viseme presets via
    //    `viseme_preset_binds`). For custom-expression variants we use
    //    `emit_vrm_with_custom_expressions` to pre-register the named
    //    custom expression so `VRMExpressionController.setCustomExpressionWeight`
    //    doesn't silently no-op (the controllers require the name to be in
    //    `VRMC_vrm.expressions.custom` before they accept a weight write).
    let vrm_relpath = format!("{}.vrm", params.id);
    let vrm_path = output_dir.join(&vrm_relpath);
    let mtoon_defaults = crate::params::MToonParams::defaults(&params.id);
    if params.is_preset {
        emit_vrm(&mtoon_defaults, &vrm_path)?;
    } else {
        emit_vrm_with_custom_expressions(
            &mtoon_defaults,
            &vrm_path,
            &[params.expression_name.as_str()],
        )?;
    }

    // 2. .vrma: one node for the expression target + a 0→1→0 ramp.
    let mut doc = build_empty_vrma();
    let nodes = doc["nodes"].as_array_mut().unwrap();
    nodes.push(serde_json::json!({
        "name": format!("{}_expr_target", params.expression_name)
    }));
    let node_idx = nodes.len() - 1;

    let kind = if params.is_preset {
        ExpressionKind::Preset(&params.expression_name)
    } else {
        ExpressionKind::Custom(&params.expression_name)
    };
    let keyframes = [
        (0.0_f32, 0.0_f32),
        (params.duration_s / 2.0, 1.0),
        (params.duration_s, 0.0),
    ];

    let mut buffer = Vec::<u8>::new();
    add_expression_weight_channel(&mut doc, &mut buffer, node_idx, kind, &keyframes);

    finalize_vrma_scenes(&mut doc);

    let vrma_relpath = format!("{}.vrma", params.id);
    let vrma_path = output_dir.join(&vrma_relpath);
    let vrma_bytes = write_vrma_glb(&doc, &buffer)?;
    std::fs::write(&vrma_path, &vrma_bytes)?;

    // 3. .test.yaml.
    let plan = crate::sidecar::build_vrma_expression_test_plan(params, &vrm_relpath, &vrma_relpath);
    let yaml_path = output_dir.join(format!("{}.test.yaml", params.id));
    crate::sidecar::write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emit a VRMA lookAt sweep triplet: .vrm + .vrma + .test.yaml.
///
/// The .vrm uses the avatar lookAt.type specified by `params.avatar_lookat_type`
/// (bone or expression). The .vrma carries a single lookAt gaze rotation from
/// identity to the target direction over `params.duration_s`. The .test.yaml
/// declares `animation.vrma` so the runner drives the VRMA op sequence.
pub fn emit_vrma_lookat_triplet(
    output_dir: &Utf8Path,
    params: &crate::vrma_params::VrmaLookAtParams,
) -> Result<()> {
    use crate::vrma_emit::{
        add_look_at_channel, build_empty_vrma, finalize_vrma_scenes, write_vrma_glb,
    };
    use crate::vrma_params::{AvatarLookAtType, RotationAxis};

    std::fs::create_dir_all(output_dir)?;

    // 1. .vrm with the avatar's lookAt.type matching params.
    let vrm_relpath = format!("{}.vrm", params.id);
    let vrm_path = output_dir.join(&vrm_relpath);
    let mtoon_defaults = crate::params::MToonParams::defaults(&params.id);
    let avatar_lookat = match params.avatar_lookat_type {
        AvatarLookAtType::Bone => LookAtType::Bone,
        AvatarLookAtType::Expression => LookAtType::Expression,
    };
    emit_vrm_with_lookat_type(&mtoon_defaults, avatar_lookat, &vrm_path)?;

    // 2. .vrma with a single lookAt gaze direction.
    let mut doc = build_empty_vrma();
    doc["nodes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "name": "vrma_lookat_target"
        }));
    let node_idx: usize = 0; // first (and only) node

    let half_rad = (params.angle_deg.to_radians()) / 2.0;
    let sin_h = half_rad.sin();
    let cos_h = half_rad.cos();
    let target_quat = match params.axis {
        RotationAxis::X => [sin_h, 0.0, 0.0, cos_h],
        RotationAxis::Y => [0.0, sin_h, 0.0, cos_h],
        RotationAxis::Z => [0.0, 0.0, sin_h, cos_h],
    };
    let keyframes: [(f32, [f32; 4]); 2] = [
        (0.0_f32, [0.0_f32, 0.0, 0.0, 1.0]),
        (params.duration_s, target_quat),
    ];

    let mut buffer = Vec::<u8>::new();
    add_look_at_channel(
        &mut doc,
        &mut buffer,
        node_idx,
        [0.0, 0.06, 0.0],
        &keyframes,
    );

    finalize_vrma_scenes(&mut doc);

    let vrma_relpath = format!("{}.vrma", params.id);
    let vrma_path = output_dir.join(&vrma_relpath);
    let vrma_bytes = write_vrma_glb(&doc, &buffer)?;
    std::fs::write(&vrma_path, &vrma_bytes)?;

    // 3. .test.yaml.
    let plan = crate::sidecar::build_vrma_lookat_test_plan(params, &vrm_relpath, &vrma_relpath);
    crate::sidecar::write_test_yaml(&plan, &output_dir.join(format!("{}.test.yaml", params.id)))?;

    Ok(())
}

#[cfg(test)]
mod multichain_emit_integration_tests {
    use super::*;
    use crate::params::MToonParams;
    use crate::spring_bone::*;
    use camino::Utf8Path;
    use tempfile::tempdir;

    #[test]
    fn emit_three_chain_scene_produces_three_springs_in_glb_json() {
        let mtoon = MToonParams::defaults("multichain_test");
        let scene = SpringBoneSceneParams {
            springs: vec![
                SpringBoneParams::defaults("chain_a"),
                SpringBoneParams::defaults("chain_b"),
                SpringBoneParams::defaults("chain_c"),
            ],
            colliders: vec![],
            collider_groups: vec![],
            spring_collider_groups: vec![vec![], vec![], vec![]],
        };
        let tmp = tempdir().unwrap();
        let vrm_path = Utf8Path::from_path(tmp.path()).unwrap().join("out.vrm");
        emit_vrm_with_spring_bone_multichain(&mtoon, &scene, &vrm_path).unwrap();
        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        let springs = &doc["extensions"]["VRMC_springBone"]["springs"];
        assert_eq!(springs.as_array().unwrap().len(), 3);
    }

    #[test]
    fn emit_two_chain_scene_produces_two_skins() {
        let mtoon = MToonParams::defaults("mc2_test");
        let scene = SpringBoneSceneParams {
            springs: vec![
                SpringBoneParams::defaults("ca"),
                SpringBoneParams::defaults("cb"),
            ],
            colliders: vec![],
            collider_groups: vec![],
            spring_collider_groups: vec![vec![], vec![]],
        };
        let tmp = tempdir().unwrap();
        let vrm_path = Utf8Path::from_path(tmp.path()).unwrap().join("out.vrm");
        emit_vrm_with_spring_bone_multichain(&mtoon, &scene, &vrm_path).unwrap();
        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        let skins = doc["skins"].as_array().unwrap();
        assert_eq!(skins.len(), 2, "one skin per chain");
        // Sphere mesh + N chain meshes = N+1 meshes.
        let meshes = doc["meshes"].as_array().unwrap();
        assert_eq!(meshes.len(), 3, "sphere + 2 chain meshes");
    }
}

#[cfg(test)]
mod collider_emit_tests {
    use super::*;
    use crate::spring_bone::*;
    use camino::Utf8Path;
    use tempfile::tempdir;

    #[test]
    fn emit_with_sphere_collider_produces_loadable_glb_with_collider_json() {
        let mtoon = crate::params::MToonParams::defaults("test_collider");
        let mut spring = SpringBoneParams::defaults("test_chain");
        spring.joint_count = 4;
        let scene = SpringBoneSceneParams {
            springs: vec![spring],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Sphere { radius: 0.05 },
                offset: [0.0, -0.04, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "head_g".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };

        let tmp = tempdir().unwrap();
        let vrm_path = Utf8Path::from_path(tmp.path()).unwrap().join("out.vrm");
        emit_vrm_with_spring_bone_colliders(&mtoon, &scene, &vrm_path).unwrap();
        assert!(vrm_path.exists());

        // Inspect the GLB's JSON chunk to verify the collider made it through.
        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).expect("read JSON chunk");
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        let vrmc = &doc["extensions"]["VRMC_springBone"];
        assert!(vrmc["colliders"].is_array());
        assert_eq!(vrmc["colliders"].as_array().unwrap().len(), 1);
        let c0 = &vrmc["colliders"][0];
        assert!(c0["shape"]["sphere"].is_object());
        assert!(vrmc["colliderGroups"].is_array());
        assert_eq!(
            vrmc["springs"][0]["colliderGroups"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}

#[cfg(test)]
mod extended_emit_integration_tests {
    use super::*;
    use crate::params::MToonParams;
    use crate::spring_bone::*;
    use camino::Utf8Path;
    use tempfile::tempdir;

    #[test]
    fn emitted_glb_with_plane_collider_declares_extended_collider_in_extensions_used() {
        let mtoon = MToonParams::defaults("test_plane");
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("test")],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Plane {
                    normal: [0.0, 1.0, 0.0],
                },
                offset: [0.0, -0.10, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "g".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };
        let tmp = tempdir().unwrap();
        let vrm_path = Utf8Path::from_path(tmp.path()).unwrap().join("out.vrm");
        emit_vrm_with_spring_bone_colliders(&mtoon, &scene, &vrm_path).unwrap();
        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        let used = doc["extensionsUsed"].as_array().unwrap();
        let names: Vec<&str> = used.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            names.contains(&"VRMC_springBone"),
            "extensionsUsed must declare VRMC_springBone: {names:?}"
        );
        assert!(
            names.contains(&"VRMC_springBone_extended_collider"),
            "extensionsUsed must declare VRMC_springBone_extended_collider when plane shape used: {names:?}"
        );
    }

    #[test]
    fn emit_vrm_v0_produces_parseable_glb_with_full_extension() {
        use tempfile::tempdir;
        let tmp = tempdir().unwrap();
        let path = Utf8Path::from_path(tmp.path()).unwrap().join("v0.vrm");
        emit_vrm_v0("v0_test", &[MToonParams::defaults("v0_test")], &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        let used = doc["extensionsUsed"].as_array().unwrap();
        let names: Vec<&str> = used.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            names.contains(&"VRM"),
            "extensionsUsed must declare VRM: {names:?}"
        );
        assert!(
            doc["extensions"]["VRM"].is_object(),
            "extensions.VRM must be present"
        );
        assert_eq!(doc["extensions"]["VRM"]["specVersion"], "0.0");
        assert!(
            doc["extensions"]["VRM"]["meta"].is_object(),
            "VRM.meta must be present"
        );
        assert!(
            doc["extensions"]["VRM"]["humanoid"].is_object(),
            "VRM.humanoid must be present"
        );
        let mat_props = doc["extensions"]["VRM"]["materialProperties"]
            .as_array()
            .unwrap();
        assert_eq!(mat_props.len(), 1, "one default MToon material");
    }

    #[test]
    fn emitted_glb_with_base_sphere_collider_does_not_declare_extended_collider() {
        let mtoon = MToonParams::defaults("test_sphere_no_ext");
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("test")],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Sphere { radius: 0.05 },
                offset: [0.0, -0.04, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "g".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };
        let tmp = tempdir().unwrap();
        let vrm_path = Utf8Path::from_path(tmp.path()).unwrap().join("out.vrm");
        emit_vrm_with_spring_bone_colliders(&mtoon, &scene, &vrm_path).unwrap();
        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        let used = doc["extensionsUsed"].as_array().unwrap();
        let names: Vec<&str> = used.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            !names.contains(&"VRMC_springBone_extended_collider"),
            "base sphere collider must NOT declare VRMC_springBone_extended_collider: {names:?}"
        );
    }
}

#[cfg(test)]
mod spring_bone_v0_tests {
    use super::*;

    #[test]
    fn emit_spring_bone_v0_writes_secondary_animation() {
        let tmp = tempfile::tempdir().unwrap();
        let stem = camino::Utf8PathBuf::from_path_buf(tmp.path().join("sb_v0")).unwrap();
        let mtoon = crate::params::MToonParams::defaults("sb_v0");
        let spring = crate::spring_bone::SpringBoneParams::defaults("sb_v0");
        emit_with_sidecars_spring_bone_v0(&mtoon, &spring, &stem).unwrap();
        let bytes = std::fs::read(stem.with_extension("vrm")).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("secondaryAnimation"));
        assert!(text.contains("boneGroups"));
        assert!(text.contains("stiffiness"));
        // test.yaml is tagged 0.x
        let yaml = std::fs::read_to_string(stem.with_extension("test.yaml")).unwrap();
        assert!(yaml.contains("0.x") || yaml.contains("\"0.x\""));
    }

    #[test]
    fn emit_spring_bone_v0_swing_has_animation_block() {
        let tmp = tempfile::tempdir().unwrap();
        let stem = camino::Utf8PathBuf::from_path_buf(tmp.path().join("swing_sb_v0")).unwrap();
        let mtoon = crate::params::MToonParams::defaults("swing_sb_v0");
        let spring = crate::spring_bone::SpringBoneParams::defaults("swing_sb_v0");
        emit_with_sidecars_spring_bone_v0_swing(&mtoon, &spring, &stem).unwrap();
        let yaml = std::fs::read_to_string(stem.with_extension("test.yaml")).unwrap();
        assert!(
            yaml.contains("animation"),
            "swing plan must carry animate_root_transform"
        );
        assert!(yaml.contains("0.x"), "plan must be tagged spec_version 0.x");
        // .vrm still carries the secondaryAnimation (same asset as settle)
        let vrm_bytes = std::fs::read(stem.with_extension("vrm")).unwrap();
        let text = String::from_utf8_lossy(&vrm_bytes);
        assert!(text.contains("secondaryAnimation"));
    }

    #[test]
    #[ignore = "requires .tools/vrm-validator-cli"]
    fn emit_spring_bone_v0_passes_validator() {
        use vrm_validator_wrap::{validate, ValidatorConfig};
        let cfg = match ValidatorConfig::from_env() {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "SKIP: validator shim not reachable ({e}). Set VRM_VALIDATOR_BIN to an absolute path \
                     or run scripts/install-validator.sh from the workspace root."
                );
                return;
            }
        };
        let tmp = tempfile::tempdir().unwrap();
        let stem = camino::Utf8PathBuf::from_path_buf(tmp.path().join("sb_v0_val")).unwrap();
        let mtoon = crate::params::MToonParams::defaults("sb_v0_val");
        let spring = crate::spring_bone::SpringBoneParams::defaults("sb_v0_val");
        emit_with_sidecars_spring_bone_v0(&mtoon, &spring, &stem).unwrap();
        let vrm = stem.with_extension("vrm");
        let report = validate(&cfg, &vrm).expect("validator must run");
        if report.issues.num_errors > 0 {
            let summary = report
                .issues
                .messages
                .iter()
                .filter(|m| m.severity == 0)
                .map(|m| format!("{}: {}", m.code, m.message))
                .collect::<Vec<_>>()
                .join("; ");
            panic!(
                "VRM 0.x spring-bone asset has {} validator errors: {summary}",
                report.issues.num_errors
            );
        }
        eprintln!(
            "emit_spring_bone_v0_passes_validator: 0 errors, {} warnings",
            report.issues.num_warnings
        );
    }

    /// Emit test: sphere-collider v0 asset contains the expected JSON keys.
    #[test]
    fn emit_spring_bone_colliders_v0_writes_collider_groups_and_secondary_animation() {
        use crate::spring_bone::{
            ColliderAttach, ColliderGroupParams, ColliderParams, ColliderShape, SpringBoneParams,
            SpringBoneSceneParams,
        };
        let tmp = tempfile::tempdir().unwrap();
        let stem = camino::Utf8PathBuf::from_path_buf(tmp.path().join("sb_coll_v0")).unwrap();
        let mtoon = crate::params::MToonParams::defaults("sb_coll_v0");
        let mut spring = SpringBoneParams::defaults("sb_coll_v0");
        spring.joint_count = 4;
        let scene = SpringBoneSceneParams {
            springs: vec![spring],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Sphere { radius: 0.06 },
                offset: [0.0, -0.04, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "head_g".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };
        emit_with_sidecars_spring_bone_colliders_v0(&mtoon, &scene, &stem)
            .expect("emission must succeed");

        let vrm_bytes = std::fs::read(stem.with_extension("vrm")).unwrap();
        let text = String::from_utf8_lossy(&vrm_bytes);
        assert!(
            text.contains("colliderGroups"),
            "VRM 0.x asset must contain colliderGroups"
        );
        assert!(
            text.contains("radius"),
            "VRM 0.x collider must contain radius"
        );
        assert!(
            text.contains("secondaryAnimation"),
            "VRM 0.x asset must contain secondaryAnimation"
        );

        // Verify the test.yaml carries spec_version 0.x.
        let yaml = std::fs::read_to_string(stem.with_extension("test.yaml")).unwrap();
        assert!(
            yaml.contains("0.x") || yaml.contains("\"0.x\""),
            "test.yaml must be tagged spec_version 0.x"
        );
    }

    /// Validator-gated integration test: sphere-collider v0 asset passes the
    /// VRM validator with zero errors.
    #[test]
    #[ignore = "requires .tools/vrm-validator-cli"]
    fn emit_spring_bone_colliders_v0_passes_validator() {
        use crate::spring_bone::{
            ColliderAttach, ColliderGroupParams, ColliderParams, ColliderShape, SpringBoneParams,
            SpringBoneSceneParams,
        };
        use vrm_validator_wrap::{validate, ValidatorConfig};

        let cfg = match ValidatorConfig::from_env() {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "SKIP: validator shim not reachable ({e}). Set VRM_VALIDATOR_BIN to an absolute path \
                     (e.g. VRM_VALIDATOR_BIN=$(git rev-parse --show-toplevel)/.tools/vrm-validator-cli) \
                     or run scripts/install-validator.sh from the workspace root."
                );
                return;
            }
        };

        let tmp = tempfile::tempdir().unwrap();
        let stem = camino::Utf8PathBuf::from_path_buf(tmp.path().join("sb_coll_v0_val")).unwrap();
        let mtoon = crate::params::MToonParams::defaults("sb_coll_v0_val");
        let mut spring = SpringBoneParams::defaults("sb_coll_v0_val");
        spring.joint_count = 4;
        let scene = SpringBoneSceneParams {
            springs: vec![spring],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Sphere { radius: 0.06 },
                offset: [0.0, -0.04, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "head_g".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };
        emit_with_sidecars_spring_bone_colliders_v0(&mtoon, &scene, &stem)
            .expect("emission must succeed");

        let vrm = stem.with_extension("vrm");
        let report = validate(&cfg, &vrm).expect("validator must run");
        if report.issues.num_errors > 0 {
            let summary = report
                .issues
                .messages
                .iter()
                .filter(|m| m.severity == 0)
                .map(|m| format!("{}: {}", m.code, m.message))
                .collect::<Vec<_>>()
                .join("; ");
            panic!(
                "VRM 0.x sphere-collider asset has {} validator errors: {summary}",
                report.issues.num_errors
            );
        }
        eprintln!(
            "emit_spring_bone_colliders_v0_passes_validator: 0 errors, {} warnings",
            report.issues.num_warnings
        );
    }
}

#[cfg(test)]
mod doublesided_quad_tests {
    use super::*;
    use crate::params::MToonParams;
    use camino::Utf8Path;
    use tempfile::tempdir;

    #[test]
    fn doublesided_spec_test_pair_emits_two_triplets_false_has_cross_variant() {
        let tmp = tempdir().unwrap();
        let dir = Utf8Path::from_path(tmp.path()).unwrap();
        emit_doublesided_spec_test_pair(dir).unwrap();

        for id in ["doublesided_quad_false", "doublesided_quad_true"] {
            assert!(dir.join(format!("{id}.vrm")).exists(), "{id}.vrm missing");
            assert!(
                dir.join(format!("{id}.test.yaml")).exists(),
                "{id}.test.yaml missing"
            );
            assert!(
                dir.join(format!("{id}.meta.json")).exists(),
                "{id}.meta.json missing"
            );
        }

        // The false plan declares the cross-variant assertion; the true plan does not.
        let false_yaml =
            std::fs::read_to_string(dir.join("doublesided_quad_false.test.yaml")).unwrap();
        assert!(false_yaml.contains("cross_variant"));
        assert!(false_yaml.contains("doublesided_quad_true"));
        let true_yaml =
            std::fs::read_to_string(dir.join("doublesided_quad_true.test.yaml")).unwrap();
        assert!(!true_yaml.contains("cross_variant"));
    }

    #[test]
    fn doublesided_quad_emit_has_quad_geom_no_morphs_and_double_sided_flag() {
        let mut params = MToonParams::defaults("ds_quad_test");
        params.double_sided = true;
        let tmp = tempdir().unwrap();
        let vrm_path = Utf8Path::from_path(tmp.path()).unwrap().join("out.vrm");
        emit_vrm_doublesided_quad(&params, &vrm_path).unwrap();

        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();

        // No morph targets on the quad primitive (a confounder we deliberately drop).
        let prim = &doc["meshes"][0]["primitives"][0];
        assert!(
            prim.get("targets").is_none(),
            "quad primitive must carry no morph targets"
        );
        // Material carries the doubleSided flag verbatim.
        assert_eq!(doc["materials"][0]["doubleSided"], serde_json::json!(true));
        // Quad geometry: accessor 0 = POSITION (4 verts), accessor 3 = indices (6).
        assert_eq!(doc["accessors"][0]["count"], serde_json::json!(4));
        assert_eq!(doc["accessors"][3]["count"], serde_json::json!(6));
    }
}

#[cfg(test)]
mod multichain_v0_emit_tests {
    use super::*;
    use crate::params::MToonParams;
    use crate::spring_bone::*;
    use crate::sweep::spring_bone_multichain_sweep;
    use camino::Utf8Path;
    use tempfile::tempdir;

    #[test]
    fn multichain_v0_emit_has_secondary_animation_and_n_bone_groups() {
        // Take the first variant from the multichain sweep.
        let variants = spring_bone_multichain_sweep();
        let (mtoon, scene) = &variants[0];

        let tmp = tempdir().unwrap();
        let stem = Utf8Path::from_path(tmp.path()).unwrap().join("mc_v0_test");
        emit_with_sidecars_spring_bone_multichain_v0(mtoon, scene, &stem).unwrap();

        let vrm_path = stem.with_extension("vrm");
        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();

        // Must carry secondaryAnimation (not VRMC_springBone).
        let sa = &doc["extensions"]["VRM"]["secondaryAnimation"];
        assert!(
            !sa.is_null(),
            "0.x multichain asset must have extensions.VRM.secondaryAnimation"
        );

        // boneGroups count must equal scene.springs.len().
        let bone_groups = sa["boneGroups"].as_array().expect("boneGroups array");
        assert_eq!(
            bone_groups.len(),
            scene.springs.len(),
            "boneGroups.len() must equal scene.springs.len()"
        );
    }

    #[test]
    fn multichain_v0_emit_carries_vrm_extension_not_vrmc() {
        let mtoon = MToonParams::defaults("mc_v0_ext_check");
        let scene = SpringBoneSceneParams {
            springs: vec![
                SpringBoneParams::defaults("chain_a"),
                SpringBoneParams::defaults("chain_b"),
            ],
            colliders: vec![],
            collider_groups: vec![],
            spring_collider_groups: vec![vec![], vec![]],
        };
        let tmp = tempdir().unwrap();
        let vrm_path = Utf8Path::from_path(tmp.path()).unwrap().join("out.vrm");
        emit_vrm_with_spring_bone_multichain_v0(&mtoon, &scene, &vrm_path).unwrap();

        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();

        // extensionsUsed must contain "VRM" and "KHR_materials_unlit".
        let ext_used = doc["extensionsUsed"]
            .as_array()
            .expect("extensionsUsed array");
        let ext_names: Vec<&str> = ext_used.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            ext_names.contains(&"VRM"),
            "extensionsUsed must contain VRM"
        );
        assert!(
            ext_names.contains(&"KHR_materials_unlit"),
            "extensionsUsed must contain KHR_materials_unlit"
        );
        // Must NOT contain VRMC_springBone.
        assert!(
            !ext_names.contains(&"VRMC_springBone"),
            "0.x asset must not declare VRMC_springBone"
        );
        // Must NOT contain extensionsRequired (0.x has none).
        assert!(
            doc.get("extensionsRequired").is_none(),
            "0.x asset must not have extensionsRequired"
        );

        // N skins — one per chain.
        let skins = doc["skins"].as_array().expect("skins array");
        assert_eq!(skins.len(), 2, "one skin per chain");

        // Sphere + N chain meshes.
        let meshes = doc["meshes"].as_array().expect("meshes array");
        assert_eq!(meshes.len(), 3, "sphere + 2 chain meshes");
    }
}

#[cfg(test)]
mod explicit_tail_tests {
    use super::*;
    use crate::glb::extract_json_chunk;
    use tempfile::tempdir;

    fn parse_glb_json(path: &camino::Utf8Path) -> serde_json::Value {
        let bytes = std::fs::read(path).unwrap();
        let json_bytes = extract_json_chunk(&bytes).expect("GLB must have a JSON chunk");
        serde_json::from_slice(&json_bytes).unwrap()
    }

    #[test]
    fn explicit_tail_v1_adds_end_joint_7cm_along_axis() {
        let dir = tempdir().unwrap();
        let out = camino::Utf8PathBuf::from_path_buf(dir.path().join("et.vrm")).unwrap();
        let mtoon = crate::params::MToonParams::defaults("et");
        let mut spring = crate::spring_bone::SpringBoneParams::defaults("et");
        spring.joint_count = 2;
        spring.chain_axis = [0.0, 0.0, 1.0];
        spring.explicit_tail = true;
        emit_vrm_with_spring_bone(&mtoon, &spring, &out).unwrap();

        let json = parse_glb_json(&out);
        let joints = json["extensions"]["VRMC_springBone"]["springs"][0]["joints"]
            .as_array()
            .unwrap();
        assert_eq!(joints.len(), 3, "2 chain joints + 1 explicit _end");
        let end_node_idx = joints[2]["node"].as_u64().unwrap() as usize;
        let t = json["nodes"][end_node_idx]["translation"]
            .as_array()
            .unwrap();
        assert!(
            (t[2].as_f64().unwrap() - 0.07).abs() < 1e-6,
            "7cm along +Z, got {}",
            t[2].as_f64().unwrap()
        );
        assert!(t[0].as_f64().unwrap().abs() < 1e-6);
        assert!(t[1].as_f64().unwrap().abs() < 1e-6);
        // The _end node must NOT appear in the skin's joints (not mesh-weighted).
        let skin_joints = json["skins"][0]["joints"].as_array().unwrap();
        assert!(
            !skin_joints
                .iter()
                .any(|j| j.as_u64().unwrap() as usize == end_node_idx),
            "spring_joint_end must not be in skin.joints"
        );
    }

    #[test]
    fn no_explicit_tail_v1_keeps_joint_count() {
        let dir = tempdir().unwrap();
        let out = camino::Utf8PathBuf::from_path_buf(dir.path().join("net.vrm")).unwrap();
        let mtoon = crate::params::MToonParams::defaults("net");
        let mut spring = crate::spring_bone::SpringBoneParams::defaults("net");
        spring.joint_count = 2;
        // explicit_tail defaults to false
        emit_vrm_with_spring_bone(&mtoon, &spring, &out).unwrap();

        let json = parse_glb_json(&out);
        let joints = json["extensions"]["VRMC_springBone"]["springs"][0]["joints"]
            .as_array()
            .unwrap();
        assert_eq!(joints.len(), 2, "no _end without explicit_tail");
    }

    #[test]
    fn explicit_tail_ignored_in_v0() {
        let dir = tempdir().unwrap();
        let out = camino::Utf8PathBuf::from_path_buf(dir.path().join("v0.vrm")).unwrap();
        let mtoon = crate::params::MToonParams::defaults("v0et");
        let mut spring = crate::spring_bone::SpringBoneParams::defaults("v0et");
        spring.joint_count = 2;
        spring.explicit_tail = true; // must be ignored by 0.x
        emit_vrm_with_spring_bone_v0(&mtoon, &spring, &out).unwrap();

        let json = parse_glb_json(&out);
        let bones = json["extensions"]["VRM"]["secondaryAnimation"]["boneGroups"][0]["bones"]
            .as_array()
            .unwrap();
        assert_eq!(
            bones.len(),
            1,
            "0.x lists only the root regardless of explicit_tail"
        );
    }
}

#[cfg(test)]
mod byte_identity_guard {
    use super::*;
    use tempfile::tempdir;

    // BLAKE3 of the DEFAULT spring-bone GLBs. Captured after Part A proved the
    // chain_axis/explicit_tail feature preserved byte-identity vs pre-feature
    // commit 4639d35. If either fails, the default (-Y) geometry path drifted —
    // investigate the geometry change; do NOT update these hashes casually.
    const DEFAULT_V1_BLAKE3: &str =
        "007ae2a770766a107fd94e900da396b71b8ab92af086b56b4b63fba9ba86572a";
    const DEFAULT_V0_BLAKE3: &str =
        "769e9084089987369bc3899a727ee2dbced7ad7ebaa89b3f36e9a776605a80f2";

    fn emit_default_and_hash(v0: bool) -> String {
        let dir = tempdir().unwrap();
        let out = camino::Utf8PathBuf::from_path_buf(dir.path().join("d.vrm")).unwrap();
        let mtoon = crate::params::MToonParams::defaults("springbone_default");
        let spring = crate::spring_bone::SpringBoneParams::defaults("springbone_default");
        if v0 {
            emit_vrm_with_spring_bone_v0(&mtoon, &spring, &out).unwrap();
        } else {
            emit_vrm_with_spring_bone(&mtoon, &spring, &out).unwrap();
        }
        let bytes = std::fs::read(&out).unwrap();
        blake3::hash(&bytes).to_hex().to_string()
    }

    #[test]
    fn default_v1_spring_bone_asset_is_byte_identical() {
        assert_eq!(
            emit_default_and_hash(false),
            DEFAULT_V1_BLAKE3,
            "V1 default -Y output drifted"
        );
    }

    #[test]
    fn default_v0_spring_bone_asset_is_byte_identical() {
        assert_eq!(
            emit_default_and_hash(true),
            DEFAULT_V0_BLAKE3,
            "V0 default -Y output drifted"
        );
    }

    // BLAKE3 of the DEFAULT collider + multichain GLBs (first variant of each
    // sweep: collider = sphere, off_x=-0.05, r=0.03, SpringBoneParams::defaults;
    // multichain = n2_sp0p02_share_all, SpringBoneParams::defaults per chain).
    // Captured after Task 4b conversion proved byte-identity vs 4639d35.
    // Do NOT update these hashes casually.
    const DEFAULT_COLLIDER_V1_BLAKE3: &str =
        "f85a7a55b80f170c7ad01d9832fa2f970ff4d9a5288b128320fcc4607b1ebdd1";
    const DEFAULT_MULTICHAIN_V1_BLAKE3: &str =
        "a413cffa2aa6d3b0ac24f4d7e26cb7e593b84bf4b0b793a1d53924bfda70f190";

    fn emit_default_collider_v1_hash() -> String {
        use crate::spring_bone::{
            ColliderAttach, ColliderGroupParams, ColliderParams, ColliderShape,
            SpringBoneSceneParams,
        };
        let dir = tempdir().unwrap();
        let out = camino::Utf8PathBuf::from_path_buf(dir.path().join("d.vrm")).unwrap();
        // First variant of spring_bone_collider_sweep(): sphere, off_x=-0.05, r=0.03.
        let id = "springbone_collider_sphere_xneg0p05_r0p03";
        let mtoon = crate::params::MToonParams::defaults(id);
        let spring = crate::spring_bone::SpringBoneParams::defaults(id);
        let scene = SpringBoneSceneParams {
            springs: vec![spring],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Sphere { radius: 0.03 },
                offset: [-0.05, -0.10, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "head_g".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };
        emit_vrm_with_spring_bone_colliders(&mtoon, &scene, &out).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        blake3::hash(&bytes).to_hex().to_string()
    }

    fn emit_default_multichain_v1_hash() -> String {
        use crate::spring_bone::{
            ColliderAttach, ColliderGroupParams, ColliderParams, ColliderShape,
            SpringBoneSceneParams,
        };
        let dir = tempdir().unwrap();
        let out = camino::Utf8PathBuf::from_path_buf(dir.path().join("d.vrm")).unwrap();
        // First variant of spring_bone_multichain_sweep(): n=2, sp=0.02, share_all.
        let id = "springbone_multichain_n2_sp0p02_share_all";
        let mtoon = crate::params::MToonParams::defaults(id);
        let springs: Vec<_> = (0..2)
            .map(|i| crate::spring_bone::SpringBoneParams::defaults(format!("{id}_chain_{i}")))
            .collect();
        let scene = SpringBoneSceneParams {
            springs,
            colliders: vec![ColliderParams {
                shape: ColliderShape::Sphere { radius: 0.04 },
                offset: [0.03, -0.10, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "shared".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0], vec![0]],
        };
        emit_vrm_with_spring_bone_multichain(&mtoon, &scene, &out).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        blake3::hash(&bytes).to_hex().to_string()
    }

    #[test]
    fn default_collider_v1_asset_is_byte_identical() {
        assert_eq!(
            emit_default_collider_v1_hash(),
            DEFAULT_COLLIDER_V1_BLAKE3,
            "collider V1 default -Y output drifted"
        );
    }

    #[test]
    fn default_multichain_v1_asset_is_byte_identical() {
        assert_eq!(
            emit_default_multichain_v1_hash(),
            DEFAULT_MULTICHAIN_V1_BLAKE3,
            "multichain V1 default -Y output drifted"
        );
    }
}
