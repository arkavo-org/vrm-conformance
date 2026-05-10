use vrm_asset_generator::{buffer::pack_mesh, mesh::sphere};

#[test]
fn pack_sphere_emits_expected_accessors() {
    let m = sphere(1.0, 8, 16);
    let packed = pack_mesh(&m);

    // 4 accessors: positions, normals, uvs, indices.
    let acc = packed.json["accessors"].as_array().unwrap();
    assert_eq!(acc.len(), 4);

    // Positions accessor: VEC3, FLOAT, count = vertex count
    assert_eq!(acc[0]["type"], "VEC3");
    assert_eq!(acc[0]["componentType"], 5126); // GL_FLOAT
    assert_eq!(
        acc[0]["count"].as_u64().unwrap() as usize,
        m.positions.len()
    );

    // Indices accessor: SCALAR, count = m.indices.len()
    assert_eq!(acc[3]["type"], "SCALAR");
    assert_eq!(acc[3]["count"].as_u64().unwrap() as usize, m.indices.len());

    // 4 bufferViews
    let bv = packed.json["bufferViews"].as_array().unwrap();
    assert_eq!(bv.len(), 4);

    // Single buffer with byteLength matching binary blob length
    let buf = &packed.json["buffers"][0];
    assert_eq!(
        buf["byteLength"].as_u64().unwrap() as usize,
        packed.binary.len()
    );

    // Binary length should be 4-aligned (we'll let GLB writer pad if not, but
    // the per-bufferView offsets must align to component size).
    assert!(
        packed.binary.len()
            >= 12 * m.positions.len()
                + 12 * m.normals.len()
                + 8 * m.uvs.len()
                + 4 * m.indices.len(),
        "binary should hold all 4 streams"
    );
}
