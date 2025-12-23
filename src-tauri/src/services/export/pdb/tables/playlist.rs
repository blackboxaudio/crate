//! Playlist row building for PDB files

use crate::services::export::pdb::strings::DeviceSQLString;

/// Internal playlist node for PDB generation
#[derive(Debug, Clone)]
pub struct PdbPlaylistNode {
    pub id: u32,
    pub parent_id: u32,
    pub name: String,
    pub is_folder: bool,
    pub sort_order: u32,
}

/// Internal playlist entry for PDB generation
#[derive(Debug, Clone)]
pub struct PdbPlaylistEntry {
    pub entry_index: u32,
    pub track_id: u32,
    pub playlist_id: u32,
}

/// Build a playlist tree row
///
/// Format:
/// - 0x00-0x03: Parent ID
/// - 0x04-0x07: Unknown (0)
/// - 0x08-0x0B: Sort order
/// - 0x0C-0x0F: ID
/// - 0x10-0x13: Is folder flag
/// - 0x14+: Name string
pub fn build_playlist_tree_row(node: &PdbPlaylistNode) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x03: Parent ID
    row.extend_from_slice(&node.parent_id.to_le_bytes());
    // 0x04-0x07: Unknown (0)
    row.extend_from_slice(&0u32.to_le_bytes());
    // 0x08-0x0B: Sort order
    row.extend_from_slice(&node.sort_order.to_le_bytes());
    // 0x0C-0x0F: ID
    row.extend_from_slice(&node.id.to_le_bytes());
    // 0x10-0x13: Is folder flag (u32)
    let is_folder_val: u32 = if node.is_folder { 1 } else { 0 };
    row.extend_from_slice(&is_folder_val.to_le_bytes());

    // Name string
    let name_str = DeviceSQLString::new(&node.name);
    name_str.write_to(&mut row).expect("write to Vec should not fail");

    row
}

/// Build a playlist entry row
///
/// Format:
/// - 0x00-0x03: Entry index
/// - 0x04-0x07: Track ID
/// - 0x08-0x0B: Playlist ID
pub fn build_playlist_entry_row(entry: &PdbPlaylistEntry) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x03: Entry index
    row.extend_from_slice(&entry.entry_index.to_le_bytes());
    // 0x04-0x07: Track ID
    row.extend_from_slice(&entry.track_id.to_le_bytes());
    // 0x08-0x0B: Playlist ID
    row.extend_from_slice(&entry.playlist_id.to_le_bytes());

    row
}
