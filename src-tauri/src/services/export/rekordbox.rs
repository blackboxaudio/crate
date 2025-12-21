//! Rekordbox PDB file writer
//!
//! Generates export.pdb files compatible with Pioneer CDJ/XDJ equipment.
//! Based on the Deep Symmetry analysis: https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Seek, Write};
use std::path::Path;

use crate::error::{CrateError, Result};
use crate::models::{Playlist, Track};

/// Page size for PDB files (standard is 4096 bytes)
const PAGE_SIZE: u32 = 4096;

/// Table types in the PDB format
#[repr(u32)]
#[derive(Clone, Copy, Debug)]
enum TableType {
    Tracks = 0,
    Genres = 1,
    Artists = 2,
    Albums = 3,
    Labels = 4,
    Keys = 5,
    Colors = 6,
    PlaylistTree = 7,
    PlaylistEntries = 8,
    #[allow(dead_code)]
    Artwork = 13,
}

/// A track entry for the PDB
#[derive(Debug, Clone)]
struct PdbTrack {
    id: u32,
    title: String,
    artist_id: u32,
    album_id: u32,
    genre_id: u32,
    key_id: u32,
    color_id: u32,
    duration_seconds: u32,
    tempo: u32, // BPM * 100
    bitrate: u32,
    file_path: String,
    rating: u8,
}

/// A playlist tree entry
#[derive(Debug, Clone)]
struct PdbPlaylistNode {
    id: u32,
    parent_id: u32,
    name: String,
    is_folder: bool,
    sort_order: u32,
}

/// A playlist entry (track in playlist)
#[derive(Debug, Clone)]
struct PdbPlaylistEntry {
    entry_index: u32,
    track_id: u32,
    playlist_id: u32,
}

/// Rekordbox PDB file writer
pub struct RekordboxPdbWriter {
    // Deduplicated lookup tables
    artists: HashMap<String, u32>,
    albums: HashMap<String, u32>,
    genres: HashMap<String, u32>,
    keys: HashMap<String, u32>,
    colors: HashMap<String, u32>,

    // Main data
    tracks: Vec<PdbTrack>,
    playlist_nodes: Vec<PdbPlaylistNode>,
    playlist_entries: Vec<PdbPlaylistEntry>,

    // ID counters
    next_track_id: u32,
    next_artist_id: u32,
    next_album_id: u32,
    next_genre_id: u32,
    next_key_id: u32,
    next_color_id: u32,
    next_playlist_id: u32,
    next_entry_index: u32,

    // Playlist ID mapping (Crate ID -> PDB ID)
    playlist_id_map: HashMap<String, u32>,
}

impl RekordboxPdbWriter {
    /// Create a new empty PDB writer
    pub fn new() -> Self {
        Self {
            artists: HashMap::new(),
            albums: HashMap::new(),
            genres: HashMap::new(),
            keys: HashMap::new(),
            colors: HashMap::new(),
            tracks: Vec::new(),
            playlist_nodes: Vec::new(),
            playlist_entries: Vec::new(),
            next_track_id: 1,
            next_artist_id: 1,
            next_album_id: 1,
            next_genre_id: 1,
            next_key_id: 1,
            next_color_id: 1,
            next_playlist_id: 1,
            next_entry_index: 1,
            playlist_id_map: HashMap::new(),
        }
    }

    /// Create from existing PDB data (for merging)
    pub fn from_existing(_data: &[u8]) -> Result<Self> {
        // For now, start fresh - merging with existing PDB is complex
        // In a full implementation, we'd parse the existing PDB using rekordcrate
        // and extract the max IDs to continue from
        log::warn!("Merging with existing PDB not yet implemented, starting fresh");

        // TODO: Parse existing PDB to find max IDs
        // For now, we'll overwrite the existing PDB
        Ok(Self::new())
    }

    /// Add a track and return its PDB ID
    pub fn add_track(&mut self, track: &Track, usb_path: &str) -> u32 {
        let id = self.next_track_id;
        self.next_track_id += 1;

        // Get or create artist
        let artist_name = track.artist.clone().unwrap_or_else(|| "Unknown Artist".to_string());
        let artist_id = *self.artists.entry(artist_name).or_insert_with(|| {
            let id = self.next_artist_id;
            self.next_artist_id += 1;
            id
        });

        // Get or create album
        let album_name = track.album.clone().unwrap_or_else(|| "Unknown Album".to_string());
        let album_id = *self.albums.entry(album_name).or_insert_with(|| {
            let id = self.next_album_id;
            self.next_album_id += 1;
            id
        });

        // Get or create genre
        let genre_name = track.genre.clone().unwrap_or_else(|| "Unknown".to_string());
        let genre_id = *self.genres.entry(genre_name).or_insert_with(|| {
            let id = self.next_genre_id;
            self.next_genre_id += 1;
            id
        });

        // Get or create key
        let key_name = track.key.clone().unwrap_or_else(|| "".to_string());
        let key_id = if key_name.is_empty() {
            0
        } else {
            *self.keys.entry(key_name).or_insert_with(|| {
                let id = self.next_key_id;
                self.next_key_id += 1;
                id
            })
        };

        // Get or create color
        let color_name = track.color.clone().unwrap_or_else(|| "".to_string());
        let color_id = if color_name.is_empty() {
            0
        } else {
            *self.colors.entry(color_name).or_insert_with(|| {
                let id = self.next_color_id;
                self.next_color_id += 1;
                id
            })
        };

        // Calculate tempo (BPM * 100)
        let tempo = track.bpm.map(|b| (b * 100.0) as u32).unwrap_or(0);

        // Build the USB file path (with leading /)
        let file_path = format!("/Contents/{}", usb_path);

        let pdb_track = PdbTrack {
            id,
            title: track.title.clone().unwrap_or_else(|| "Untitled".to_string()),
            artist_id,
            album_id,
            genre_id,
            key_id,
            color_id,
            duration_seconds: (track.duration_ms / 1000) as u32,
            tempo,
            bitrate: track.bitrate.unwrap_or(0) as u32,
            file_path,
            rating: track.rating as u8,
        };

        self.tracks.push(pdb_track);
        id
    }

    /// Add a playlist to the PDB
    pub fn add_playlist(&mut self, playlist: &Playlist, track_ids: &[u32]) {
        // Map the Crate playlist ID to a PDB ID
        let pdb_id = if let Some(&existing_id) = self.playlist_id_map.get(&playlist.id) {
            existing_id
        } else {
            let id = self.next_playlist_id;
            self.next_playlist_id += 1;
            self.playlist_id_map.insert(playlist.id.clone(), id);
            id
        };

        // Get parent PDB ID
        let parent_pdb_id = playlist
            .parent_id
            .as_ref()
            .and_then(|pid| self.playlist_id_map.get(pid))
            .copied()
            .unwrap_or(0);

        // Add playlist node
        let node = PdbPlaylistNode {
            id: pdb_id,
            parent_id: parent_pdb_id,
            name: playlist.name.clone(),
            is_folder: playlist.is_folder,
            sort_order: playlist.sort_order as u32,
        };
        self.playlist_nodes.push(node);

        // Add playlist entries (tracks)
        for &track_id in track_ids {
            let entry = PdbPlaylistEntry {
                entry_index: self.next_entry_index,
                track_id,
                playlist_id: pdb_id,
            };
            self.next_entry_index += 1;
            self.playlist_entries.push(entry);
        }
    }

    /// Write the PDB file to disk
    pub fn write(&self, path: &Path) -> Result<()> {
        let file = File::create(path)
            .map_err(|e| CrateError::Device(format!("Failed to create PDB file: {}", e)))?;
        let mut writer = BufWriter::new(file);

        // Write the PDB structure
        self.write_pdb(&mut writer)?;

        writer
            .flush()
            .map_err(|e| CrateError::Device(format!("Failed to flush PDB file: {}", e)))?;

        Ok(())
    }

    /// Write the complete PDB structure
    fn write_pdb<W: Write + Seek>(&self, writer: &mut W) -> Result<()> {
        // PDB file structure:
        // 1. File header (first page)
        // 2. Table data pages (multiple pages per table)
        //
        // For simplicity, we'll write a minimal but valid PDB with:
        // - Header page with table descriptors
        // - One page per table with row data

        // Calculate number of tables we need
        let _num_tables = 9; // tracks, genres, artists, albums, labels, keys, colors, playlist_tree, playlist_entries

        // Prepare table data
        let genres_data = self.build_string_table_data(&self.genres);
        let artists_data = self.build_string_table_data(&self.artists);
        let albums_data = self.build_album_table_data();
        let labels_data: Vec<u8> = vec![]; // Empty for now
        let keys_data = self.build_string_table_data(&self.keys);
        let colors_data = self.build_color_table_data();
        let tracks_data = self.build_tracks_table_data();
        let playlist_tree_data = self.build_playlist_tree_data();
        let playlist_entries_data = self.build_playlist_entries_data();

        // Calculate pages needed for each table
        let pages_per_table = |data_len: usize| -> u32 {
            if data_len == 0 {
                1
            } else {
                ((data_len as u32) / (PAGE_SIZE - 40)) + 1
            }
        };

        // Header takes first page
        let mut current_page: u32 = 1;

        // Track table page assignments
        let mut table_first_pages: Vec<u32> = Vec::new();
        let mut table_last_pages: Vec<u32> = Vec::new();

        // Assign pages for each table in order
        for data in [
            &tracks_data,
            &genres_data,
            &artists_data,
            &albums_data,
            &labels_data,
            &keys_data,
            &colors_data,
            &playlist_tree_data,
            &playlist_entries_data,
        ] {
            let pages = pages_per_table(data.len());
            table_first_pages.push(current_page);
            table_last_pages.push(current_page + pages - 1);
            current_page += pages;
        }

        let total_pages = current_page;

        // Write header page
        self.write_header_page(writer, total_pages, &table_first_pages, &table_last_pages)?;

        // Write table data pages
        let all_data = [
            (TableType::Tracks as u8, &tracks_data),
            (TableType::Genres as u8, &genres_data),
            (TableType::Artists as u8, &artists_data),
            (TableType::Albums as u8, &albums_data),
            (TableType::Labels as u8, &labels_data),
            (TableType::Keys as u8, &keys_data),
            (TableType::Colors as u8, &colors_data),
            (TableType::PlaylistTree as u8, &playlist_tree_data),
            (TableType::PlaylistEntries as u8, &playlist_entries_data),
        ];

        for (idx, (table_type, data)) in all_data.iter().enumerate() {
            self.write_table_pages(
                writer,
                *table_type,
                data,
                table_first_pages[idx],
                table_last_pages[idx],
            )?;
        }

        Ok(())
    }

    /// Write the file header page
    fn write_header_page<W: Write + Seek>(
        &self,
        writer: &mut W,
        total_pages: u32,
        first_pages: &[u32],
        last_pages: &[u32],
    ) -> Result<()> {
        let mut page = vec![0u8; PAGE_SIZE as usize];

        // Header structure (simplified):
        // 0x00: 4 bytes - always 0
        // 0x04: 4 bytes - page size (4096)
        // 0x08: 4 bytes - number of tables
        // 0x0C: 4 bytes - next unused page
        // 0x10: 4 bytes - unknown
        // 0x14: 4 bytes - sequence number
        // 0x18: 8 bytes - padding
        // 0x20+: Table descriptors (24 bytes each)

        // Magic/padding
        page[0..4].copy_from_slice(&0u32.to_le_bytes());
        // Page size
        page[4..8].copy_from_slice(&PAGE_SIZE.to_le_bytes());
        // Number of tables
        page[8..12].copy_from_slice(&(first_pages.len() as u32).to_le_bytes());
        // Next unused page
        page[12..16].copy_from_slice(&total_pages.to_le_bytes());
        // Unknown (usually 0)
        page[16..20].copy_from_slice(&0u32.to_le_bytes());
        // Sequence number
        page[20..24].copy_from_slice(&1u32.to_le_bytes());

        // Table descriptors start at offset 0x20 (32)
        // Each descriptor is 16 bytes: type(4) + empty_candidate(4) + first_page(4) + last_page(4)
        let mut offset = 32usize;
        for i in 0..first_pages.len() {
            // Table type
            page[offset..offset + 4].copy_from_slice(&(i as u32).to_le_bytes());
            // Empty candidate (0 = no empty pages)
            page[offset + 4..offset + 8].copy_from_slice(&0u32.to_le_bytes());
            // First page
            page[offset + 8..offset + 12].copy_from_slice(&first_pages[i].to_le_bytes());
            // Last page
            page[offset + 12..offset + 16].copy_from_slice(&last_pages[i].to_le_bytes());
            offset += 16; // 16 bytes per descriptor, no padding
        }

        writer
            .write_all(&page)
            .map_err(|e| CrateError::Device(format!("Failed to write header: {}", e)))?;

        Ok(())
    }

    /// Write pages for a table with proper heap-based row format
    fn write_table_pages<W: Write + Seek>(
        &self,
        writer: &mut W,
        table_type: u8,
        data: &[u8],
        first_page: u32,
        last_page: u32,
    ) -> Result<()> {
        // Header is 40 bytes (0x00-0x27)
        // Row data starts at offset 0x28 (40)
        // Row index grows backward from page end
        const HEADER_SIZE: usize = 40;
        const ROW_INDEX_ENTRY_SIZE: usize = 2; // 2 bytes per row offset

        let num_pages = (last_page - first_page + 1) as usize;

        // For simplicity, we'll write all data to a single page if it fits
        // Otherwise, we split across pages
        let max_data_per_page = PAGE_SIZE as usize - HEADER_SIZE - 8; // Reserve 8 bytes for minimal row index

        for page_idx in 0..num_pages {
            let mut page = vec![0u8; PAGE_SIZE as usize];
            let page_num = first_page + page_idx as u32;

            // Calculate what data goes on this page
            let data_start = page_idx * max_data_per_page;
            let data_end = std::cmp::min(data_start + max_data_per_page, data.len());
            let chunk = if data_start < data.len() {
                &data[data_start..data_end]
            } else {
                &[]
            };
            let used_size = chunk.len() as u16;

            // Page header (40 bytes):
            // 0x00: 4 bytes - padding (zeros)
            page[0..4].copy_from_slice(&0u32.to_le_bytes());
            // 0x04: 4 bytes - page index
            page[4..8].copy_from_slice(&page_num.to_le_bytes());
            // 0x08: 4 bytes - page type (table type)
            page[8..12].copy_from_slice(&(table_type as u32).to_le_bytes());
            // 0x0C: 4 bytes - next page (0 if last)
            let next = if page_num < last_page {
                page_num + 1
            } else {
                0
            };
            page[12..16].copy_from_slice(&next.to_le_bytes());
            // 0x10: 4 bytes - version/sequence number
            page[16..20].copy_from_slice(&1u32.to_le_bytes());
            // 0x14: 4 bytes - unknown (zeros) - already zero

            // 0x18-0x1B: num_rows packed (13 bits num_row_offsets + 11 bits num_rows) + page_flags
            // For now, set num_rows = 1 if we have data, 0 otherwise
            let num_rows: u32 = if !chunk.is_empty() { 1 } else { 0 };
            // Pack: num_row_offsets (13 bits) << 11 | num_rows (11 bits)
            // Simplified: just set num_rows in the lower bits
            let row_counts = (num_rows << 11) | num_rows;
            page[24..27].copy_from_slice(&row_counts.to_le_bytes()[0..3]);
            // 0x1B: page_flags (0x64 for data pages)
            page[27] = 0x64;

            // 0x1C-0x1D: free_size
            let free_size = (PAGE_SIZE as u16) - (HEADER_SIZE as u16) - used_size - 4; // 4 for row index
            page[28..30].copy_from_slice(&free_size.to_le_bytes());
            // 0x1E-0x1F: used_size
            page[30..32].copy_from_slice(&used_size.to_le_bytes());

            // 0x20-0x27: Additional header bytes
            // These seem to be row-related offsets. Set to 0x1fff (empty marker) for now
            page[32..34].copy_from_slice(&0x1fffu16.to_le_bytes()); // _u5_
            page[34..36].copy_from_slice(&0x1fffu16.to_le_bytes()); // _unkrows_
            page[36..38].copy_from_slice(&0x0000u16.to_le_bytes()); // _u6_ (0 for data pages)
            page[38..40].copy_from_slice(&0x0000u16.to_le_bytes()); // _u7_

            // Copy row data starting at offset 0x28 (40)
            if !chunk.is_empty() {
                page[HEADER_SIZE..HEADER_SIZE + chunk.len()].copy_from_slice(chunk);
            }

            // Write row index at end of page (growing backward)
            // For each row, we need a 2-byte offset pointing to row start
            // Plus row presence bitmask (2 bytes) + 2 bytes padding per 16 rows
            if !chunk.is_empty() {
                // Row offset: points to offset 0x28 (40) where data starts
                let row_offset: u16 = HEADER_SIZE as u16;
                // Row presence bitmask: bit 0 set = row 0 present
                let row_presence: u16 = 0x0001;
                // Unknown bytes after bitmask
                let row_unknown: u16 = 0x0000;

                // Write at end of page (last 6 bytes before end for 1 row):
                // [row_presence: 2][unknown: 2][row_offset: 2]
                let index_start = PAGE_SIZE as usize - 6;
                page[index_start..index_start + 2].copy_from_slice(&row_presence.to_le_bytes());
                page[index_start + 2..index_start + 4].copy_from_slice(&row_unknown.to_le_bytes());
                page[index_start + 4..index_start + 6].copy_from_slice(&row_offset.to_le_bytes());
            }

            writer
                .write_all(&page)
                .map_err(|e| CrateError::Device(format!("Failed to write table page: {}", e)))?;
        }

        Ok(())
    }

    /// Build table data for a string lookup table (genres, artists, keys)
    fn build_string_table_data(&self, map: &HashMap<String, u32>) -> Vec<u8> {
        let mut data = Vec::new();

        // Sort by ID for consistent ordering
        let mut entries: Vec<_> = map.iter().collect();
        entries.sort_by_key(|(_, id)| *id);

        for (name, id) in entries {
            // Row structure for string tables:
            // 2 bytes: row flags (0x00 for normal)
            // 2 bytes: ID (little-endian)
            // 1 byte: name type (0x90 for inline string)
            // variable: name string with length prefix

            data.extend_from_slice(&[0x00, 0x00]); // Flags
            data.extend_from_slice(&(*id as u16).to_le_bytes()); // ID

            // Inline UTF-8 string
            self.write_device_sql_string(&mut data, name);
        }

        data
    }

    /// Build table data for albums (includes artist reference)
    fn build_album_table_data(&self) -> Vec<u8> {
        let mut data = Vec::new();

        let mut entries: Vec<_> = self.albums.iter().collect();
        entries.sort_by_key(|(_, id)| *id);

        for (name, id) in entries {
            data.extend_from_slice(&[0x00, 0x00]); // Flags
            data.extend_from_slice(&(*id as u16).to_le_bytes()); // ID
            data.extend_from_slice(&[0x00, 0x00]); // Artist ID (not linked for now)
            self.write_device_sql_string(&mut data, name);
        }

        data
    }

    /// Build table data for colors
    fn build_color_table_data(&self) -> Vec<u8> {
        let mut data = Vec::new();

        let mut entries: Vec<_> = self.colors.iter().collect();
        entries.sort_by_key(|(_, id)| *id);

        for (name, id) in entries {
            data.extend_from_slice(&[0x00, 0x00]); // Flags
            data.extend_from_slice(&(*id as u16).to_le_bytes()); // ID
            data.push(0x00); // Unknown byte
            self.write_device_sql_string(&mut data, name);
        }

        data
    }

    /// Build table data for tracks
    fn build_tracks_table_data(&self) -> Vec<u8> {
        let mut data = Vec::new();

        for track in &self.tracks {
            // Track row structure (simplified):
            // This is a complex structure with many fields
            // We'll write essential fields for CDJ compatibility

            let _row_start = data.len();

            // Row subtype (determines offset sizes)
            data.push(0x24); // Subtype for full track data

            // Various indices into the row for string fields
            // We'll calculate these after writing all the data

            // Fixed numeric fields (using small offsets)
            data.extend_from_slice(&[0x00, 0x00]); // Index shift
            data.extend_from_slice(&(0u32).to_le_bytes()); // Bitmask
            data.extend_from_slice(&track.duration_seconds.to_le_bytes()); // Duration
            data.extend_from_slice(&(0u32).to_le_bytes()); // Sample depth
            data.extend_from_slice(&(44100u32).to_le_bytes()); // Sample rate (assume 44.1kHz)
            data.extend_from_slice(&(0u32).to_le_bytes()); // Unknown
            data.extend_from_slice(&(0u16).to_le_bytes()); // Unknown
            data.extend_from_slice(&(0u16).to_le_bytes()); // Unknown
            data.extend_from_slice(&track.bitrate.to_le_bytes()); // Bitrate
            data.extend_from_slice(&track.id.to_le_bytes()); // Track ID
            data.extend_from_slice(&(track.artist_id as u16).to_le_bytes()); // Artist ID
            data.extend_from_slice(&(0u16).to_le_bytes()); // Original artist ID
            data.extend_from_slice(&(0u16).to_le_bytes()); // Remixer ID
            data.extend_from_slice(&(0u16).to_le_bytes()); // Composer ID
            data.extend_from_slice(&(track.album_id as u16).to_le_bytes()); // Album ID
            data.extend_from_slice(&(0u16).to_le_bytes()); // Label ID
            data.extend_from_slice(&(track.genre_id as u16).to_le_bytes()); // Genre ID
            data.extend_from_slice(&(track.key_id as u16).to_le_bytes()); // Key ID
            data.extend_from_slice(&(track.color_id as u16).to_le_bytes()); // Color ID
            data.extend_from_slice(&(0u16).to_le_bytes()); // Artwork ID
            data.extend_from_slice(&track.tempo.to_le_bytes()); // Tempo * 100
            data.extend_from_slice(&(0u32).to_le_bytes()); // Unknown
            data.push(track.rating * 51); // Rating (0-255 scale, 5 stars = 255)
            data.extend_from_slice(&[0x00; 7]); // Padding

            // String fields (we'll write them inline)
            // Title
            self.write_device_sql_string(&mut data, &track.title);
            // File path
            self.write_device_sql_string(&mut data, &track.file_path);
        }

        data
    }

    /// Build playlist tree data
    fn build_playlist_tree_data(&self) -> Vec<u8> {
        let mut data = Vec::new();

        for node in &self.playlist_nodes {
            // Playlist tree row structure:
            // 2 bytes: flags
            // 4 bytes: ID
            // 4 bytes: parent ID
            // 4 bytes: sort order
            // 1 byte: is folder flag
            // variable: name string

            data.extend_from_slice(&[0x00, 0x00]); // Flags
            data.extend_from_slice(&node.id.to_le_bytes());
            data.extend_from_slice(&node.parent_id.to_le_bytes());
            data.extend_from_slice(&node.sort_order.to_le_bytes());
            data.push(if node.is_folder { 1 } else { 0 });

            self.write_device_sql_string(&mut data, &node.name);
        }

        data
    }

    /// Build playlist entries data
    fn build_playlist_entries_data(&self) -> Vec<u8> {
        let mut data = Vec::new();

        for entry in &self.playlist_entries {
            // Playlist entry row structure:
            // 2 bytes: flags
            // 4 bytes: entry index
            // 4 bytes: track ID
            // 4 bytes: playlist ID

            data.extend_from_slice(&[0x00, 0x00]); // Flags
            data.extend_from_slice(&entry.entry_index.to_le_bytes());
            data.extend_from_slice(&entry.track_id.to_le_bytes());
            data.extend_from_slice(&entry.playlist_id.to_le_bytes());
        }

        data
    }

    /// Write a DeviceSQL string (used in PDB format)
    fn write_device_sql_string(&self, data: &mut Vec<u8>, s: &str) {
        // DeviceSQL string format:
        // 1 byte: encoding flags
        //   0x40 = ASCII, 0x90 = UTF-8 with short length, 0xC0 = UTF-8 with long length
        // 1-2 bytes: length
        // variable: string data

        let bytes = s.as_bytes();
        let len = bytes.len();

        if s.is_ascii() && len < 64 {
            // Short ASCII string
            data.push(0x40 | (len as u8));
            data.extend_from_slice(bytes);
        } else if len < 64 {
            // Short UTF-8 string
            data.push(0x90);
            data.push(len as u8);
            data.extend_from_slice(bytes);
        } else {
            // Long UTF-8 string
            data.push(0xC0);
            data.extend_from_slice(&(len as u16).to_le_bytes());
            data.extend_from_slice(bytes);
        }
    }
}

impl Default for RekordboxPdbWriter {
    fn default() -> Self {
        Self::new()
    }
}
