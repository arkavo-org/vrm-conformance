use vrm_asset_generator::humanoid::minimal_skeleton;

#[test]
fn skeleton_includes_all_required_bones() {
    let s = minimal_skeleton();
    let required = [
        "hips",
        "spine",
        "chest",
        "neck",
        "head",
        "leftShoulder",
        "leftUpperArm",
        "leftLowerArm",
        "leftHand",
        "rightShoulder",
        "rightUpperArm",
        "rightLowerArm",
        "rightHand",
        "leftUpperLeg",
        "leftLowerLeg",
        "leftFoot",
        "rightUpperLeg",
        "rightLowerLeg",
        "rightFoot",
    ];
    for b in required {
        assert!(s.bone_to_node.contains_key(b), "missing required bone: {b}");
    }
}

#[test]
fn nodes_are_indexed_consistently() {
    let s = minimal_skeleton();
    let nodes = s.nodes_json.as_array().unwrap();
    for (bone, idx) in &s.bone_to_node {
        let node = &nodes[*idx];
        let name = node["name"].as_str().unwrap();
        assert_eq!(name, bone, "bone {bone} maps to node named {name}");
    }
}
