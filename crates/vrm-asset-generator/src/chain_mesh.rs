//! Cylinder mesh skinned to spring-bone chain joints.
//!
//! Generated meshes for spring-bone tests need to deform under physics —
//! without a mesh weighted to the chain joints, chain motion is invisible
//! and cross-renderer diffing produces a null signal. This module emits
//! a cylinder whose rings of vertices are hard-weighted to the corresponding
//! chain joint, so when a renderer's spring-bone physics moves a joint, the
//! cylinder bends with it.
//!
//! Geometry is authored in **bind-pose world space** — i.e., the rest pose
//! of each joint. The inverse-bind matrices (computed in `emit.rs` based
//! on each joint's world position) cancel that out so the skinning math
//! sees the mesh in joint-local space at bind time.
//!
//! ## Status: wired
//!
//! The chain cylinder is emitted alongside the head sphere by both
//! `emit_vrm_with_spring_bone` (1.0) and `emit_vrm_with_spring_bone_v0` (0.x).
//! VRMMetalKit 0.13.1 closed the non-skinned-mesh-drop bug
//! ([VRMMetalKit#181](https://github.com/arkavo-org/VRMMetalKit/issues/181)),
//! so sphere + chain coexist across all renderers.

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

/// Build a cylinder of `joint_count + 1` rings starting at `top_world` and
/// stepping `segment_length_m` along `axis` per ring. Each ring is a circle
/// of `ring_segments` verts in the plane perpendicular to `axis`, hard-weighted
/// to its joint (ring N reuses joint N-1 so the tail caps cleanly).
///
/// For `axis` parallel to ±Y the in-plane basis is pinned to (+X, +Z) so the
/// historical vertical layout is reproduced byte-for-byte.
pub fn build_chain_cylinder(
    joint_count: u32,
    segment_length_m: f32,
    radius: f32,
    top_world: [f32; 3],
    axis: [f32; 3],
    ring_segments: u32,
) -> SkinnedMeshData {
    assert!(joint_count > 0, "chain mesh needs at least 1 joint");
    assert!(ring_segments >= 3, "ring needs at least 3 verts");

    let n_rings = joint_count as usize + 1;
    let n_segs = ring_segments as usize;
    let n_verts = n_rings * n_segs;

    let a = Vec3::from_array(axis).normalize();
    let top = Vec3::from_array(top_world);
    let (u, v) = perp_basis(a);

    let mut positions = Vec::with_capacity(n_verts);
    let mut normals = Vec::with_capacity(n_verts);
    let mut uvs = Vec::with_capacity(n_verts);
    let mut joints = Vec::with_capacity(n_verts);
    let mut weights = Vec::with_capacity(n_verts);

    for ring in 0..n_rings {
        let center = top + a * (ring as f32 * segment_length_m);
        let weighted_joint = ring.min(joint_count as usize - 1) as u16;

        for seg in 0..n_segs {
            let phi = (seg as f32) * 2.0 * std::f32::consts::PI / (n_segs as f32);
            let radial = u * phi.cos() + v * phi.sin();
            let p = center + radial * radius;
            let uv = Vec2::new(
                (seg as f32) / (n_segs as f32),
                (ring as f32) / (n_rings as f32 - 1.0).max(1.0),
            );

            positions.push(p.into());
            normals.push(radial.into());
            uvs.push(uv.into());
            joints.push([weighted_joint, 0, 0, 0]);
            weights.push([1.0, 0.0, 0.0, 0.0]);
        }
    }

    let mut indices = Vec::with_capacity(2 * n_segs * (n_rings - 1) * 3);
    for r in 0..n_rings - 1 {
        for s in 0..n_segs {
            let s_next = (s + 1) % n_segs;
            let i00 = (r * n_segs + s) as u32;
            let i01 = (r * n_segs + s_next) as u32;
            let i10 = ((r + 1) * n_segs + s) as u32;
            let i11 = ((r + 1) * n_segs + s_next) as u32;
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

/// Orthonormal basis (u, v) spanning the plane perpendicular to unit `a`.
/// Pinned to (+X, +Z) when `a` is parallel to ±Y so the legacy vertical
/// cylinder is reproduced exactly.
fn perp_basis(a: Vec3) -> (Vec3, Vec3) {
    if a.x.abs() < 1e-6 && a.z.abs() < 1e-6 {
        (Vec3::X, Vec3::Z)
    } else {
        let u = a.cross(Vec3::Y).normalize();
        let v = a.cross(u).normalize();
        (u, v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: default -Y axis reproduces the legacy vertical layout.
    fn down_cyl(joints: u32, seg: f32, r: f32, top_y: f32, segs: u32) -> SkinnedMeshData {
        build_chain_cylinder(joints, seg, r, [0.0, top_y, 0.0], [0.0, -1.0, 0.0], segs)
    }

    #[test]
    fn vertex_count_matches_rings_times_segments() {
        let m = down_cyl(4, 0.05, 0.02, 1.31, 8);
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
        let m = down_cyl(4, 0.05, 0.02, 1.31, 8);
        assert_eq!(m.indices.len(), 192);
        let n_verts = m.positions.len() as u32;
        for &i in &m.indices {
            assert!(i < n_verts, "index {i} out of range ({n_verts})");
        }
    }

    #[test]
    fn ring_zero_sits_at_top_world_y() {
        let m = down_cyl(4, 0.05, 0.02, 1.31, 8);
        for v in &m.positions[..8] {
            assert!((v[1] - 1.31).abs() < 1e-6, "ring 0 vertex Y = {}", v[1]);
        }
    }

    #[test]
    fn tail_ring_extends_to_chain_tip() {
        // top=1.31, 4 joints @ 0.05 each, tail ring at top - 4*0.05 = 1.11
        let m = down_cyl(4, 0.05, 0.02, 1.31, 8);
        let last_ring_start = m.positions.len() - 8;
        for v in &m.positions[last_ring_start..] {
            assert!((v[1] - 1.11).abs() < 1e-6, "tail ring Y = {}", v[1]);
        }
    }

    #[test]
    fn each_ring_is_hard_weighted_to_its_joint() {
        let m = down_cyl(4, 0.05, 0.02, 1.31, 8);
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
        let m = down_cyl(4, 0.05, radius, 1.31, 12);
        for v in &m.positions {
            let r = (v[0] * v[0] + v[2] * v[2]).sqrt();
            assert!(
                (r - radius).abs() < 1e-6,
                "vertex at radius {r}, expected {radius}"
            );
        }
    }

    #[test]
    fn default_axis_reproduces_legacy_vertical_positions() {
        let m = down_cyl(4, 0.05, 0.02, 1.31, 8);
        for v in &m.positions[..8] {
            assert!((v[1] - 1.31).abs() < 1e-6, "ring0 Y={}", v[1]);
            let r = (v[0] * v[0] + v[2] * v[2]).sqrt();
            assert!((r - 0.02).abs() < 1e-6);
        }
        let last = m.positions.len() - 8;
        for v in &m.positions[last..] {
            assert!((v[1] - 1.11).abs() < 1e-6, "tail Y={}", v[1]);
        }
    }

    #[test]
    fn forward_axis_walks_along_z() {
        let m = build_chain_cylinder(2, 0.05, 0.02, [0.0, 1.16, 0.0], [0.0, 0.0, 1.0], 8);
        let last = m.positions.len() - 8;
        let cz: f32 = m.positions[last..].iter().map(|v| v[2]).sum::<f32>() / 8.0;
        assert!((cz - 0.10).abs() < 1e-5, "tail ring center Z={cz}");
        for v in &m.positions[last..] {
            assert!((v[2] - 0.10).abs() < 1e-5, "tail vert Z={}", v[2]);
        }
    }
}
