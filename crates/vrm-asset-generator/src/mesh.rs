//! Generated mesh fixtures used for material isolation tests.
//!
//! The sphere is the default — material tests want to isolate the MToon
//! math, not test geometry rendering, so the mesh is intentionally minimal
//! and constant across all material parameter combinations.

use glam::{Vec2, Vec3};

#[derive(Debug, Clone)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// UV-sphere with `lat_segments × lon_segments` quads, split into triangles.
/// Defaults of (32, 64) give a smooth-enough sphere without bloating the
/// generated `.glb` files.
pub fn sphere(radius: f32, lat_segments: u32, lon_segments: u32) -> MeshData {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for lat in 0..=lat_segments {
        let theta = lat as f32 * std::f32::consts::PI / lat_segments as f32;
        let sin_t = theta.sin();
        let cos_t = theta.cos();

        for lon in 0..=lon_segments {
            let phi = lon as f32 * 2.0 * std::f32::consts::PI / lon_segments as f32;
            let sin_p = phi.sin();
            let cos_p = phi.cos();

            let n = Vec3::new(cos_p * sin_t, cos_t, sin_p * sin_t);
            let p = n * radius;
            let uv = Vec2::new(
                lon as f32 / lon_segments as f32,
                lat as f32 / lat_segments as f32,
            );

            positions.push(p.into());
            normals.push(n.into());
            uvs.push(uv.into());
        }
    }

    let row = lon_segments + 1;
    for lat in 0..lat_segments {
        for lon in 0..lon_segments {
            let i0 = lat * row + lon;
            let i1 = i0 + row;
            indices.extend_from_slice(&[i0, i1, i0 + 1, i0 + 1, i1, i1 + 1]);
        }
    }

    MeshData {
        positions,
        normals,
        uvs,
        indices,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_is_topologically_consistent() {
        let m = sphere(1.0, 8, 16);
        let expected_verts = (8 + 1) * (16 + 1);
        assert_eq!(m.positions.len(), expected_verts);
        assert_eq!(m.normals.len(), expected_verts);
        assert_eq!(m.uvs.len(), expected_verts);
        // Two triangles per quad, 8*16 quads.
        assert_eq!(m.indices.len(), 8 * 16 * 6);

        // Every index must be in range.
        let max = m.indices.iter().max().copied().unwrap_or(0);
        assert!((max as usize) < m.positions.len());
    }

    #[test]
    fn sphere_normals_are_unit_length() {
        let m = sphere(2.5, 8, 16);
        for n in &m.normals {
            let len2 = n[0] * n[0] + n[1] * n[1] + n[2] * n[2];
            assert!((len2 - 1.0).abs() < 1e-5, "normal not unit: {n:?}");
        }
    }

    #[test]
    fn sphere_radius_is_respected() {
        let m = sphere(3.7, 4, 8);
        for p in &m.positions {
            let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            assert!((r - 3.7).abs() < 1e-5, "position off sphere: {p:?}");
        }
    }
}

/// Open single-quad fixture in the XY plane at z=0, front face toward +Z.
///
/// Unlike `sphere()` (closed + convex, where back-faces are never visible),
/// this open quad lets a renderer's back-face culling be *observed*: viewed
/// from behind (−Z side), a spec-conformant renderer culls it when the
/// material's `doubleSided == false` and renders it when `true`
/// (glTF 2.0 §Materials). Used by the doubleSided culling conformance test.
///
/// Winding matches the +Z front-face convention: the geometric normal of
/// every triangle (CCW front) points +Z. `half_extent` is the half-side
/// length, so the quad spans `[-half_extent, +half_extent]` in X and Y.
pub fn quad(half_extent: f32) -> MeshData {
    let s = half_extent;
    let positions = vec![
        [-s, -s, 0.0], // 0 bottom-left
        [s, -s, 0.0],  // 1 bottom-right
        [s, s, 0.0],   // 2 top-right
        [-s, s, 0.0],  // 3 top-left
    ];
    let normals = vec![[0.0, 0.0, 1.0]; 4];
    let uvs = vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    // CCW when viewed from +Z → geometric normal +Z (front-facing toward +Z).
    let indices = vec![0, 1, 2, 0, 2, 3];
    MeshData {
        positions,
        normals,
        uvs,
        indices,
    }
}

#[cfg(test)]
mod quad_tests {
    use super::*;

    #[test]
    fn quad_has_4_verts_2_triangles() {
        let q = quad(0.5);
        assert_eq!(q.positions.len(), 4);
        assert_eq!(q.normals.len(), 4);
        assert_eq!(q.uvs.len(), 4);
        assert_eq!(q.indices.len(), 6, "two triangles = 6 indices");
    }

    #[test]
    fn quad_front_face_normal_points_plus_z() {
        // Self-verifying winding: the geometric normal of every triangle
        // (cross product of its edges, in index order) MUST point +Z, so a
        // renderer treating CCW-from-+Z as front-facing sees this quad's
        // front from a +Z camera and its BACK from a −Z camera. If a future
        // edit flips the winding, this test fails rather than silently
        // inverting the doubleSided spec test.
        let q = quad(0.5);
        for tri in q.indices.chunks(3) {
            let p0 = q.positions[tri[0] as usize];
            let p1 = q.positions[tri[1] as usize];
            let p2 = q.positions[tri[2] as usize];
            let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            // cross(e1, e2).z
            let nz = e1[0] * e2[1] - e1[1] * e2[0];
            assert!(
                nz > 0.0,
                "triangle {tri:?} geometric normal must point +Z, got nz={nz}"
            );
        }
    }

    #[test]
    fn quad_stored_normals_match_geometry() {
        let q = quad(1.0);
        for n in &q.normals {
            assert_eq!(*n, [0.0, 0.0, 1.0]);
        }
    }
}
