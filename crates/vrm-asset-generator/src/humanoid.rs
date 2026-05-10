//! VRM 1.0 minimal humanoid skeleton stub.
//!
//! Emits a fixed A-pose skeleton with the spec-required bones as glTF nodes.
//! Phase 1 material tests don't pose the avatar; the skeleton exists only to
//! satisfy VRMC_vrm.humanoid.humanBones validation. Bone positions are
//! rough A-pose defaults.

use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Skeleton {
    /// glTF `nodes` array fragment.
    pub nodes_json: Value,
    /// Index of the root node (always `hips`'s parent or hips itself).
    pub root_node: usize,
    /// Map of VRM 1.0 bone name to glTF node index.
    pub bone_to_node: BTreeMap<String, usize>,
}

/// Bone definition: (name, parent_bone_or_None, translation_relative_to_parent).
struct B {
    name: &'static str,
    parent: Option<&'static str>,
    t: [f32; 3],
}

fn bones() -> &'static [B] {
    &[
        B {
            name: "hips",
            parent: None,
            t: [0.0, 0.86, 0.0],
        },
        B {
            name: "spine",
            parent: Some("hips"),
            t: [0.0, 0.10, 0.0],
        },
        B {
            name: "chest",
            parent: Some("spine"),
            t: [0.0, 0.10, 0.0],
        },
        B {
            name: "neck",
            parent: Some("chest"),
            t: [0.0, 0.20, 0.0],
        },
        B {
            name: "head",
            parent: Some("neck"),
            t: [0.0, 0.10, 0.0],
        },
        B {
            name: "leftShoulder",
            parent: Some("chest"),
            t: [0.05, 0.18, 0.0],
        },
        B {
            name: "leftUpperArm",
            parent: Some("leftShoulder"),
            t: [0.10, 0.0, 0.0],
        },
        B {
            name: "leftLowerArm",
            parent: Some("leftUpperArm"),
            t: [0.25, 0.0, 0.0],
        },
        B {
            name: "leftHand",
            parent: Some("leftLowerArm"),
            t: [0.25, 0.0, 0.0],
        },
        B {
            name: "rightShoulder",
            parent: Some("chest"),
            t: [-0.05, 0.18, 0.0],
        },
        B {
            name: "rightUpperArm",
            parent: Some("rightShoulder"),
            t: [-0.10, 0.0, 0.0],
        },
        B {
            name: "rightLowerArm",
            parent: Some("rightUpperArm"),
            t: [-0.25, 0.0, 0.0],
        },
        B {
            name: "rightHand",
            parent: Some("rightLowerArm"),
            t: [-0.25, 0.0, 0.0],
        },
        B {
            name: "leftUpperLeg",
            parent: Some("hips"),
            t: [0.10, 0.0, 0.0],
        },
        B {
            name: "leftLowerLeg",
            parent: Some("leftUpperLeg"),
            t: [0.0, -0.40, 0.0],
        },
        B {
            name: "leftFoot",
            parent: Some("leftLowerLeg"),
            t: [0.0, -0.40, 0.0],
        },
        B {
            name: "rightUpperLeg",
            parent: Some("hips"),
            t: [-0.10, 0.0, 0.0],
        },
        B {
            name: "rightLowerLeg",
            parent: Some("rightUpperLeg"),
            t: [0.0, -0.40, 0.0],
        },
        B {
            name: "rightFoot",
            parent: Some("rightLowerLeg"),
            t: [0.0, -0.40, 0.0],
        },
    ]
}

/// Sum of bone translations from the root down to `target_bone`, giving
/// the bone's rest-pose world position. Used to compute inverse-bind
/// matrices for the spring-bone chain (the chain joints' world Y at rest
/// is `world_y(head) - i * segment_length`).
pub fn rest_pose_world_position(target_bone: &str) -> [f32; 3] {
    let bs = bones();
    let by_name: std::collections::BTreeMap<&str, &B> = bs.iter().map(|b| (b.name, b)).collect();
    let mut pos = [0.0_f32; 3];
    let mut cur = Some(target_bone);
    while let Some(name) = cur {
        let b = by_name[name];
        pos[0] += b.t[0];
        pos[1] += b.t[1];
        pos[2] += b.t[2];
        cur = b.parent;
    }
    pos
}

pub fn minimal_skeleton() -> Skeleton {
    let bones = bones();
    let mut bone_to_node = BTreeMap::new();
    for (i, b) in bones.iter().enumerate() {
        bone_to_node.insert(b.name.to_string(), i);
    }

    // Build children arrays.
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); bones.len()];
    for (i, b) in bones.iter().enumerate() {
        if let Some(parent_name) = b.parent {
            let pidx = bone_to_node[parent_name];
            children[pidx].push(i);
        }
    }

    let nodes: Vec<Value> = bones
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let mut node = json!({
                "name": b.name,
                "translation": b.t,
            });
            if !children[i].is_empty() {
                node["children"] = json!(children[i]);
            }
            node
        })
        .collect();

    Skeleton {
        nodes_json: Value::Array(nodes),
        root_node: bone_to_node["hips"],
        bone_to_node,
    }
}

/// Append N child nodes to the parent at `parent_node_index`, forming a
/// linear chain. Each chain node carries `translation = (0, -segment_length_m, 0)`
/// relative to its parent (the chain hangs straight down in rest pose).
/// Returns the new chain node indices in head-to-tail order.
///
/// This mutates `skeleton.nodes_json` in place; the new nodes are appended
/// to the end of the nodes array, and the parent's `children` array gains
/// the index of the first chain node.
pub fn append_spring_chain(
    skeleton: &mut Skeleton,
    parent_node_index: usize,
    joint_count: u32,
    segment_length_m: f32,
) -> Vec<usize> {
    if joint_count == 0 {
        return Vec::new();
    }

    let nodes = skeleton
        .nodes_json
        .as_array_mut()
        .expect("skeleton nodes_json must be an array");
    let mut chain_indices = Vec::with_capacity(joint_count as usize);

    let segment_translation = json!([0.0, -segment_length_m, 0.0]);

    // Reserve indices and emit each chain node. The parent of the first
    // chain node is parent_node_index; the parent of each subsequent chain
    // node is the previous chain node.
    for i in 0..joint_count {
        let my_idx = nodes.len();
        let mut node = json!({
            "name": format!("spring_joint_{i}"),
            "translation": segment_translation.clone(),
        });
        // All but the last chain joint will be assigned a `children`
        // entry pointing at the next joint.
        if i + 1 < joint_count {
            node["children"] = json!([my_idx + 1]);
        }
        nodes.push(node);
        chain_indices.push(my_idx);
    }

    // Wire the first chain node into the parent's children array.
    let parent = nodes
        .get_mut(parent_node_index)
        .expect("parent_node_index out of range");
    let mut parent_children = parent
        .get("children")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    parent_children.push(json!(chain_indices[0]));
    parent["children"] = Value::Array(parent_children);

    chain_indices
}
