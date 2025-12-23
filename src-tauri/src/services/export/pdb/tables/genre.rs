//! Genre row building for PDB files

use crate::services::export::pdb::strings::DeviceSQLString;

/// Build a genre row
///
/// Format:
/// - 0x00-0x03: ID
/// - 0x04+: Name string
pub fn build_genre_row(id: u32, name: &str) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x03: ID
    row.extend_from_slice(&id.to_le_bytes());

    // Name string
    let name_str = DeviceSQLString::new(name);
    name_str
        .write_to(&mut row)
        .expect("write to Vec should not fail");

    row
}
