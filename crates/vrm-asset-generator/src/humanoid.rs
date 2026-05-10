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
