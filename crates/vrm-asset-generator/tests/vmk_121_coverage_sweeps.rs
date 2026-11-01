//! End-to-end: the VMK 1.2.1 re-gate coverage sweeps (VMK#456, VMK#457).
//! `emit-texture-transform-rigged-sweep`, `emit-slot-texture-transform-sweep`
//! and `emit-material-name-blend-sweep` emit well-formed triplets whose glTF
//! carries the structure each cell exists to test. See docs/findings.md
//! 2026-11-01.

use serde_json::Value;
use std::path::Path;
use std::process::Command;

fn emit(subcommand: &str, out: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args([subcommand, "--output-dir", out.to_str().unwrap()])
        .output()
        .expect("run vrm-asset-generator");
    assert!(
        output.status.success(),
        "{subcommand} should exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn count_ext(dir: &Path, ext: &str) -> usize {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(ext))
        .count()
}

/// Returns the GLB's JSON chunk and the BIN chunk length.
fn read_glb(path: &Path) -> (Value, usize) {
    let bytes = std::fs::read(path).expect("read .vrm");
    assert_eq!(&bytes[0..4], b"glTF");
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let json: Value = serde_json::from_slice(&bytes[20..20 + json_len]).expect("JSON chunk");
    let bin_header = 20 + json_len;
    let bin_len =
        u32::from_le_bytes(bytes[bin_header..bin_header + 4].try_into().unwrap()) as usize;
    (json, bin_len)
}

fn uses_texture_transform(doc: &Value) -> bool {
    doc["extensionsUsed"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e == "KHR_texture_transform")
}

#[test]
fn rigged_sweep_emits_a_head_skinned_sphere() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("rigged");
    emit("emit-texture-transform-rigged-sweep", &out);
    assert_eq!(count_ext(&out, ".vrm"), 8);
    assert_eq!(count_ext(&out, ".test.yaml"), 8);

    let (doc, bin_len) = read_glb(&out.join("mtoon_uvxform_rigged_offset_x_0p5.vrm"));
    let mesh_node = doc["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .position(|n| n.get("mesh").is_some())
        .expect("a mesh node");
    let node = &doc["nodes"][mesh_node];
    assert_eq!(node["skin"], 0, "mesh node must reference the skin");

    // glTF: a skinned mesh node's own transform is ignored, so it sits at
    // the scene root rather than under the head bone.
    assert!(doc["scenes"][0]["nodes"]
        .as_array()
        .unwrap()
        .contains(&Value::from(mesh_node)));
    let parented = doc["nodes"].as_array().unwrap().iter().any(|n| {
        n["children"]
            .as_array()
            .is_some_and(|c| c.contains(&Value::from(mesh_node)))
    });
    assert!(!parented, "skinned mesh node must not be a child of a bone");

    let skin = &doc["skins"][0];
    let head = skin["joints"][0].as_u64().unwrap() as usize;
    assert_eq!(skin["joints"].as_array().unwrap().len(), 1);
    assert!(doc["nodes"][head]["name"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("head"));

    let attrs = &doc["meshes"][0]["primitives"][0]["attributes"];
    let count = |acc: &Value| doc["accessors"][acc.as_u64().unwrap() as usize]["count"].clone();
    assert_eq!(count(&attrs["JOINTS_0"]), count(&attrs["POSITION"]));
    assert_eq!(count(&attrs["WEIGHTS_0"]), count(&attrs["POSITION"]));
    assert_eq!(count(&skin["inverseBindMatrices"]), 1);

    // Bind-pose positions are in world space: the sphere (radius 0.3) sits
    // at head height, not at the origin.
    let pos_min_y = doc["accessors"][attrs["POSITION"].as_u64().unwrap() as usize]["min"][1]
        .as_f64()
        .unwrap();
    assert!(
        pos_min_y > 0.5,
        "bind-pose sphere must sit at head height, min y = {pos_min_y}"
    );

    let declared = doc["buffers"][0]["byteLength"].as_u64().unwrap() as usize;
    assert!(
        declared <= bin_len && bin_len - declared < 4,
        "buffer length {declared} vs BIN {bin_len}"
    );
    assert!(uses_texture_transform(&doc));
}

#[test]
fn slot_sweep_places_the_transform_on_the_named_textureinfo_only() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("slot");
    emit("emit-slot-texture-transform-sweep", &out);
    assert_eq!(count_ext(&out, ".vrm"), 25);

    let mtoon = |doc: &Value| doc["materials"][0]["extensions"]["VRMC_materials_mtoon"].clone();
    let tt = |info: &Value| info["extensions"]["KHR_texture_transform"]["offset"].clone();
    let shift = serde_json::json!([0.5, 0.0]);

    let (none, _) = read_glb(&out.join("mtoon_slotxform_shade_none.vrm"));
    assert!(!uses_texture_transform(&none));
    assert!(mtoon(&none)["shadeMultiplyTexture"]["extensions"].is_null());

    let (slotonly, _) = read_glb(&out.join("mtoon_slotxform_shade_slotonly.vrm"));
    assert!(uses_texture_transform(&slotonly));
    assert_eq!(tt(&mtoon(&slotonly)["shadeMultiplyTexture"]), shift);
    assert!(slotonly["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"].is_null());

    let (baseonly, _) = read_glb(&out.join("mtoon_slotxform_shade_baseonly.vrm"));
    assert_eq!(
        tt(&baseonly["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]),
        shift
    );
    assert!(mtoon(&baseonly)["shadeMultiplyTexture"]["extensions"].is_null());

    let (baseident, _) = read_glb(&out.join("mtoon_slotxform_shade_baseident.vrm"));
    assert!(!uses_texture_transform(&baseident));
    assert!(baseident["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"].is_object());

    let (normal, _) = read_glb(&out.join("mtoon_slotxform_normal_slotonly.vrm"));
    assert_eq!(tt(&normal["materials"][0]["normalTexture"]), shift);
    assert_eq!(normal["materials"][0]["normalTexture"]["index"], 1);

    let (owidth, _) = read_glb(&out.join("mtoon_slotxform_owidth_both.vrm"));
    assert_eq!(tt(&mtoon(&owidth)["outlineWidthMultiplyTexture"]), shift);
    assert_eq!(
        tt(&owidth["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]),
        shift
    );
}

#[test]
fn name_blend_sweep_names_the_material_and_keeps_authored_flags() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("names");
    emit("emit-material-name-blend-sweep", &out);
    assert_eq!(count_ext(&out, ".vrm"), 12);

    let (lace, _) = read_glb(&out.join("matname_blend_lace_zw0_ss.vrm"));
    let m = &lace["materials"][0];
    assert_eq!(m["name"], "matname_blend_lace_zw0_ss");
    assert_eq!(m["alphaMode"], "BLEND");
    assert_eq!(m["doubleSided"], false);
    assert_eq!(
        m["extensions"]["VRMC_materials_mtoon"]["transparentWithZWrite"],
        false
    );

    let (plain, _) = read_glb(&out.join("matname_blend_plain_zw1_ds.vrm"));
    let m = &plain["materials"][0];
    assert_eq!(m["doubleSided"], true);
    assert_eq!(
        m["extensions"]["VRMC_materials_mtoon"]["transparentWithZWrite"],
        true
    );
}

#[test]
fn existing_sweep_sidecars_do_not_gain_the_slot_field() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("uvxform");
    let output = Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args([
            "emit-texture-transform-sweep",
            "--output-dir",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let meta = std::fs::read_to_string(out.join("mtoon_uvxform_offset_x_0p5.meta.json")).unwrap();
    assert!(
        !meta.contains("slot_texture_transform"),
        "unset slot transform must not serialize, keeping existing sidecars byte-identical"
    );
}
