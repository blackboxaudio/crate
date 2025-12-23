//! Color row building for PDB files

use crate::services::export::pdb::strings::DeviceSQLString;

/// Standard Rekordbox color definitions
pub const STANDARD_COLORS: &[(u8, &str)] = &[
    (1, "Pink"),
    (2, "Red"),
    (3, "Orange"),
    (4, "Yellow"),
    (5, "Green"),
    (6, "Aqua"),
    (7, "Blue"),
    (8, "Purple"),
];

/// Map color name to Rekordbox color ID
pub fn color_name_to_id(name: &str) -> u8 {
    match name.to_lowercase().as_str() {
        "pink" => 1,
        "red" => 2,
        "orange" => 3,
        "yellow" => 4,
        "green" => 5,
        "aqua" | "cyan" | "teal" => 6,
        "blue" => 7,
        "purple" | "violet" => 8,
        _ => 0, // No color
    }
}

/// Build a color row
///
/// Format:
/// - 0x00-0x03: Unknown1 (0)
/// - 0x04: Unknown2 (same as color ID)
/// - 0x05: Color index
/// - 0x06-0x07: Unknown3 (0)
/// - 0x08+: Name string
pub fn build_color_row(id: u8, name: &str) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x03: Unknown1 (0)
    row.extend_from_slice(&0u32.to_le_bytes());
    // 0x04: Unknown2 (same as color ID)
    row.push(id);
    // 0x05: Color index
    row.push(id);
    // 0x06-0x07: Unknown3 (0)
    row.extend_from_slice(&0u16.to_le_bytes());

    // Name string
    let name_str = DeviceSQLString::new(name);
    name_str
        .write_to(&mut row)
        .expect("write to Vec should not fail");

    row
}
