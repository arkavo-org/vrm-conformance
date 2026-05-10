use vrm_asset_generator::humanoid::{append_spring_chain, minimal_skeleton};

#[test]
fn chain_appends_to_parent_bone() {
    let mut skeleton = minimal_skeleton();
    let head_node = skeleton.bone_to_node["head"];

    let chain = append_spring_chain(&mut skeleton, head_node, 4, 0.05);

    // Returned chain has 4 fresh node indices, all greater than the
    // original max node index in the skeleton.
    assert_eq!(chain.len(), 4);
    let original_node_count = 19; // the 19 humanoid bones
    for &idx in &chain {
        assert!(idx >= original_node_count, "chain node {idx} not appended");
    }
    let unique: std::collections::HashSet<_> = chain.iter().collect();
    assert_eq!(unique.len(), 4, "chain node indices must be unique");

    // The head node's children list now includes the FIRST chain node.
    let nodes = skeleton.nodes_json.as_array().unwrap();
    let head_children = nodes[head_node]["children"]
        .as_array()
        .expect("head node should have children array");
    let head_children_indices: Vec<u64> = head_children.iter().filter_map(|v| v.as_u64()).collect();
    assert!(
        head_children_indices.contains(&(chain[0] as u64)),
        "head's children must include the first chain node (got {head_children_indices:?})",
    );

    // Each chain node (except the last) lists the next chain node as its child.
    for window in chain.windows(2) {
        let (this_idx, next_idx) = (window[0], window[1]);
        let this_node = &nodes[this_idx];
        let children = this_node["children"]
            .as_array()
            .unwrap_or_else(|| panic!("chain node {this_idx} missing children"));
        assert!(
            children.iter().any(|v| v.as_u64() == Some(next_idx as u64)),
            "chain node {this_idx} must list {next_idx} as child",
        );
    }

    // Last chain node has no children array (or empty).
    let last_idx = *chain.last().unwrap();
    let last_node = &nodes[last_idx];
    let last_children = last_node.get("children");
    if let Some(c) = last_children {
        let arr = c.as_array().unwrap();
        assert!(arr.is_empty(), "last chain node must have no children");
    }

    // Each chain node carries a translation matching segment_length_m along Y.
    let first_chain_node = &nodes[chain[0]];
    let t = first_chain_node["translation"].as_array().unwrap();
    let dy = t[1].as_f64().unwrap();
    // Default chain hangs straight down from head; segment 0.05 m.
    assert!(
        (dy - (-0.05)).abs() < 1e-6,
        "first chain joint translation Y should be -0.05, got {dy}",
    );
}

#[test]
fn chain_length_zero_is_a_no_op() {
    let mut skeleton = minimal_skeleton();
    let head_node = skeleton.bone_to_node["head"];
    let before = skeleton.nodes_json.as_array().unwrap().len();

    let chain = append_spring_chain(&mut skeleton, head_node, 0, 0.05);

    let after = skeleton.nodes_json.as_array().unwrap().len();
    assert_eq!(chain.len(), 0);
    assert_eq!(after, before, "zero-length chain should not add nodes");
}
