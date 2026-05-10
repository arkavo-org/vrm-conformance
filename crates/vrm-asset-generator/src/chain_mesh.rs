//! Cylinder mesh skinned to spring-bone chain joints.
//!
//! Generated meshes for spring-bone tests need to deform under physics —
//! without a mesh weighted to the chain joints, chain motion is invisible
//! and cross-renderer diffing produces a null signal. This module emits
//! a vertical cylinder whose rings of vertices are hard-weighted to the
//! corresponding chain joint, so when a renderer's spring-bone physics
//! moves a joint, the cylinder bends with it.
//!
//! Geometry is authored in **bind-pose world space** — i.e., the rest pose
//! of each joint. The inverse-bind matrices (computed in `emit.rs` based
//! on each joint's world Y position) cancel that out so the skinning math
//! sees the mesh in joint-local space at bind time.
//!
//! ## Status: deferred infrastructure
//!
//! The cylinder + `buffer::pack_sphere_and_chain` + skin JSON wiring all
//! exist and were locally smoke-tested against both adapters. three-vrm
//! renders the result correctly (sphere + chain coexist). VRMMetalKit
//! drops the non-skinned sphere mesh when any skin is present in the
//! glTF document, even with the sphere annotated as `firstPerson.type
//! = "both"` and parented under hips — so adding the chain skin to the
//! spring-bone emit path regresses the avg_luminance property assertions
//! (the sphere is what they measure).
//!
//! Filed upstream as
//! [arkavo-org/VRMMetalKit#181](https://github.com/arkavo-org/VRMMetalKit/issues/181)
//! (non-skinned meshes dropped when skin is present). Until that lands,
//! `emit_vrm_with_spring_bone` keeps the sphere-only mesh. The chain
//! geometry + IBM packing here is unit-tested standalone and ready to
//! wire when the upstream issue is resolved.

use glam::{Vec2, Vec3};

#[derive(Debug, Clone)]
pub struct SkinnedMeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    /// Per-vertex joint indices into the skin's `joints` array.
    /// VEC4 of u16 — hard-weighted, so [joint_idx, 0, 0, 0].
    pub joints: Vec<[u16; 4]>,
    /// Per-vertex weights matching `joints`. Sum to 1.0 per vertex.
    /// Hard-weighted, so [1.0, 0.0, 0.0, 0.0].
    pub weights: Vec<[f32; 4]>,
}

/// Build a vertical cylinder running from `top_world_y` straight down by
/// `joint_count * segment_length_m`. The cylinder has one vertex ring per
/// joint plus one bottom-cap ring (so N+1 rings total for N joints), each
/// with `ring_segments` vertices around the chain axis. Each ring is
/// hard-weighted to its corresponding joint — ring 0 to joint 0, ring 1 to
/// joint 1, ..., ring N to joint N-1 (the tail ring shares joint N-1 so
/// the cylinder ends cleanly at the chain tip).
///
/// X = sideways, Y = down the chain, Z = forward. Cylinder is centered on
/// the chain axis at X=0, Z=0.
pub fn build_chain_cylinder(
    joint_count: u32,
    segment_length_m: f32,
    radius: f32,
    top_world_y: f32,
    ring_segments: u32,
) -> SkinnedMeshData {
    assert!(joint_count > 0, "chain mesh needs at least 1 joint");
    assert!(ring_segments >= 3, "ring needs at least 3 verts");

    let n_rings = joint_count as usize + 1;
    let n_segs = ring_segments as usize;
    let n_verts = n_rings * n_segs;

    let mut positions = Vec::with_capacity(n_verts);
    let mut normals = Vec::with_capacity(n_verts);
    let mut uvs = Vec::with_capacity(n_verts);
    let mut joints = Vec::with_capacity(n_verts);
    let mut weights = Vec::with_capacity(n_verts);

    for ring in 0..n_rings {
        // Ring 0 sits at the top (joint 0 position). Ring N (the tail
        // ring) sits at the chain tip (joint_count * segment below top).
        let y = top_world_y - (ring as f32) * segment_length_m;

        // Each ring is weighted to its corresponding joint. Ring N (the
        // bottom cap) is weighted to the LAST joint (index joint_count-1)
        // so the tail of the cylinder tracks the chain tip.
        let weighted_joint = ring.min(joint_count as usize - 1) as u16;

        for seg in 0..n_segs {
            let phi = (seg as f32) * 2.0 * std::f32::consts::PI / (n_segs as f32);
            let cos_p = phi.cos();
            let sin_p = phi.sin();
            let n = Vec3::new(cos_p, 0.0, sin_p);
            let p = Vec3::new(radius * cos_p, y, radius * sin_p);
            let uv = Vec2::new(
                (seg as f32) / (n_segs as f32),
                (ring as f32) / (n_rings as f32 - 1.0).max(1.0),
            );

            positions.push(p.into());
            normals.push(n.into());
            uvs.push(uv.into());
            joints.push([weighted_joint, 0, 0, 0]);
            weights.push([1.0, 0.0, 0.0, 0.0]);
        }
    }

    // Triangles: between ring r and ring r+1, n_segs quads, each split into
    // two triangles. Total = 2 * n_segs * (n_rings - 1) triangles.
    let mut indices = Vec::with_capacity(2 * n_segs * (n_rings - 1) * 3);
    for r in 0..n_rings - 1 {
        for s in 0..n_segs {
            let s_next = (s + 1) % n_segs;
            let i00 = (r * n_segs + s) as u32;
            let i01 = (r * n_segs + s_next) as u32;
            let i10 = ((r + 1) * n_segs + s) as u32;
            let i11 = ((r + 1) * n_segs + s_next) as u32;
            // Outward-facing (right-handed winding with normals pointing
            // outward): i00, i10, i01; i01, i10, i11.
            indices.extend_from_slice(&[i00, i10, i01, i01, i10, i11]);
        }
    }

    SkinnedMeshData {
        positions,
        normals,
        uvs,
        indices,
        joints,
        weights,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_count_matches_rings_times_segments() {
        let m = build_chain_cylinder(4, 0.05, 0.02, 1.31, 8);
        // 4 joints + 1 tail ring = 5 rings, 8 segments per ring = 40 verts.
        assert_eq!(m.positions.len(), 40);
        assert_eq!(m.normals.len(), 40);
        assert_eq!(m.uvs.len(), 40);
        assert_eq!(m.joints.len(), 40);
        assert_eq!(m.weights.len(), 40);
    }

    #[test]
    fn index_count_matches_quads_per_ring_gap() {
        // 4 ring-gaps × 8 segs × 2 tris × 3 indices = 192 indices.
        let m = build_chain_cylinder(4, 0.05, 0.02, 1.31, 8);
        assert_eq!(m.indices.len(), 192);
        let n_verts = m.positions.len() as u32;
        for &i in &m.indices {
            assert!(i < n_verts, "index {i} out of range ({n_verts})");
        }
    }

    #[test]
    fn ring_zero_sits_at_top_world_y() {
        let m = build_chain_cylinder(4, 0.05, 0.02, 1.31, 8);
        for v in &m.positions[..8] {
            assert!((v[1] - 1.31).abs() < 1e-6, "ring 0 vertex Y = {}", v[1]);
        }
    }

    #[test]
    fn tail_ring_extends_to_chain_tip() {
        // top=1.31, 4 joints @ 0.05 each, tail ring at top - 4*0.05 = 1.11
        let m = build_chain_cylinder(4, 0.05, 0.02, 1.31, 8);
        let last_ring_start = m.positions.len() - 8;
        for v in &m.positions[last_ring_start..] {
            assert!((v[1] - 1.11).abs() < 1e-6, "tail ring Y = {}", v[1]);
        }
    }

    #[test]
    fn each_ring_is_hard_weighted_to_its_joint() {
        let m = build_chain_cylinder(4, 0.05, 0.02, 1.31, 8);
        // Rings 0..3 weight to joints 0..3. Ring 4 (tail) reuses joint 3.
        for ring in 0..5_usize {
            let expected_joint = ring.min(3) as u16;
            for s in 0..8 {
                let idx = ring * 8 + s;
                assert_eq!(
                    m.joints[idx],
                    [expected_joint, 0, 0, 0],
                    "vertex {idx} (ring {ring}, seg {s})"
                );
                assert_eq!(m.weights[idx], [1.0, 0.0, 0.0, 0.0]);
            }
        }
    }

    #[test]
    fn ring_vertices_are_at_correct_radius() {
        let radius = 0.025_f32;
        let m = build_chain_cylinder(4, 0.05, radius, 1.31, 12);
        for v in &m.positions {
            let r = (v[0] * v[0] + v[2] * v[2]).sqrt();
            assert!(
                (r - radius).abs() < 1e-6,
                "vertex at radius {r}, expected {radius}"
            );
        }
    }
}
