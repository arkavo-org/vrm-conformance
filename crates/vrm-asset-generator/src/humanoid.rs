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
#[derive(Clone, Copy)]
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
    build_skeleton(bones())
}

/// Canonical skeleton plus the 30 VRM 1.0 optional finger bones.
/// Opt-in: used only by finger-sweep emission so node indices in the
/// existing corpus stay stable.
pub fn skeleton_with_fingers() -> Skeleton {
    let mut all: Vec<B> = bones().to_vec();
    all.extend_from_slice(finger_bones());
    build_skeleton(&all)
}

fn build_skeleton(bones: &[B]) -> Skeleton {
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

/// VRM 1.0 optional finger bones (15 per hand). Rest translations are
/// rough plausible defaults relative to the hand bone — the conformance
/// signal is the dumped rotation quaternion, not anatomy. Left hand local
/// +X is distal (toward fingertips); right hand mirrors X.
fn finger_bones() -> &'static [B] {
    &[
        // Left thumb
        B {
            name: "leftThumbMetacarpal",
            parent: Some("leftHand"),
            t: [0.02, -0.01, 0.03],
        },
        B {
            name: "leftThumbProximal",
            parent: Some("leftThumbMetacarpal"),
            t: [0.03, 0.0, 0.01],
        },
        B {
            name: "leftThumbDistal",
            parent: Some("leftThumbProximal"),
            t: [0.03, 0.0, 0.01],
        },
        // Left index
        B {
            name: "leftIndexProximal",
            parent: Some("leftHand"),
            t: [0.08, 0.0, 0.02],
        },
        B {
            name: "leftIndexIntermediate",
            parent: Some("leftIndexProximal"),
            t: [0.035, 0.0, 0.0],
        },
        B {
            name: "leftIndexDistal",
            parent: Some("leftIndexIntermediate"),
            t: [0.025, 0.0, 0.0],
        },
        // Left middle
        B {
            name: "leftMiddleProximal",
            parent: Some("leftHand"),
            t: [0.085, 0.0, 0.0],
        },
        B {
            name: "leftMiddleIntermediate",
            parent: Some("leftMiddleProximal"),
            t: [0.04, 0.0, 0.0],
        },
        B {
            name: "leftMiddleDistal",
            parent: Some("leftMiddleIntermediate"),
            t: [0.027, 0.0, 0.0],
        },
        // Left ring
        B {
            name: "leftRingProximal",
            parent: Some("leftHand"),
            t: [0.08, 0.0, -0.018],
        },
        B {
            name: "leftRingIntermediate",
            parent: Some("leftRingProximal"),
            t: [0.037, 0.0, 0.0],
        },
        B {
            name: "leftRingDistal",
            parent: Some("leftRingIntermediate"),
            t: [0.025, 0.0, 0.0],
        },
        // Left little
        B {
            name: "leftLittleProximal",
            parent: Some("leftHand"),
            t: [0.07, 0.0, -0.034],
        },
        B {
            name: "leftLittleIntermediate",
            parent: Some("leftLittleProximal"),
            t: [0.03, 0.0, 0.0],
        },
        B {
            name: "leftLittleDistal",
            parent: Some("leftLittleIntermediate"),
            t: [0.02, 0.0, 0.0],
        },
        // Right thumb
        B {
            name: "rightThumbMetacarpal",
            parent: Some("rightHand"),
            t: [-0.02, -0.01, 0.03],
        },
        B {
            name: "rightThumbProximal",
            parent: Some("rightThumbMetacarpal"),
            t: [-0.03, 0.0, 0.01],
        },
        B {
            name: "rightThumbDistal",
            parent: Some("rightThumbProximal"),
            t: [-0.03, 0.0, 0.01],
        },
        // Right index
        B {
            name: "rightIndexProximal",
            parent: Some("rightHand"),
            t: [-0.08, 0.0, 0.02],
        },
        B {
            name: "rightIndexIntermediate",
            parent: Some("rightIndexProximal"),
            t: [-0.035, 0.0, 0.0],
        },
        B {
            name: "rightIndexDistal",
            parent: Some("rightIndexIntermediate"),
            t: [-0.025, 0.0, 0.0],
        },
        // Right middle
        B {
            name: "rightMiddleProximal",
            parent: Some("rightHand"),
            t: [-0.085, 0.0, 0.0],
        },
        B {
            name: "rightMiddleIntermediate",
            parent: Some("rightMiddleProximal"),
            t: [-0.04, 0.0, 0.0],
        },
        B {
            name: "rightMiddleDistal",
            parent: Some("rightMiddleIntermediate"),
            t: [-0.027, 0.0, 0.0],
        },
        // Right ring
        B {
            name: "rightRingProximal",
            parent: Some("rightHand"),
            t: [-0.08, 0.0, -0.018],
        },
        B {
            name: "rightRingIntermediate",
            parent: Some("rightRingProximal"),
            t: [-0.037, 0.0, 0.0],
        },
        B {
            name: "rightRingDistal",
            parent: Some("rightRingIntermediate"),
            t: [-0.025, 0.0, 0.0],
        },
        // Right little
        B {
            name: "rightLittleProximal",
            parent: Some("rightHand"),
            t: [-0.07, 0.0, -0.034],
        },
        B {
            name: "rightLittleIntermediate",
            parent: Some("rightLittleProximal"),
            t: [-0.03, 0.0, 0.0],
        },
        B {
            name: "rightLittleDistal",
            parent: Some("rightLittleIntermediate"),
            t: [-0.02, 0.0, 0.0],
        },
    ]
}

/// Append N child nodes to the parent at `parent_node_index`, forming a
/// linear chain. By default each chain node carries
/// `translation = (0, -segment_length_m, 0)` relative to its parent (the
/// chain hangs straight down in rest pose). Use [`append_spring_chain_axis`]
/// to orient the chain in an arbitrary direction.
///
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
    append_spring_chain_axis(
        skeleton,
        parent_node_index,
        joint_count,
        segment_length_m,
        [0.0, -1.0, 0.0],
    )
}

/// Like [`append_spring_chain`] but lays the chain along `chain_axis`
/// (a unit direction in the parent's local space) instead of straight -Y.
///
/// Each chain node's `translation` is `chain_axis * segment_length_m`.
/// Returns the new chain node indices in head-to-tail order.
pub fn append_spring_chain_axis(
    skeleton: &mut Skeleton,
    parent_node_index: usize,
    joint_count: u32,
    segment_length_m: f32,
    chain_axis: [f32; 3],
) -> Vec<usize> {
    if joint_count == 0 {
        return Vec::new();
    }

    let nodes = skeleton
        .nodes_json
        .as_array_mut()
        .expect("skeleton nodes_json must be an array");
    let mut chain_indices = Vec::with_capacity(joint_count as usize);

    let segment_translation = json!([
        chain_axis[0] * segment_length_m,
        chain_axis[1] * segment_length_m,
        chain_axis[2] * segment_length_m,
    ]);

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

#[cfg(test)]
mod chain_axis_tests {
    use super::*;

    #[test]
    fn chain_extends_along_given_axis() {
        let mut sk = minimal_skeleton();
        let head = sk.bone_to_node["head"];
        let idxs = append_spring_chain_axis(&mut sk, head, 3, 0.05, [0.0, 0.0, 1.0]);
        let nodes = sk.nodes_json.as_array().unwrap();
        for &i in &idxs {
            let t = nodes[i]["translation"].as_array().unwrap();
            assert!((t[0].as_f64().unwrap() - 0.0).abs() < 1e-6);
            assert!((t[1].as_f64().unwrap() - 0.0).abs() < 1e-6);
            assert!((t[2].as_f64().unwrap() - 0.05).abs() < 1e-6, "Z segment");
        }
        // leaf (last) has no children -> forces 7cm synthesis in 0.x
        let leaf = *idxs.last().unwrap();
        assert!(nodes[leaf].get("children").is_none());
    }

    #[test]
    fn default_axis_still_points_down() {
        let mut sk = minimal_skeleton();
        let head = sk.bone_to_node["head"];
        let idxs = append_spring_chain(&mut sk, head, 2, 0.05);
        let nodes = sk.nodes_json.as_array().unwrap();
        let t = nodes[idxs[0]]["translation"].as_array().unwrap();
        assert!((t[1].as_f64().unwrap() - (-0.05)).abs() < 1e-6, "-Y");
    }
}

#[cfg(test)]
mod finger_skeleton_tests {
    use super::*;

    #[test]
    fn minimal_skeleton_is_unchanged_at_19_bones() {
        let sk = minimal_skeleton();
        assert_eq!(sk.bone_to_node.len(), 19);
        assert!(!sk.bone_to_node.contains_key("leftIndexProximal"));
    }

    #[test]
    fn finger_skeleton_has_49_bones_with_all_finger_names() {
        let sk = skeleton_with_fingers();
        assert_eq!(sk.bone_to_node.len(), 49, "19 core + 30 finger bones");
        for side in ["left", "right"] {
            for seg in [
                "ThumbMetacarpal",
                "ThumbProximal",
                "ThumbDistal",
                "IndexProximal",
                "IndexIntermediate",
                "IndexDistal",
                "MiddleProximal",
                "MiddleIntermediate",
                "MiddleDistal",
                "RingProximal",
                "RingIntermediate",
                "RingDistal",
                "LittleProximal",
                "LittleIntermediate",
                "LittleDistal",
            ] {
                assert!(
                    sk.bone_to_node.contains_key(&format!("{side}{seg}")),
                    "missing {side}{seg}"
                );
            }
        }
    }

    #[test]
    fn finger_chains_are_parented_to_hands() {
        let sk = skeleton_with_fingers();
        let nodes = sk.nodes_json.as_array().unwrap();
        let left_hand = sk.bone_to_node["leftHand"];
        let left_index_prox = sk.bone_to_node["leftIndexProximal"];
        let hand_children: Vec<u64> = nodes[left_hand]["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_u64().unwrap())
            .collect();
        assert!(hand_children.contains(&(left_index_prox as u64)));
    }

    #[test]
    fn right_fingers_mirror_left_in_x() {
        let sk = skeleton_with_fingers();
        let nodes = sk.nodes_json.as_array().unwrap();
        let l = sk.bone_to_node["leftIndexProximal"];
        let r = sk.bone_to_node["rightIndexProximal"];
        let lx = nodes[l]["translation"][0].as_f64().unwrap();
        let rx = nodes[r]["translation"][0].as_f64().unwrap();
        assert!((lx + rx).abs() < 1e-6, "X must mirror: {lx} vs {rx}");
    }
}
