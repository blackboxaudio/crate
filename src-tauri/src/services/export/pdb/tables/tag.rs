//! Tag row building for PDB files (exportExt.pdb)

#![allow(dead_code)]

use crate::services::export::pdb::strings::DeviceSQLString;

/// Internal tag data for PDB generation
#[derive(Debug, Clone)]
pub struct PdbTag {
    pub id: u32,
    pub name: String,
    pub category_id: u32,  // 0 if this is a category
    pub category_pos: u32, // Position within category
    pub is_category: bool,
}

/// Internal track-tag association
#[derive(Debug, Clone)]
pub struct PdbTrackTag {
    pub track_id: u32,
    pub tag_id: u32,
}

/// Build a tag row (short variant with u8 offsets)
///
/// Format:
/// - 0x00-0x01: Subtype (0x0680)
/// - 0x02-0x03: Tag index
/// - 0x04-0x0B: Unknown (8 bytes)
/// - 0x0C-0x0F: Category ID
/// - 0x10-0x13: Category position
/// - 0x14-0x17: ID
/// - 0x18-0x1B: Is category flag
/// - 0x1C: Unknown
/// - 0x1D: Flags
/// - 0x1E: Constant (0x03)
/// - 0x1F: Name offset
/// - 0x20: Unknown string offset
/// - 0x21+: Strings
pub fn build_tag_row(tag: &PdbTag, row_index: u16) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x01: Subtype (0x0680 = short variant, u8 offsets)
    row.extend_from_slice(&0x0680u16.to_le_bytes());
    // 0x02-0x03: Tag index (row_index * 0x20)
    row.extend_from_slice(&row_index.wrapping_mul(0x20).to_le_bytes());
    // 0x04-0x0B: Unknown (8 bytes of zeros)
    row.extend_from_slice(&[0u8; 8]);
    // 0x0C-0x0F: Category ID
    row.extend_from_slice(&tag.category_id.to_le_bytes());
    // 0x10-0x13: Category position
    row.extend_from_slice(&tag.category_pos.to_le_bytes());
    // 0x14-0x17: ID
    row.extend_from_slice(&tag.id.to_le_bytes());
    // 0x18-0x1B: Is category flag
    let is_cat_val: u32 = if tag.is_category { 1 } else { 0 };
    row.extend_from_slice(&is_cat_val.to_le_bytes());
    // 0x1C: Unknown
    row.push(0);
    // 0x1D: Flags
    row.push(0);
    // 0x1E: Constant (0x03)
    row.push(0x03);
    // 0x1F: Name offset (points to string at offset 0x21)
    row.push(0x21);
    // 0x20: Unknown string offset
    row.push(0x22);

    // Write the name string
    let name_str = DeviceSQLString::new(&tag.name);
    name_str
        .write_to(&mut row)
        .expect("write to Vec should not fail");

    // Write empty unknown string
    let empty_str = DeviceSQLString::empty();
    empty_str
        .write_to(&mut row)
        .expect("write to Vec should not fail");

    row
}

/// Build a track-tag row
///
/// Format:
/// - 0x00-0x03: Unknown (0)
/// - 0x04-0x07: Track ID
/// - 0x08-0x0B: Tag ID
/// - 0x0C-0x0F: Unknown (0)
pub fn build_track_tag_row(track_tag: &PdbTrackTag) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x03: Unknown (zeros)
    row.extend_from_slice(&0u32.to_le_bytes());
    // 0x04-0x07: Track ID
    row.extend_from_slice(&track_tag.track_id.to_le_bytes());
    // 0x08-0x0B: Tag ID
    row.extend_from_slice(&track_tag.tag_id.to_le_bytes());
    // 0x0C-0x0F: Unknown (zeros)
    row.extend_from_slice(&0u32.to_le_bytes());

    row
}
