use vrm_asset_generator::glb::{write_glb, GlbDocument};

#[test]
fn writes_glb_with_valid_magic_and_chunks() {
    let doc = GlbDocument {
        json: br#"{"asset":{"version":"2.0"}}"#.to_vec(),
        binary: vec![0u8; 16],
    };
    let bytes = write_glb(&doc).unwrap();

    assert_eq!(&bytes[0..4], b"glTF");
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert_eq!(version, 2);
    let total = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    assert_eq!(total, bytes.len());

    // First chunk = JSON
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    assert_eq!(&bytes[16..20], b"JSON");
    assert!(json_len % 4 == 0, "json chunk must be 4-byte aligned");

    // Second chunk = BIN
    let bin_offset = 20 + json_len;
    let bin_len =
        u32::from_le_bytes(bytes[bin_offset..bin_offset + 4].try_into().unwrap()) as usize;
    assert_eq!(&bytes[bin_offset + 4..bin_offset + 8], b"BIN\0");
    assert!(bin_len % 4 == 0, "bin chunk must be 4-byte aligned");
}

#[test]
fn empty_binary_omits_bin_chunk() {
    let doc = GlbDocument {
        json: br#"{"asset":{"version":"2.0"}}"#.to_vec(),
        binary: Vec::new(),
    };
    let bytes = write_glb(&doc).unwrap();

    // Only JSON chunk; BIN chunk is optional per spec.
    let total = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    assert_eq!(total, 12 + 8 + json_len, "no BIN chunk should be present");
}
