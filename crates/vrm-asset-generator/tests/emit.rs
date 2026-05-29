use camino::Utf8PathBuf;
use vrm_asset_generator::{
    emit::{emit_vrm, emit_vrm_with_spring_bone_v0},
    glb::extract_json_chunk,
    params::MToonParams,
    spring_bone::SpringBoneParams,
};
use vrm_validator_wrap::{validate, ValidatorConfig};

fn config_or_skip() -> Option<ValidatorConfig> {
    match ValidatorConfig::from_env() {
        Ok(c) => Some(c),
        Err(_) => {
            eprintln!("SKIP: validator not installed");
            None
        }
    }
}

#[test]
fn emits_validator_clean_vrm_with_default_mtoon() {
    let Some(cfg) = config_or_skip() else { return };

    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::from_path_buf(dir.path().join("default.vrm")).unwrap();

    let params = MToonParams::defaults("default");
    emit_vrm(&params, &out).expect("emission must succeed");

    let report = validate(&cfg, &out).expect("validator must produce a report");
    assert_eq!(
        report.issues.num_errors, 0,
        "emitted VRM should have zero validator errors. report: {:#?}",
        report.issues.messages
    );

    // mimeType should be GLB.
    assert_eq!(report.mime_type.as_deref(), Some("model/gltf-binary"));
}

#[test]
fn emits_validator_clean_vrm_with_outline() {
    let Some(cfg) = config_or_skip() else { return };

    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::from_path_buf(dir.path().join("outline.vrm")).unwrap();

    let mut params = MToonParams::defaults("outline_world_05cm");
    params.outline_width_mode = vrm_asset_generator::params::OutlineWidthMode::WorldCoordinates;
    params.outline_width_factor = 0.005;
    params.outline_color_factor = [0.0, 0.0, 0.0];

    emit_vrm(&params, &out).unwrap();
    let report = validate(&cfg, &out).unwrap();
    assert_eq!(report.issues.num_errors, 0);
}

/// Structural assertion: every synthetic VRM emitted by `emit_vrm` must
/// carry five POSITION-only morph targets on its primitive, and its
/// `VRMC_vrm.expressions.preset` map must bind each viseme preset
/// (aa, ih, ou, ee, oh) to the corresponding morph target index on the
/// mesh node. This is the load-bearing piece for the conformance gap
/// described in `docs/findings.md` (visemes accepted, mesh not deformed):
/// without these bindings, no SSIM signal can distinguish a deforming
/// renderer from a non-deforming one.
#[test]
fn emit_vrm_binds_five_visemes_to_mesh_morph_targets() {
    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::from_path_buf(dir.path().join("viseme_smoke.vrm")).unwrap();

    let params = MToonParams::defaults("viseme_smoke");
    emit_vrm(&params, &out).expect("emission must succeed");

    let bytes = std::fs::read(out.as_std_path()).unwrap();
    let json_bytes = extract_json_chunk(&bytes).expect("GLB must have a JSON chunk");
    let doc: serde_json::Value = serde_json::from_slice(&json_bytes).unwrap();

    // The single primitive should carry five POSITION-only morph targets,
    // matching the five visemes (aa, ih, ou, ee, oh) bound below.
    let primitive = &doc["meshes"][0]["primitives"][0];
    let targets = primitive["targets"]
        .as_array()
        .expect("primitive must have a `targets` array");
    assert_eq!(
        targets.len(),
        5,
        "expected 5 morph targets (aa/ih/ou/ee/oh)"
    );
    for (i, t) in targets.iter().enumerate() {
        assert!(
            t["POSITION"].is_u64(),
            "morph target {i} must reference a POSITION accessor"
        );
    }

    // The expressions.preset map must bind each viseme by name to one of
    // the morph targets on the mesh node. Index ordering: aa=0, ih=1, ou=2,
    // ee=3, oh=4 — matches the morph-targets array on the primitive.
    let presets = &doc["extensions"]["VRMC_vrm"]["expressions"]["preset"];
    let mesh_node = primitive_owner_node(&doc).expect("mesh node must be discoverable");

    for (expected_idx, name) in ["aa", "ih", "ou", "ee", "oh"].iter().enumerate() {
        let bind = &presets[name]["morphTargetBinds"][0];
        assert_eq!(
            bind["node"].as_u64().unwrap() as usize,
            mesh_node,
            "preset {name} should bind to the mesh-bearing node"
        );
        assert_eq!(
            bind["index"].as_u64().unwrap() as usize,
            expected_idx,
            "preset {name} should bind to morph target index {expected_idx}"
        );
        let weight = bind["weight"].as_f64().unwrap();
        assert!(
            (weight - 1.0).abs() < 1e-6,
            "preset {name} weight should be 1.0"
        );
    }
}

/// Find the glTF node that owns mesh 0 (the mesh-bearing node added by
/// `emit_vrm`). Returns its index.
fn primitive_owner_node(doc: &serde_json::Value) -> Option<usize> {
    let nodes = doc["nodes"].as_array()?;
    nodes.iter().position(|n| n["mesh"].as_u64() == Some(0))
}

/// VRM 0.x spring-bone assets must populate `VRM.humanoid.humanBones` from
/// the skeleton so real renderers (VRMMetalKit, three-vrm, UniVRM) can find
/// the required hips bone and load the avatar successfully.
#[test]
fn spring_bone_v0_asset_has_non_empty_human_bones_with_hips() {
    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::from_path_buf(dir.path().join("sb_v0_hips.vrm")).unwrap();

    let mtoon = MToonParams::defaults("sb_v0_hips");
    let spring = SpringBoneParams::defaults("sb_v0_hips");
    emit_vrm_with_spring_bone_v0(&mtoon, &spring, &out).expect("v0 emission must succeed");

    let bytes = std::fs::read(out.as_std_path()).unwrap();
    let json_bytes = extract_json_chunk(&bytes).expect("GLB must have a JSON chunk");
    let doc: serde_json::Value = serde_json::from_slice(&json_bytes).unwrap();

    let human_bones = doc["extensions"]["VRM"]["humanoid"]["humanBones"]
        .as_array()
        .expect("humanoid.humanBones must be an array");

    assert!(
        !human_bones.is_empty(),
        "humanoid.humanBones must not be empty in a v0 spring-bone asset"
    );

    let hips_entry = human_bones
        .iter()
        .find(|e| e["bone"].as_str() == Some("hips"))
        .expect("humanoid.humanBones must contain a hips entry");

    assert!(
        hips_entry["node"].is_u64(),
        "hips entry must have a numeric node index"
    );
    assert_eq!(
        hips_entry["useDefaultValues"], true,
        "hips entry must have useDefaultValues: true"
    );
}
