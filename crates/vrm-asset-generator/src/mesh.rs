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
