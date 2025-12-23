//! Artist row building for PDB files

use crate::services::export::pdb::strings::DeviceSQLString;

/// Build an artist row (short variant with u8 offsets)
///
/// Format:
/// - 0x00-0x01: Subtype (0x60 = short variant)
/// - 0x02-0x03: Index shift
/// - 0x04-0x07: ID
/// - 0x08-0x09: Offset array [3, name_offset]
/// - 0x0A+: Name string
pub fn build_artist_row(id: u32, name: &str, row_index: u16) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x01: Subtype (0x60 = short variant, u8 offsets)
    row.extend_from_slice(&0x0060u16.to_le_bytes());
    // 0x02-0x03: Index shift
    row.extend_from_slice(&row_index.wrapping_mul(0x20).to_le_bytes());
    // 0x04-0x07: ID
    row.extend_from_slice(&id.to_le_bytes());
    // 0x08-0x09: Offset array [3, name_offset]
    // Name starts at offset 10 (after 8 bytes header + 2 bytes offsets)
    row.push(3); // First offset is always 3
    row.push(10); // Name offset

    // Write the name string
    let name_str = DeviceSQLString::new(name);
    name_str
        .write_to(&mut row)
        .expect("write to Vec should not fail");

    row
}
