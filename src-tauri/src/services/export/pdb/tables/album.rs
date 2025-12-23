//! Album row building for PDB files

use crate::services::export::pdb::strings::DeviceSQLString;

/// Build an album row (short variant with u8 offsets)
///
/// Format:
/// - 0x00-0x01: Subtype (0x80 = short variant)
/// - 0x02-0x03: Index shift
/// - 0x04-0x07: Unknown2 (0)
/// - 0x08-0x0B: Artist ID
/// - 0x0C-0x0F: ID
/// - 0x10-0x13: Unknown3 (0)
/// - 0x14-0x15: Offset array [3, name_offset]
/// - 0x16+: Name string
pub fn build_album_row(id: u32, name: &str, artist_id: u32, row_index: u16) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x01: Subtype (0x80 = short variant, u8 offsets)
    row.extend_from_slice(&0x0080u16.to_le_bytes());
    // 0x02-0x03: Index shift
    row.extend_from_slice(&row_index.wrapping_mul(0x20).to_le_bytes());
    // 0x04-0x07: Unknown2 (0)
    row.extend_from_slice(&0u32.to_le_bytes());
    // 0x08-0x0B: Artist ID
    row.extend_from_slice(&artist_id.to_le_bytes());
    // 0x0C-0x0F: ID
    row.extend_from_slice(&id.to_le_bytes());
    // 0x10-0x13: Unknown3 (0)
    row.extend_from_slice(&0u32.to_le_bytes());
    // 0x14-0x15: Offset array [3, name_offset]
    // Name starts at offset 22 (after 20 bytes header + 2 bytes offsets)
    row.push(3); // First offset is always 3
    row.push(22); // Name offset

    // Write the name string
    let name_str = DeviceSQLString::new(name);
    name_str
        .write_to(&mut row)
        .expect("write to Vec should not fail");

    row
}
