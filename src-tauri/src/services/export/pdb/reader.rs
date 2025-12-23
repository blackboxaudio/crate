//! PDB file reader for parsing existing export.pdb files
//!
//! This module implements reading of Rekordbox PDB files to extract
//! tracks, playlists, and lookup tables for merging with new exports.

#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};

use binrw::BinRead;

use super::constants::*;
use super::error::{PdbError, Result};
use super::strings::DeviceSQLString;
use super::tables::TableType;
use super::types::{FileHeader, PageHeader, TableDescriptor};

/// A parsed track from a PDB file
#[derive(Debug, Clone)]
pub struct ParsedTrack {
    pub id: u32,
    pub title: String,
    pub artist_id: u32,
    pub album_id: u32,
    pub genre_id: u32,
    pub key_id: u32,
    pub color_id: u8,
    pub file_path: String,
    pub anlz_path: String,
    pub duration_seconds: u16,
    pub tempo: u32,
}

/// A parsed playlist from a PDB file
#[derive(Debug, Clone)]
pub struct ParsedPlaylist {
    pub id: u32,
    pub parent_id: u32,
    pub name: String,
    pub is_folder: bool,
    pub sort_order: u32,
    pub track_ids: Vec<u32>,
}

/// A complete parsed PDB file
#[derive(Debug, Clone)]
pub struct ParsedPdb {
    pub tracks: Vec<ParsedTrack>,
    pub playlists: Vec<ParsedPlaylist>,
    pub artists: HashMap<u32, String>,
    pub albums: HashMap<u32, (String, u32)>, // (name, artist_id)
    pub genres: HashMap<u32, String>,
    pub keys: HashMap<u32, String>,
    pub next_track_id: u32,
    pub next_artist_id: u32,
    pub next_album_id: u32,
    pub next_genre_id: u32,
    pub next_key_id: u32,
    pub next_playlist_id: u32,
}

impl ParsedPdb {
    /// Parse a PDB file from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);

        // Read file header
        let file_header = FileHeader::read_le(&mut cursor)?;

        if file_header.page_size != PAGE_SIZE {
            return Err(PdbError::Parse(format!(
                "Unexpected page size: {}",
                file_header.page_size
            )));
        }

        // Read table descriptors
        let mut table_descriptors = Vec::new();
        for _ in 0..file_header.num_tables {
            let desc = TableDescriptor::read_le(&mut cursor)?;
            table_descriptors.push(desc);
        }

        // Parse each table type
        let mut tracks = Vec::new();
        let mut artists = HashMap::new();
        let mut albums = HashMap::new();
        let mut genres = HashMap::new();
        let mut keys = HashMap::new();
        let mut playlist_nodes = Vec::new();
        let mut playlist_entries: Vec<(u32, u32, u32)> = Vec::new(); // (entry_idx, track_id, playlist_id)

        for desc in &table_descriptors {
            let table_type = TableType::try_from(desc.table_type)
                .map_err(|_| PdbError::InvalidTableType(desc.table_type))?;

            match table_type {
                TableType::Tracks => {
                    tracks = parse_track_table(data, desc)?;
                }
                TableType::Artists => {
                    artists = parse_artist_table(data, desc)?;
                }
                TableType::Albums => {
                    albums = parse_album_table(data, desc)?;
                }
                TableType::Genres => {
                    genres = parse_genre_table(data, desc)?;
                }
                TableType::Keys => {
                    keys = parse_key_table(data, desc)?;
                }
                TableType::PlaylistTree => {
                    playlist_nodes = parse_playlist_tree_table(data, desc)?;
                }
                TableType::PlaylistEntries => {
                    playlist_entries = parse_playlist_entries_table(data, desc)?;
                }
                _ => {
                    // Skip other table types
                }
            }
        }

        // Build playlists with track IDs
        let mut playlists: Vec<ParsedPlaylist> = playlist_nodes
            .into_iter()
            .map(|(id, parent_id, name, is_folder, sort_order)| ParsedPlaylist {
                id,
                parent_id,
                name,
                is_folder,
                sort_order,
                track_ids: Vec::new(),
            })
            .collect();

        // Associate tracks with playlists
        for (_, track_id, playlist_id) in playlist_entries {
            if let Some(playlist) = playlists.iter_mut().find(|p| p.id == playlist_id) {
                playlist.track_ids.push(track_id);
            }
        }

        // Calculate next IDs
        let next_track_id = tracks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        let next_artist_id = artists.keys().max().copied().unwrap_or(0) + 1;
        let next_album_id = albums.keys().max().copied().unwrap_or(0) + 1;
        let next_genre_id = genres.keys().max().copied().unwrap_or(0) + 1;
        let next_key_id = keys.keys().max().copied().unwrap_or(0) + 1;
        let next_playlist_id = playlists.iter().map(|p| p.id).max().unwrap_or(0) + 1;

        Ok(ParsedPdb {
            tracks,
            playlists,
            artists,
            albums,
            genres,
            keys,
            next_track_id,
            next_artist_id,
            next_album_id,
            next_genre_id,
            next_key_id,
            next_playlist_id,
        })
    }

    /// Find a track by its USB file path
    pub fn find_track_by_path(&self, path: &str) -> Option<&ParsedTrack> {
        let path_lower = path.to_lowercase();
        self.tracks
            .iter()
            .find(|t| t.file_path.to_lowercase() == path_lower)
    }
}

/// Parse rows from a page, handling row groups
fn parse_rows_from_page(page_data: &[u8]) -> Result<Vec<&[u8]>> {
    if page_data.len() < PAGE_SIZE as usize {
        return Err(PdbError::Parse("Page data too short".to_string()));
    }

    // Read page header
    let mut cursor = Cursor::new(page_data);
    let header = PageHeader::read_le(&mut cursor)?;

    let num_row_groups = header.num_row_groups() as usize;
    let num_rows = header.num_rows() as usize;

    if num_rows == 0 || num_row_groups == 0 {
        return Ok(Vec::new());
    }

    let mut rows = Vec::with_capacity(num_rows);

    // Row groups are stored at the end of the page, growing backwards
    // Each row group is 2 bytes (presence flags) + 16 * 2 bytes (offsets) = 34 bytes
    const ROW_GROUP_SIZE: usize = 34;

    for group_idx in 0..num_row_groups {
        // Calculate position of this row group from end of page
        let group_start = PAGE_SIZE as usize - (group_idx + 1) * ROW_GROUP_SIZE;

        if group_start < HEAP_START_OFFSET {
            break;
        }

        // Read presence flags (first 2 bytes of group)
        let presence_flags =
            u16::from_le_bytes([page_data[group_start], page_data[group_start + 1]]);

        // Read row offsets (16 x u16, but stored in reverse order)
        for row_in_group in 0..MAX_ROWS_PER_GROUP {
            // Check if this row is present
            if (presence_flags & (1 << row_in_group)) == 0 {
                continue;
            }

            // Offset is stored at group_start + 2 + (15 - row_in_group) * 2
            let offset_pos = group_start + 2 + (15 - row_in_group) * 2;
            let row_offset =
                u16::from_le_bytes([page_data[offset_pos], page_data[offset_pos + 1]]) as usize;

            // Row offset is relative to heap start (0x28)
            let actual_offset = HEAP_START_OFFSET + row_offset;

            if actual_offset >= group_start {
                // Row data would overlap with row groups
                continue;
            }

            // Find the end of this row (start of next row or row groups)
            // For now, we'll parse up to the next row or the row groups area
            let row_end = group_start;
            let row_data = &page_data[actual_offset..row_end];

            rows.push(row_data);
        }
    }

    Ok(rows)
}

/// Traverse all pages for a table and collect rows
fn collect_table_rows<'a>(data: &'a [u8], desc: &TableDescriptor) -> Result<Vec<&'a [u8]>> {
    let mut all_rows = Vec::new();
    let mut current_page = desc.first_page;

    // Skip the index page (first page is always index)
    if current_page != NULL_PAGE_MARKER {
        let page_offset = current_page as usize * PAGE_SIZE as usize;
        if page_offset + PAGE_SIZE as usize <= data.len() {
            let page_data = &data[page_offset..page_offset + PAGE_SIZE as usize];
            let mut cursor = Cursor::new(page_data);
            let header = PageHeader::read_le(&mut cursor)?;

            // If this is an index page (flags = 0x64), get the next page
            if header.page_flags == PAGE_FLAGS_INDEX {
                current_page = header.next_page;
            }
        }
    }

    // Now traverse data pages
    while current_page != NULL_PAGE_MARKER {
        let page_offset = current_page as usize * PAGE_SIZE as usize;
        if page_offset + PAGE_SIZE as usize > data.len() {
            break;
        }

        let page_data = &data[page_offset..page_offset + PAGE_SIZE as usize];

        // Parse rows from this page
        let rows = parse_rows_from_page(page_data)?;
        all_rows.extend(rows);

        // Get next page
        let mut cursor = Cursor::new(page_data);
        let header = PageHeader::read_le(&mut cursor)?;
        current_page = header.next_page;
    }

    Ok(all_rows)
}

/// Parse track table rows
fn parse_track_table(data: &[u8], desc: &TableDescriptor) -> Result<Vec<ParsedTrack>> {
    let rows = collect_table_rows(data, desc)?;
    let mut tracks = Vec::new();

    for row_data in rows {
        if row_data.len() < 92 {
            // Minimum track header size
            continue;
        }

        // Parse fixed header
        let mut cursor = Cursor::new(row_data);

        // Skip subtype and index_shift (4 bytes)
        cursor.seek(SeekFrom::Current(4))?;

        // Skip bitmask (4 bytes)
        cursor.seek(SeekFrom::Current(4))?;

        // Sample rate (4 bytes) - skip
        cursor.seek(SeekFrom::Current(4))?;

        // Composer ID (4 bytes) - skip
        cursor.seek(SeekFrom::Current(4))?;

        // File size (4 bytes) - skip
        cursor.seek(SeekFrom::Current(4))?;

        // Unknown2 (4 bytes) - skip
        cursor.seek(SeekFrom::Current(4))?;

        // Unknown3/4 (4 bytes) - skip
        cursor.seek(SeekFrom::Current(4))?;

        // Artwork ID (4 bytes) - skip
        cursor.seek(SeekFrom::Current(4))?;

        // Key ID (4 bytes)
        let mut key_id_bytes = [0u8; 4];
        cursor.read_exact(&mut key_id_bytes)?;
        let key_id = u32::from_le_bytes(key_id_bytes);

        // Original artist ID (4 bytes) - skip
        cursor.seek(SeekFrom::Current(4))?;

        // Label ID (4 bytes) - skip
        cursor.seek(SeekFrom::Current(4))?;

        // Remixer ID (4 bytes) - skip
        cursor.seek(SeekFrom::Current(4))?;

        // Bitrate (4 bytes) - skip
        cursor.seek(SeekFrom::Current(4))?;

        // Track number (4 bytes) - skip
        cursor.seek(SeekFrom::Current(4))?;

        // Tempo (4 bytes)
        let mut tempo_bytes = [0u8; 4];
        cursor.read_exact(&mut tempo_bytes)?;
        let tempo = u32::from_le_bytes(tempo_bytes);

        // Genre ID (4 bytes)
        let mut genre_id_bytes = [0u8; 4];
        cursor.read_exact(&mut genre_id_bytes)?;
        let genre_id = u32::from_le_bytes(genre_id_bytes);

        // Album ID (4 bytes)
        let mut album_id_bytes = [0u8; 4];
        cursor.read_exact(&mut album_id_bytes)?;
        let album_id = u32::from_le_bytes(album_id_bytes);

        // Artist ID (4 bytes)
        let mut artist_id_bytes = [0u8; 4];
        cursor.read_exact(&mut artist_id_bytes)?;
        let artist_id = u32::from_le_bytes(artist_id_bytes);

        // Track ID (4 bytes)
        let mut track_id_bytes = [0u8; 4];
        cursor.read_exact(&mut track_id_bytes)?;
        let id = u32::from_le_bytes(track_id_bytes);

        // Disc number (2 bytes) - skip
        cursor.seek(SeekFrom::Current(2))?;

        // Play count (2 bytes) - skip
        cursor.seek(SeekFrom::Current(2))?;

        // Year (2 bytes) - skip
        cursor.seek(SeekFrom::Current(2))?;

        // Sample depth (2 bytes) - skip
        cursor.seek(SeekFrom::Current(2))?;

        // Duration (2 bytes)
        let mut duration_bytes = [0u8; 2];
        cursor.read_exact(&mut duration_bytes)?;
        let duration_seconds = u16::from_le_bytes(duration_bytes);

        // Unknown5 (2 bytes) - skip
        cursor.seek(SeekFrom::Current(2))?;

        // Color ID (1 byte)
        let mut color_id_byte = [0u8; 1];
        cursor.read_exact(&mut color_id_byte)?;
        let color_id = color_id_byte[0];

        // Now parse strings - skip to string offsets at 0x5C
        // The track has 22 string offsets starting at byte 92
        if row_data.len() < 92 + 44 {
            // Need at least header + offset array
            continue;
        }

        // Read string offsets (22 x u16)
        let offset_start = 92;
        let mut string_offsets = Vec::with_capacity(22);
        for i in 0..22 {
            let offset_pos = offset_start + i * 2;
            if offset_pos + 2 > row_data.len() {
                break;
            }
            let offset = u16::from_le_bytes([row_data[offset_pos], row_data[offset_pos + 1]]);
            string_offsets.push(offset as usize);
        }

        // Parse the strings we care about
        // String indices: 14=ANLZ path, 17=Title, 20=File path
        let title = read_string_at_index(row_data, &string_offsets, 17);
        let anlz_path = read_string_at_index(row_data, &string_offsets, 14);
        let file_path = read_string_at_index(row_data, &string_offsets, 20);

        tracks.push(ParsedTrack {
            id,
            title,
            artist_id,
            album_id,
            genre_id,
            key_id,
            color_id,
            file_path,
            anlz_path,
            duration_seconds,
            tempo,
        });
    }

    Ok(tracks)
}

/// Read a string at a given index from the string offset array
fn read_string_at_index(row_data: &[u8], offsets: &[usize], index: usize) -> String {
    if index >= offsets.len() {
        return String::new();
    }

    let offset = offsets[index];
    if offset == 0 || offset == 3 || offset >= row_data.len() {
        // Offset 3 is the placeholder for first string
        if index == 0 && offsets.len() > 1 {
            // First string, read from offset 92 + 44 = 136
            let start = 136;
            if start < row_data.len() {
                let mut cursor = Cursor::new(&row_data[start..]);
                if let Ok(s) = DeviceSQLString::read_from(&mut cursor) {
                    return s.to_string_lossy();
                }
            }
        }
        return String::new();
    }

    // String offset is relative to start of row
    if offset < row_data.len() {
        let mut cursor = Cursor::new(&row_data[offset..]);
        if let Ok(s) = DeviceSQLString::read_from(&mut cursor) {
            return s.to_string_lossy();
        }
    }

    String::new()
}

/// Parse artist table rows
fn parse_artist_table(data: &[u8], desc: &TableDescriptor) -> Result<HashMap<u32, String>> {
    let rows = collect_table_rows(data, desc)?;
    let mut artists = HashMap::new();

    for row_data in rows {
        if row_data.len() < 10 {
            continue;
        }

        // Format: subtype(2) + index_shift(2) + id(4) + offsets(2) + name
        let id = u32::from_le_bytes([row_data[4], row_data[5], row_data[6], row_data[7]]);

        // Name starts at offset 10
        if row_data.len() > 10 {
            let mut cursor = Cursor::new(&row_data[10..]);
            if let Ok(s) = DeviceSQLString::read_from(&mut cursor) {
                artists.insert(id, s.to_string_lossy());
            }
        }
    }

    Ok(artists)
}

/// Parse album table rows
fn parse_album_table(data: &[u8], desc: &TableDescriptor) -> Result<HashMap<u32, (String, u32)>> {
    let rows = collect_table_rows(data, desc)?;
    let mut albums = HashMap::new();

    for row_data in rows {
        if row_data.len() < 22 {
            continue;
        }

        // Format: subtype(2) + index_shift(2) + unknown(4) + artist_id(4) + id(4) + unknown(4) + offsets(2) + name
        let artist_id = u32::from_le_bytes([row_data[8], row_data[9], row_data[10], row_data[11]]);
        let id = u32::from_le_bytes([row_data[12], row_data[13], row_data[14], row_data[15]]);

        // Name starts at offset 22
        if row_data.len() > 22 {
            let mut cursor = Cursor::new(&row_data[22..]);
            if let Ok(s) = DeviceSQLString::read_from(&mut cursor) {
                albums.insert(id, (s.to_string_lossy(), artist_id));
            }
        }
    }

    Ok(albums)
}

/// Parse genre table rows
fn parse_genre_table(data: &[u8], desc: &TableDescriptor) -> Result<HashMap<u32, String>> {
    let rows = collect_table_rows(data, desc)?;
    let mut genres = HashMap::new();

    for row_data in rows {
        if row_data.len() < 5 {
            continue;
        }

        // Format: id(4) + name
        let id = u32::from_le_bytes([row_data[0], row_data[1], row_data[2], row_data[3]]);

        if row_data.len() > 4 {
            let mut cursor = Cursor::new(&row_data[4..]);
            if let Ok(s) = DeviceSQLString::read_from(&mut cursor) {
                genres.insert(id, s.to_string_lossy());
            }
        }
    }

    Ok(genres)
}

/// Parse key table rows
fn parse_key_table(data: &[u8], desc: &TableDescriptor) -> Result<HashMap<u32, String>> {
    let rows = collect_table_rows(data, desc)?;
    let mut keys = HashMap::new();

    for row_data in rows {
        if row_data.len() < 9 {
            continue;
        }

        // Format: id(4) + id_again(4) + name
        let id = u32::from_le_bytes([row_data[0], row_data[1], row_data[2], row_data[3]]);

        if row_data.len() > 8 {
            let mut cursor = Cursor::new(&row_data[8..]);
            if let Ok(s) = DeviceSQLString::read_from(&mut cursor) {
                keys.insert(id, s.to_string_lossy());
            }
        }
    }

    Ok(keys)
}

/// Parse playlist tree table rows
fn parse_playlist_tree_table(
    data: &[u8],
    desc: &TableDescriptor,
) -> Result<Vec<(u32, u32, String, bool, u32)>> {
    let rows = collect_table_rows(data, desc)?;
    let mut nodes = Vec::new();

    for row_data in rows {
        if row_data.len() < 21 {
            continue;
        }

        // Format: parent_id(4) + unknown(4) + sort_order(4) + id(4) + is_folder(4) + name
        let parent_id = u32::from_le_bytes([row_data[0], row_data[1], row_data[2], row_data[3]]);
        let sort_order = u32::from_le_bytes([row_data[8], row_data[9], row_data[10], row_data[11]]);
        let id = u32::from_le_bytes([row_data[12], row_data[13], row_data[14], row_data[15]]);
        let is_folder =
            u32::from_le_bytes([row_data[16], row_data[17], row_data[18], row_data[19]]) != 0;

        let name = if row_data.len() > 20 {
            let mut cursor = Cursor::new(&row_data[20..]);
            DeviceSQLString::read_from(&mut cursor)
                .map(|s| s.to_string_lossy())
                .unwrap_or_default()
        } else {
            String::new()
        };

        nodes.push((id, parent_id, name, is_folder, sort_order));
    }

    Ok(nodes)
}

/// Parse playlist entries table rows
fn parse_playlist_entries_table(
    data: &[u8],
    desc: &TableDescriptor,
) -> Result<Vec<(u32, u32, u32)>> {
    let rows = collect_table_rows(data, desc)?;
    let mut entries = Vec::new();

    for row_data in rows {
        if row_data.len() < 12 {
            continue;
        }

        // Format: entry_index(4) + track_id(4) + playlist_id(4)
        let entry_index = u32::from_le_bytes([row_data[0], row_data[1], row_data[2], row_data[3]]);
        let track_id = u32::from_le_bytes([row_data[4], row_data[5], row_data[6], row_data[7]]);
        let playlist_id =
            u32::from_le_bytes([row_data[8], row_data[9], row_data[10], row_data[11]]);

        entries.push((entry_index, track_id, playlist_id));
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsed_pdb_empty() {
        // A minimal valid PDB would need proper structure
        // This test just ensures the parser doesn't panic on invalid input
        let result = ParsedPdb::from_bytes(&[]);
        assert!(result.is_err());
    }
}
