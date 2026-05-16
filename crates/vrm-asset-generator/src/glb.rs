//! GLB binary container writer (glTF 2.0 binary format).
//!
//! Layout:
//!   Header:   "glTF" + version=2 (u32 LE) + total_length (u32 LE)  = 12 bytes
//!   Chunk 0:  length (u32 LE) + "JSON" + padded JSON bytes (' ' pad)
//!   Chunk 1:  length (u32 LE) + "BIN\0" + padded binary bytes (0 pad)  [optional]

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct GlbDocument {
    pub json: Vec<u8>,
    pub binary: Vec<u8>,
}

const PAD_TO: usize = 4;

fn pad_to(input: &[u8], pad_byte: u8) -> Vec<u8> {
    let pad = (PAD_TO - input.len() % PAD_TO) % PAD_TO;
    let mut out = Vec::with_capacity(input.len() + pad);
    out.extend_from_slice(input);
    out.resize(input.len() + pad, pad_byte);
    out
}

/// Extract the JSON chunk payload from a GLB file's bytes.
/// Returns `None` if the magic or format are invalid.
pub fn extract_json_chunk(bytes: &[u8]) -> Option<Vec<u8>> {
    // Header: 12 bytes ("glTF" + version u32 + total_length u32)
    if bytes.len() < 12 {
        return None;
    }
    if &bytes[0..4] != b"glTF" {
        return None;
    }
    // Chunk 0 header: length u32 + type u32
    if bytes.len() < 20 {
        return None;
    }
    let chunk_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    // chunk type must be "JSON" (0x4E4F534A)
    if &bytes[16..20] != b"JSON" {
        return None;
    }
    let start = 20;
    let end = start + chunk_len;
    if bytes.len() < end {
        return None;
    }
    // Trim trailing space-padding added by write_glb
    let chunk = &bytes[start..end];
    let trimmed = chunk.iter().rposition(|&b| b != b' ').map_or(0, |p| p + 1);
    Some(chunk[..trimmed].to_vec())
}

pub fn write_glb(doc: &GlbDocument) -> Result<Vec<u8>> {
    let json_padded = pad_to(&doc.json, b' ');
    let json_chunk_len = json_padded.len();

    let (bin_padded, bin_chunk_len) = if doc.binary.is_empty() {
        (Vec::new(), 0usize)
    } else {
        let p = pad_to(&doc.binary, 0);
        let l = p.len();
        (p, l)
    };

    let total = 12
        + 8
        + json_chunk_len
        + if bin_chunk_len > 0 {
            8 + bin_chunk_len
        } else {
            0
        };

    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());

    out.extend_from_slice(&(json_chunk_len as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_padded);

    if bin_chunk_len > 0 {
        out.extend_from_slice(&(bin_chunk_len as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin_padded);
    }

    Ok(out)
}
