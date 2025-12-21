//! Rekordbox PDB file writer
//!
//! Generates export.pdb files compatible with Pioneer CDJ/XDJ equipment.
//! Based on the Deep Symmetry analysis: https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html
//! and the rekordcrate library: https://github.com/Holzhaus/rekordcrate

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Cursor, Seek, SeekFrom, Write};
use std::path::Path;

use crate::error::{CrateError, Result};
use crate::models::{Playlist, Track};

/// Page size for PDB files (standard is 4096 bytes)
const PAGE_SIZE: u32 = 4096;

/// Size of the file header (before table descriptors)
const FILE_HEADER_SIZE: usize = 0x1C; // 28 bytes

/// Size of a table descriptor
const TABLE_DESCRIPTOR_SIZE: usize = 16;

/// Size of a page header
const PAGE_HEADER_SIZE: usize = 0x20; // 32 bytes

/// Size of a data page header (additional bytes after page header)
const DATA_PAGE_HEADER_SIZE: usize = 0x08; // 8 bytes

/// Offset where row data begins in a data page
const ROW_DATA_OFFSET: usize = PAGE_HEADER_SIZE + DATA_PAGE_HEADER_SIZE; // 0x28 = 40

/// Size of a row group (16 offsets + presence flags + unknown)
const ROW_GROUP_SIZE: usize = 36;

/// Maximum rows per row group
const MAX_ROWS_PER_GROUP: usize = 16;

/// Page flags for index pages
const PAGE_FLAGS_INDEX: u8 = 0x64;

/// Page flags for data pages
const PAGE_FLAGS_DATA: u8 = 0x34;

/// Empty index entry marker
const EMPTY_INDEX_ENTRY: u32 = 0x1FFF_FFF8;

/// Table types in the PDB format
/// All 20 types (0-19) must be present in numeric order
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
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
    Unknown9 = 9,
    Unknown10 = 10,
    HistoryPlaylists = 11,
    HistoryEntries = 12,
    Artwork = 13,
    Unknown14 = 14,
    Unknown15 = 15,
    Columns = 16,
    Menu = 17,
    Unknown18 = 18,
    History = 19,
}

impl TableType {
    fn all_required() -> &'static [TableType] {
        &[
            TableType::Tracks,          // 0
            TableType::Genres,          // 1
            TableType::Artists,         // 2
            TableType::Albums,          // 3
            TableType::Labels,          // 4
            TableType::Keys,            // 5
            TableType::Colors,          // 6
            TableType::PlaylistTree,    // 7
            TableType::PlaylistEntries, // 8
            TableType::Unknown9,        // 9
            TableType::Unknown10,       // 10
            TableType::HistoryPlaylists, // 11
            TableType::HistoryEntries,  // 12
            TableType::Artwork,         // 13
            TableType::Unknown14,       // 14
            TableType::Unknown15,       // 15
            TableType::Columns,         // 16
            TableType::Menu,            // 17
            TableType::Unknown18,       // 18
            TableType::History,         // 19
        ]
    }
}

/// DeviceSQL string encoding
/// Supports short ASCII, long ASCII, and long UTF-16LE
#[derive(Debug, Clone)]
enum DeviceSQLString {
    /// Short ASCII string (length <= 126)
    ShortAscii(Vec<u8>),
    /// Long ASCII string (length > 126)
    LongAscii(Vec<u8>),
    /// UTF-16LE string (for non-ASCII content)
    LongUtf16(Vec<u16>),
}

impl DeviceSQLString {
    /// Maximum length for short ASCII strings
    const MAX_SHORT_LEN: usize = 126;

    /// Create a new DeviceSQLString from a Rust string
    fn new(s: &str) -> Self {
        let bytes = s.as_bytes();
        if s.is_ascii() && bytes.len() <= Self::MAX_SHORT_LEN {
            Self::ShortAscii(bytes.to_vec())
        } else if s.is_ascii() {
            Self::LongAscii(bytes.to_vec())
        } else {
            Self::LongUtf16(s.encode_utf16().collect())
        }
    }

    /// Create an empty string
    fn empty() -> Self {
        Self::ShortAscii(Vec::new())
    }

    /// Get the serialized size of this string
    fn binary_size(&self) -> usize {
        match self {
            Self::ShortAscii(bytes) => 1 + bytes.len(),
            Self::LongAscii(bytes) => 4 + bytes.len(),
            Self::LongUtf16(chars) => 4 + chars.len() * 2,
        }
    }

    /// Write the string to a writer
    fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        match self {
            Self::ShortAscii(bytes) => {
                // Header byte: ((len + 1) << 1) | 1
                let header = (((bytes.len() + 1) << 1) | 1) as u8;
                writer.write_all(&[header])?;
                writer.write_all(bytes)?;
            }
            Self::LongAscii(bytes) => {
                // Flags: 0x40 for ASCII
                writer.write_all(&[0x40])?;
                // Length: content length + 4
                let len = (bytes.len() + 4) as u16;
                writer.write_all(&len.to_le_bytes())?;
                // Padding byte
                writer.write_all(&[0x00])?;
                // Content
                writer.write_all(bytes)?;
            }
            Self::LongUtf16(chars) => {
                // Flags: 0x90 for UTF-16LE
                writer.write_all(&[0x90])?;
                // Length: (char count * 2) + 4
                let len = (chars.len() * 2 + 4) as u16;
                writer.write_all(&len.to_le_bytes())?;
                // Padding byte
                writer.write_all(&[0x00])?;
                // Content as little-endian UTF-16
                for c in chars {
                    writer.write_all(&c.to_le_bytes())?;
                }
            }
        }
        Ok(())
    }
}

/// A row group contains up to 16 row offsets and presence flags
#[derive(Debug, Clone)]
struct RowGroup {
    /// Offsets to rows within the page heap (relative to heap start)
    row_offsets: [u16; MAX_ROWS_PER_GROUP],
    /// Bitmask indicating which rows are present
    row_presence_flags: u16,
    /// Number of rows in this group
    num_rows: usize,
}

impl RowGroup {
    fn new() -> Self {
        Self {
            row_offsets: [0; MAX_ROWS_PER_GROUP],
            row_presence_flags: 0,
            num_rows: 0,
        }
    }

    fn add_row(&mut self, offset: u16) -> bool {
        if self.num_rows >= MAX_ROWS_PER_GROUP {
            return false;
        }
        // Offsets are stored from the end of the array
        let idx = MAX_ROWS_PER_GROUP - 1 - self.num_rows;
        self.row_offsets[idx] = offset;
        // Set presence bit (bit 0 = row 0, bit 1 = row 1, etc.)
        self.row_presence_flags |= 1 << self.num_rows;
        self.num_rows += 1;
        true
    }

    fn is_full(&self) -> bool {
        self.num_rows >= MAX_ROWS_PER_GROUP
    }

    /// Write the row group backwards from the current position
    #[allow(dead_code)]
    fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        // Write only the offsets that are present (from the first present row)
        let skip = self.row_presence_flags.leading_zeros() as usize;
        for offset in self.row_offsets.iter().skip(skip) {
            writer.write_all(&offset.to_le_bytes())?;
        }
        // Write presence flags
        writer.write_all(&self.row_presence_flags.to_le_bytes())?;
        // Write unknown field (usually 0)
        writer.write_all(&0u16.to_le_bytes())?;
        Ok(())
    }
}

/// Builds a page with proper layout
struct PageBuilder {
    /// Page type (table type)
    page_type: TableType,
    /// Page index
    page_index: u32,
    /// Next page index (0 if last)
    next_page: u32,
    /// Row data buffer
    row_data: Vec<u8>,
    /// Row groups
    row_groups: Vec<RowGroup>,
    /// Current row group being filled
    current_group: RowGroup,
    /// Total row count
    total_rows: u16,
    /// Total row offsets ever allocated
    total_offsets: u16,
}

impl PageBuilder {
    fn new(page_type: TableType, page_index: u32) -> Self {
        Self {
            page_type,
            page_index,
            next_page: 0,
            row_data: Vec::new(),
            row_groups: Vec::new(),
            current_group: RowGroup::new(),
            total_rows: 0,
            total_offsets: 0,
        }
    }

    #[allow(dead_code)]
    fn set_next_page(&mut self, next: u32) {
        self.next_page = next;
    }

    /// Calculate available space for row data
    fn available_space(&self) -> usize {
        let header_space = ROW_DATA_OFFSET;
        let row_groups_count = self.row_groups.len() + 1; // Include current group
        let row_group_space = row_groups_count * ROW_GROUP_SIZE;
        let used = self.row_data.len();

        PAGE_SIZE as usize - header_space - row_group_space - used
    }

    /// Add a row to the page, returns false if page is full
    fn add_row(&mut self, row_data: &[u8]) -> bool {
        // Check if we have space
        let needed = row_data.len() + 2; // +2 for potential new offset in current group
        if needed > self.available_space() {
            return false;
        }

        // Record the offset where this row starts (relative to heap start at 0x28)
        let offset = self.row_data.len() as u16;

        // If current group is full, start a new one
        if self.current_group.is_full() {
            self.row_groups.push(self.current_group.clone());
            self.current_group = RowGroup::new();
        }

        // Add the row offset to current group
        self.current_group.add_row(offset);
        self.total_rows += 1;
        self.total_offsets += 1;

        // Append row data
        self.row_data.extend_from_slice(row_data);

        true
    }

    /// Build the complete page as a byte vector
    fn build(mut self) -> Vec<u8> {
        let mut page = vec![0u8; PAGE_SIZE as usize];

        // Finalize the current group if it has rows
        if self.current_group.num_rows > 0 {
            self.row_groups.push(self.current_group);
        }

        // === Write Page Header (32 bytes at 0x00-0x1F) ===
        // 0x00-0x03: Padding (zeros)
        // 0x04-0x07: Page index
        page[0x04..0x08].copy_from_slice(&self.page_index.to_le_bytes());
        // 0x08-0x0B: Page type
        page[0x08..0x0C].copy_from_slice(&(self.page_type as u32).to_le_bytes());
        // 0x0C-0x0F: Next page
        page[0x0C..0x10].copy_from_slice(&self.next_page.to_le_bytes());
        // 0x10-0x13: Unknown1 (version/sequence, use 1)
        page[0x10..0x14].copy_from_slice(&1u32.to_le_bytes());
        // 0x14-0x17: Unknown2 (zeros)
        // 0x18-0x1A: Packed row counts (3 bytes)
        //   Bits 0-12: num_row_offsets (13 bits)
        //   Bits 13-23: num_rows (11 bits)
        let packed = ((self.total_offsets as u32) & 0x1FFF)
            | (((self.total_rows as u32) & 0x7FF) << 13);
        page[0x18..0x1B].copy_from_slice(&packed.to_le_bytes()[0..3]);
        // 0x1B: Page flags (0x34 for data pages)
        page[0x1B] = PAGE_FLAGS_DATA;
        // 0x1C-0x1D: Free size
        let used_size = self.row_data.len() as u16;
        let row_groups_size = (self.row_groups.len() * ROW_GROUP_SIZE) as u16;
        let heap_size = (PAGE_SIZE as u16) - (ROW_DATA_OFFSET as u16);
        let free_size = heap_size - used_size - row_groups_size;
        page[0x1C..0x1E].copy_from_slice(&free_size.to_le_bytes());
        // 0x1E-0x1F: Used size
        page[0x1E..0x20].copy_from_slice(&used_size.to_le_bytes());

        // === Write Data Page Header (8 bytes at 0x20-0x27) ===
        // Usually all zeros or small values for data pages
        // 0x20-0x21: unknown5 (often 1 or 0x1fff for empty)
        if self.total_rows == 0 {
            page[0x20..0x22].copy_from_slice(&0x1fffu16.to_le_bytes());
            page[0x22..0x24].copy_from_slice(&0x1fffu16.to_le_bytes());
        } else {
            page[0x20..0x22].copy_from_slice(&1u16.to_le_bytes());
        }
        // 0x22-0x27: Other unknown fields (zeros)

        // === Write Row Data (starting at 0x28) ===
        page[ROW_DATA_OFFSET..ROW_DATA_OFFSET + self.row_data.len()]
            .copy_from_slice(&self.row_data);

        // === Write Row Groups (backwards from page end) ===
        let mut cursor = Cursor::new(&mut page[..]);
        let mut group_offset = PAGE_SIZE as usize;

        for group in self.row_groups.iter().rev() {
            // Each group is written from its end position backwards
            group_offset -= ROW_GROUP_SIZE;
            cursor.seek(SeekFrom::Start(group_offset as u64)).unwrap();

            // Write the group
            let skip = group.row_presence_flags.leading_zeros() as usize;
            // First, skip the unused offset slots
            let skip_bytes = skip * 2;
            cursor
                .seek(SeekFrom::Current(skip_bytes as i64))
                .unwrap();

            // Write present offsets
            for offset in group.row_offsets.iter().skip(skip) {
                cursor.write_all(&offset.to_le_bytes()).unwrap();
            }

            // Write presence flags and unknown
            cursor
                .write_all(&group.row_presence_flags.to_le_bytes())
                .unwrap();
            cursor.write_all(&0u16.to_le_bytes()).unwrap();
        }

        page
    }
}

/// Builds an index page for a table
/// Index pages point to data pages and are required by Pioneer equipment
fn build_index_page(
    page_type: TableType,
    page_index: u32,
    next_page: u32,
    data_page_indices: &[u32],
) -> Vec<u8> {
    let mut page = vec![0u8; PAGE_SIZE as usize];

    // === Page Header (32 bytes at 0x00-0x1F) ===
    // 0x00-0x03: Padding (zeros)
    // 0x04-0x07: Page index
    page[0x04..0x08].copy_from_slice(&page_index.to_le_bytes());
    // 0x08-0x0B: Page type
    page[0x08..0x0C].copy_from_slice(&(page_type as u32).to_le_bytes());
    // 0x0C-0x0F: Next page
    page[0x0C..0x10].copy_from_slice(&next_page.to_le_bytes());
    // 0x10-0x13: Unknown1 (use 1)
    page[0x10..0x14].copy_from_slice(&1u32.to_le_bytes());
    // 0x14-0x17: Unknown2 (zeros)
    // 0x18-0x1A: Packed row counts (zeros for index pages)
    // 0x1B: Page flags (0x64 for index pages)
    page[0x1B] = PAGE_FLAGS_INDEX;
    // 0x1C-0x1D: Free size (0 for index pages)
    // 0x1E-0x1F: Used size (0 for index pages)

    // === Index Page Header (starts at 0x20) ===
    // 0x20-0x21: unknown_a (0x1fff)
    page[0x20..0x22].copy_from_slice(&0x1fffu16.to_le_bytes());
    // 0x22-0x23: unknown_b (0x1fff)
    page[0x22..0x24].copy_from_slice(&0x1fffu16.to_le_bytes());
    // 0x24-0x25: magic (0x03ec)
    page[0x24..0x26].copy_from_slice(&0x03ecu16.to_le_bytes());
    // 0x26-0x27: next_offset (number of entries)
    page[0x26..0x28].copy_from_slice(&(data_page_indices.len() as u16).to_le_bytes());
    // 0x28-0x2B: page_index (redundant)
    page[0x28..0x2C].copy_from_slice(&page_index.to_le_bytes());
    // 0x2C-0x2F: next_page (redundant)
    page[0x2C..0x30].copy_from_slice(&next_page.to_le_bytes());
    // 0x30-0x37: magic (0x0000000003ffffff)
    page[0x30..0x38].copy_from_slice(&0x0000_0000_03ff_ffffu64.to_le_bytes());
    // 0x38-0x39: num_entries
    page[0x38..0x3A].copy_from_slice(&(data_page_indices.len() as u16).to_le_bytes());
    // 0x3A-0x3B: first_empty (0x1fff if none)
    page[0x3A..0x3C].copy_from_slice(&0x1fffu16.to_le_bytes());

    // === Index Entries (starting at 0x3C) ===
    // Each entry is 4 bytes: (page_index << 3) | flags
    let mut offset = 0x3C;
    for &data_page_idx in data_page_indices {
        let entry = (data_page_idx << 3) | 0; // flags = 0
        page[offset..offset + 4].copy_from_slice(&entry.to_le_bytes());
        offset += 4;
    }

    // Fill remaining entries with empty marker (0x1ffffff8)
    let remaining_space = PAGE_SIZE as usize - offset - 20; // Leave 20 bytes at end
    let empty_entries = remaining_space / 4;
    for _ in 0..empty_entries {
        page[offset..offset + 4].copy_from_slice(&EMPTY_INDEX_ENTRY.to_le_bytes());
        offset += 4;
    }

    // Last 20 bytes are zeros (already initialized)

    page
}

/// Builds an empty index page for tables with no data
/// In rekordbox format, empty tables have a single page with no index entries
fn build_empty_index_page(page_type: TableType, page_index: u32, next_page: u32) -> Vec<u8> {
    let mut page = vec![0u8; PAGE_SIZE as usize];

    // === Page Header (32 bytes at 0x00-0x1F) ===
    // 0x04-0x07: Page index
    page[0x04..0x08].copy_from_slice(&page_index.to_le_bytes());
    // 0x08-0x0B: Page type
    page[0x08..0x0C].copy_from_slice(&(page_type as u32).to_le_bytes());
    // 0x0C-0x0F: Next page (points to empty candidate)
    page[0x0C..0x10].copy_from_slice(&next_page.to_le_bytes());
    // 0x10-0x13: Unknown1 (use 1)
    page[0x10..0x14].copy_from_slice(&1u32.to_le_bytes());
    // 0x1B: Page flags (0x64 for index pages)
    page[0x1B] = PAGE_FLAGS_INDEX;

    // === Index Page Header (starts at 0x20) ===
    // 0x20-0x21: unknown_a (0x1fff)
    page[0x20..0x22].copy_from_slice(&0x1fffu16.to_le_bytes());
    // 0x22-0x23: unknown_b (0x1fff)
    page[0x22..0x24].copy_from_slice(&0x1fffu16.to_le_bytes());
    // 0x24-0x25: magic (0x03ec)
    page[0x24..0x26].copy_from_slice(&0x03ecu16.to_le_bytes());
    // 0x26-0x27: next_offset = 0 (no entries for empty table)
    page[0x26..0x28].copy_from_slice(&0u16.to_le_bytes());
    // 0x28-0x2B: page_index (redundant)
    page[0x28..0x2C].copy_from_slice(&page_index.to_le_bytes());
    // 0x2C-0x2F: next_page = 0x03ffffff (no data pages)
    page[0x2C..0x30].copy_from_slice(&0x03ff_ffffu32.to_le_bytes());
    // 0x30-0x37: magic2 (0x03ffffff then 0x00000000)
    page[0x30..0x38].copy_from_slice(&0x0000_0000_03ff_ffffu64.to_le_bytes());
    // 0x38-0x39: num_entries = 0
    page[0x38..0x3A].copy_from_slice(&0u16.to_le_bytes());
    // 0x3A-0x3B: first_empty (0x1fff)
    page[0x3A..0x3C].copy_from_slice(&0x1fffu16.to_le_bytes());

    // Fill all index entries with empty marker (0x1ffffff8)
    let mut offset = 0x3C;
    let remaining_space = PAGE_SIZE as usize - offset - 20;
    let empty_entries = remaining_space / 4;
    for _ in 0..empty_entries {
        page[offset..offset + 4].copy_from_slice(&EMPTY_INDEX_ENTRY.to_le_bytes());
        offset += 4;
    }

    page
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
    color_id: u8,
    duration_seconds: u16,
    tempo: u32, // BPM * 100
    bitrate: u32,
    sample_rate: u32,
    file_size: u32,
    file_path: String,
    filename: String,
    rating: u8,
    year: u16,
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
    colors: HashMap<String, u8>,

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
    next_color_id: u8,
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
    /// Note: Full merging is not yet implemented - this starts fresh
    pub fn from_existing(_data: &[u8]) -> Result<Self> {
        // For now, start fresh - merging with existing PDB requires parsing
        // the existing file to extract max IDs and continue from there
        log::warn!("Merging with existing PDB not yet implemented, starting fresh");
        Ok(Self::new())
    }

    /// Add a track and return its PDB ID
    pub fn add_track(&mut self, track: &Track, usb_path: &str) -> u32 {
        let id = self.next_track_id;
        self.next_track_id += 1;

        // Get or create artist
        let artist_name = track
            .artist
            .clone()
            .unwrap_or_else(|| "Unknown Artist".to_string());
        let artist_id = *self.artists.entry(artist_name).or_insert_with(|| {
            let id = self.next_artist_id;
            self.next_artist_id += 1;
            id
        });

        // Get or create album
        let album_name = track
            .album
            .clone()
            .unwrap_or_else(|| "Unknown Album".to_string());
        let album_id = *self.albums.entry(album_name).or_insert_with(|| {
            let id = self.next_album_id;
            self.next_album_id += 1;
            id
        });

        // Get or create genre
        let genre_name = track
            .genre
            .clone()
            .unwrap_or_else(|| "Unknown".to_string());
        let genre_id = *self.genres.entry(genre_name).or_insert_with(|| {
            let id = self.next_genre_id;
            self.next_genre_id += 1;
            id
        });

        // Get or create key
        let key_name = track.key.clone().unwrap_or_default();
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
        let color_name = track.color.clone().unwrap_or_default();
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

        // Extract filename from path
        let filename = std::path::Path::new(&track.file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let pdb_track = PdbTrack {
            id,
            title: track.title.clone().unwrap_or_else(|| "Untitled".to_string()),
            artist_id,
            album_id,
            genre_id,
            key_id,
            color_id,
            duration_seconds: (track.duration_ms / 1000) as u16,
            tempo,
            bitrate: track.bitrate.unwrap_or(0) as u32,
            sample_rate: track.sample_rate.unwrap_or(44100) as u32,
            file_size: 0, // We don't have file size readily available
            file_path,
            filename,
            rating: track.rating as u8,
            year: 0, // Not currently tracked
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

        // Build all data pages for each table
        let mut all_data_pages: Vec<(TableType, Vec<Vec<u8>>)> = Vec::new();

        // Build pages for each table type
        for &table_type in TableType::all_required() {
            let pages = self.build_table_pages(table_type)?;
            all_data_pages.push((table_type, pages));
        }

        // Calculate page assignments
        // Tables with data: 1 index page + N data pages
        // Empty tables: 1 index page only (first == last, matching rekordbox format)
        let mut current_page: u32 = 1; // Page 0 is the header
        let mut table_info: Vec<(TableType, u32, u32, bool)> = Vec::new(); // (type, first_page, last_page, is_empty)

        for (table_type, data_pages) in &all_data_pages {
            let is_empty = data_pages.is_empty();

            if is_empty {
                // Empty table: single page (first == last)
                let single_page = current_page;
                log::debug!(
                    "Table {:?}: EMPTY, single_page={}",
                    table_type,
                    single_page
                );
                table_info.push((*table_type, single_page, single_page, true));
                current_page += 1;
            } else {
                // Table with data: index page + data pages
                let index_page = current_page;
                let num_data_pages = data_pages.len() as u32;
                let first_data_page = index_page + 1;
                let last_data_page = first_data_page + num_data_pages - 1;

                log::debug!(
                    "Table {:?}: data_pages.len()={}, index_page={}, first_data={}, last_data={}",
                    table_type,
                    data_pages.len(),
                    index_page,
                    first_data_page,
                    last_data_page
                );

                // first_page = index page, last_page = last data page
                table_info.push((*table_type, index_page, last_data_page, false));
                current_page = last_data_page + 1;
            }
        }

        let total_pages = current_page;
        log::debug!("Total pages: {}", total_pages);

        // Write file header (page 0)
        self.write_file_header(&mut writer, total_pages, &table_info)?;

        // Write table pages
        for (i, (table_type, data_pages)) in all_data_pages.iter().enumerate() {
            let (_, first_page, last_page, is_empty) = table_info[i];

            if is_empty {
                // Empty table: write single empty index page
                // Use null page marker (0x03ffffff) for next_page
                let empty_page = build_empty_index_page(*table_type, first_page, 0x03ff_ffff);
                writer
                    .write_all(&empty_page)
                    .map_err(|e| CrateError::Device(format!("Failed to write empty page: {}", e)))?;
            } else {
                // Table with data: write index page + data pages
                let index_page = first_page;
                let first_data_page = index_page + 1;
                let num_data_pages = data_pages.len() as u32;

                // Collect data page indices for the index page
                let data_page_indices: Vec<u32> = (0..num_data_pages)
                    .map(|j| first_data_page + j)
                    .collect();

                // Build and write the index page
                let idx_page = build_index_page(
                    *table_type,
                    index_page,
                    first_data_page,
                    &data_page_indices,
                );
                writer
                    .write_all(&idx_page)
                    .map_err(|e| CrateError::Device(format!("Failed to write index page: {}", e)))?;

                // Write data pages
                for (j, page_data) in data_pages.iter().enumerate() {
                    let page_num = first_data_page + j as u32;
                    let next_page = if page_num < last_page {
                        page_num + 1
                    } else {
                        // Last data page uses null page marker
                        0x03ff_ffff
                    };

                    // Update the next_page and page_index fields in the page
                    let mut page = page_data.clone();
                    page[0x0C..0x10].copy_from_slice(&next_page.to_le_bytes());
                    page[0x04..0x08].copy_from_slice(&page_num.to_le_bytes());

                    writer
                        .write_all(&page)
                        .map_err(|e| CrateError::Device(format!("Failed to write page: {}", e)))?;
                }
            }
        }

        writer
            .flush()
            .map_err(|e| CrateError::Device(format!("Failed to flush PDB file: {}", e)))?;

        Ok(())
    }

    /// Write the file header (first page)
    fn write_file_header<W: Write>(
        &self,
        writer: &mut W,
        total_pages: u32,
        table_info: &[(TableType, u32, u32, bool)],
    ) -> Result<()> {
        let mut page = vec![0u8; PAGE_SIZE as usize];

        // File header structure (28 bytes):
        // 0x00-0x03: Magic (always 0)
        // 0x04-0x07: Page size
        page[0x04..0x08].copy_from_slice(&PAGE_SIZE.to_le_bytes());
        // 0x08-0x0B: Number of tables
        page[0x08..0x0C].copy_from_slice(&(table_info.len() as u32).to_le_bytes());
        // 0x0C-0x0F: Next unused page
        page[0x0C..0x10].copy_from_slice(&total_pages.to_le_bytes());
        // 0x10-0x13: Unknown (usually small number or 0)
        page[0x10..0x14].copy_from_slice(&0u32.to_le_bytes());
        // 0x14-0x17: Sequence number
        page[0x14..0x18].copy_from_slice(&1u32.to_le_bytes());
        // 0x18-0x1B: Gap (always 0)

        // Table descriptors start at 0x1C
        let mut offset = FILE_HEADER_SIZE;
        for (table_type, first_page, last_page, _is_empty) in table_info.iter() {
            // Extract values explicitly to avoid any reference issues
            let tt = *table_type as u32;
            let fp = *first_page;
            let lp = *last_page;

            log::debug!(
                "Writing table descriptor at offset 0x{:X}: type={}, empty=0, first={}, last={}",
                offset,
                tt,
                fp,
                lp
            );

            // Table type
            page[offset..offset + 4].copy_from_slice(&tt.to_le_bytes());
            // Empty candidate (0 for now - rekordbox uses non-zero but doesn't seem required)
            page[offset + 4..offset + 8].copy_from_slice(&0u32.to_le_bytes());
            // First page
            page[offset + 8..offset + 12].copy_from_slice(&fp.to_le_bytes());
            // Last page (equals first_page for empty tables)
            page[offset + 12..offset + 16].copy_from_slice(&lp.to_le_bytes());
            offset += TABLE_DESCRIPTOR_SIZE;
        }

        writer
            .write_all(&page)
            .map_err(|e| CrateError::Device(format!("Failed to write header: {}", e)))?;

        Ok(())
    }

    /// Build pages for a specific table type
    fn build_table_pages(&self, table_type: TableType) -> Result<Vec<Vec<u8>>> {
        match table_type {
            TableType::Tracks => self.build_track_pages(),
            TableType::Genres => self.build_genre_pages(),
            TableType::Artists => self.build_artist_pages(),
            TableType::Albums => self.build_album_pages(),
            TableType::Labels => Ok(Vec::new()),
            TableType::Keys => self.build_key_pages(),
            TableType::Colors => self.build_color_pages(),
            TableType::PlaylistTree => self.build_playlist_tree_pages(),
            TableType::PlaylistEntries => self.build_playlist_entries_pages(),
            TableType::Unknown9 => Ok(Vec::new()),
            TableType::Unknown10 => Ok(Vec::new()),
            TableType::HistoryPlaylists => Ok(Vec::new()),
            TableType::HistoryEntries => Ok(Vec::new()),
            TableType::Artwork => Ok(Vec::new()),
            TableType::Unknown14 => Ok(Vec::new()),
            TableType::Unknown15 => Ok(Vec::new()),
            TableType::Columns => self.build_column_pages(),
            TableType::Menu => Ok(Vec::new()),
            TableType::Unknown18 => Ok(Vec::new()),
            TableType::History => Ok(Vec::new()),
        }
    }

    /// Build track pages
    fn build_track_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Tracks, 0);

        for (row_index, track) in self.tracks.iter().enumerate() {
            let row_data = self.build_track_row(track, row_index as u16)?;

            if !builder.add_row(&row_data) {
                // Current page is full, finalize it and start a new one
                pages.push(builder.build());
                builder = PageBuilder::new(TableType::Tracks, 0);
                builder.add_row(&row_data);
            }
        }

        // Finalize the last page
        if !self.tracks.is_empty() {
            pages.push(builder.build());
        }

        Ok(pages)
    }

    /// Build a single track row
    fn build_track_row(&self, track: &PdbTrack, row_index: u16) -> Result<Vec<u8>> {
        let mut row = Vec::new();

        // Fixed header (92 bytes = 0x5C)
        // 0x00-0x01: Subtype (0x24 = 36, with bit 0x04 set means U16 offsets)
        row.extend_from_slice(&0x0024u16.to_le_bytes());
        // 0x02-0x03: Index shift (0x20 * row_index)
        row.extend_from_slice(&((row_index * 0x20) as u16).to_le_bytes());
        // 0x04-0x07: Bitmask (always 0x000c0700)
        row.extend_from_slice(&0x000c0700u32.to_le_bytes());
        // 0x08-0x0B: Sample rate
        row.extend_from_slice(&track.sample_rate.to_le_bytes());
        // 0x0C-0x0F: Composer ID (0 = none)
        row.extend_from_slice(&0u32.to_le_bytes());
        // 0x10-0x13: File size
        row.extend_from_slice(&track.file_size.to_le_bytes());
        // 0x14-0x17: Unknown2
        row.extend_from_slice(&0u32.to_le_bytes());
        // 0x18-0x19: Unknown3
        row.extend_from_slice(&0u16.to_le_bytes());
        // 0x1A-0x1B: Unknown4
        row.extend_from_slice(&0u16.to_le_bytes());
        // 0x1C-0x1F: Artwork ID (0 = none)
        row.extend_from_slice(&0u32.to_le_bytes());
        // 0x20-0x23: Key ID
        row.extend_from_slice(&track.key_id.to_le_bytes());
        // 0x24-0x27: Original artist ID (0 = none)
        row.extend_from_slice(&0u32.to_le_bytes());
        // 0x28-0x2B: Label ID (0 = none)
        row.extend_from_slice(&0u32.to_le_bytes());
        // 0x2C-0x2F: Remixer ID (0 = none)
        row.extend_from_slice(&0u32.to_le_bytes());
        // 0x30-0x33: Bitrate
        row.extend_from_slice(&track.bitrate.to_le_bytes());
        // 0x34-0x37: Track number (0)
        row.extend_from_slice(&0u32.to_le_bytes());
        // 0x38-0x3B: Tempo (BPM * 100)
        row.extend_from_slice(&track.tempo.to_le_bytes());
        // 0x3C-0x3F: Genre ID
        row.extend_from_slice(&track.genre_id.to_le_bytes());
        // 0x40-0x43: Album ID
        row.extend_from_slice(&track.album_id.to_le_bytes());
        // 0x44-0x47: Artist ID
        row.extend_from_slice(&track.artist_id.to_le_bytes());
        // 0x48-0x4B: Track ID
        row.extend_from_slice(&track.id.to_le_bytes());
        // 0x4C-0x4D: Disc number
        row.extend_from_slice(&0u16.to_le_bytes());
        // 0x4E-0x4F: Play count
        row.extend_from_slice(&0u16.to_le_bytes());
        // 0x50-0x51: Year
        row.extend_from_slice(&track.year.to_le_bytes());
        // 0x52-0x53: Sample depth (16 bit typical)
        row.extend_from_slice(&16u16.to_le_bytes());
        // 0x54-0x55: Duration (seconds)
        row.extend_from_slice(&track.duration_seconds.to_le_bytes());
        // 0x56-0x57: Unknown5 (always 0x29)
        row.extend_from_slice(&0x0029u16.to_le_bytes());
        // 0x58: Color ID
        row.push(track.color_id);
        // 0x59: Rating (0-5)
        row.push(track.rating);
        // 0x5A-0x5B: File type (0 = MP3, 1 = M4A, 5 = FLAC, etc.)
        row.extend_from_slice(&0u16.to_le_bytes()); // Assume MP3 for now

        // Now we need to write 22 U16 string offsets followed by the strings
        // Since subtype is 0x24 (bit 0x04 IS set), we use U16 offsets

        // Build the strings we need
        let strings = [
            DeviceSQLString::empty(),                    // 0: unused/padding
            DeviceSQLString::empty(),                    // 1: ISRC
            DeviceSQLString::empty(),                    // 2: Lyricist
            DeviceSQLString::empty(),                    // 3: Unknown
            DeviceSQLString::empty(),                    // 4: Unknown
            DeviceSQLString::empty(),                    // 5: Unknown
            DeviceSQLString::empty(),                    // 6: Message
            DeviceSQLString::empty(),                    // 7: Publish info
            DeviceSQLString::empty(),                    // 8: Autoload hotcues
            DeviceSQLString::empty(),                    // 9: Unknown
            DeviceSQLString::empty(),                    // 10: Unknown
            DeviceSQLString::empty(),                    // 11: Date added
            DeviceSQLString::empty(),                    // 12: Release date
            DeviceSQLString::empty(),                    // 13: Mix name
            DeviceSQLString::empty(),                    // 14: Unknown
            DeviceSQLString::empty(),                    // 15: Analyze path
            DeviceSQLString::empty(),                    // 16: Analyze date
            DeviceSQLString::empty(),                    // 17: Comment
            DeviceSQLString::new(&track.title),          // 18: Title
            DeviceSQLString::empty(),                    // 19: Unknown
            DeviceSQLString::new(&track.filename),       // 20: Filename
            DeviceSQLString::new(&track.file_path),      // 21: File path
        ];

        // Calculate offsets - these are ABSOLUTE positions within the row
        // Fixed header is 0x5C (92) bytes, followed by 22 U16 offset values (44 bytes)
        // String data starts at position 92 + 44 = 136 within the row
        let fixed_header_size: u16 = 0x5C; // 92 bytes
        let num_offsets: u16 = 22;
        let offset_array_size: u16 = num_offsets * 2; // 44 bytes for U16 offsets
        let string_data_start: u16 = fixed_header_size + offset_array_size; // 136
        let mut current_position = string_data_start;
        let mut offsets: Vec<u16> = Vec::new();

        for (i, s) in strings.iter().enumerate() {
            if i == 0 {
                // First offset is always 3 (convention to match rekordbox)
                offsets.push(3);
            } else {
                offsets.push(current_position);
            }
            current_position += s.binary_size() as u16;
        }

        // Write the offsets (44 bytes = 22 U16 values)
        for offset in &offsets {
            row.extend_from_slice(&offset.to_le_bytes());
        }

        // Write the strings
        let mut string_buf = Vec::new();
        for s in &strings {
            s.write(&mut string_buf).unwrap();
        }
        row.extend_from_slice(&string_buf);

        Ok(row)
    }

    /// Build genre pages
    fn build_genre_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Genres, 0);

        let mut genres: Vec<_> = self.genres.iter().collect();
        genres.sort_by_key(|(_, id)| *id);

        for (name, id) in genres {
            let row_data = self.build_genre_row(*id, name)?;

            if !builder.add_row(&row_data) {
                pages.push(builder.build());
                builder = PageBuilder::new(TableType::Genres, 0);
                builder.add_row(&row_data);
            }
        }

        if !self.genres.is_empty() {
            pages.push(builder.build());
        }

        Ok(pages)
    }

    /// Build a genre row
    fn build_genre_row(&self, id: u32, name: &str) -> Result<Vec<u8>> {
        let mut row = Vec::new();

        // Genre row structure:
        // 4 bytes: ID
        // Variable: DeviceSQLString name

        row.extend_from_slice(&id.to_le_bytes());

        let name_str = DeviceSQLString::new(name);
        name_str.write(&mut row).unwrap();

        Ok(row)
    }

    /// Build artist pages
    fn build_artist_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Artists, 0);

        let mut artists: Vec<_> = self.artists.iter().collect();
        artists.sort_by_key(|(_, id)| *id);

        for (row_index, (name, id)) in artists.iter().enumerate() {
            let row_data = self.build_artist_row(**id, name, row_index as u16)?;

            if !builder.add_row(&row_data) {
                pages.push(builder.build());
                builder = PageBuilder::new(TableType::Artists, 0);
                builder.add_row(&row_data);
            }
        }

        if !self.artists.is_empty() {
            pages.push(builder.build());
        }

        Ok(pages)
    }

    /// Build an artist row
    fn build_artist_row(&self, id: u32, name: &str, row_index: u16) -> Result<Vec<u8>> {
        let mut row = Vec::new();

        // Artist row structure:
        // 2 bytes: Subtype (0x60)
        // 2 bytes: Index shift (0x20 * row_index)
        // 4 bytes: ID
        // 2 bytes: Offset array (2 offsets for name)
        // Variable: Name string

        row.extend_from_slice(&0x0060u16.to_le_bytes());
        row.extend_from_slice(&((row_index * 0x20) as u16).to_le_bytes());
        row.extend_from_slice(&id.to_le_bytes());

        // Offset array: 2 offsets (8 bytes base + 2 bytes for offsets = start at 10)
        let name_str = DeviceSQLString::new(name);
        let name_offset = 10u8; // Name starts at offset 10 (after fixed header + 2 offset bytes)

        row.push(3); // First offset (convention: always 3 to match rekordbox)
        row.push(name_offset);

        name_str.write(&mut row).unwrap();

        Ok(row)
    }

    /// Build album pages
    fn build_album_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Albums, 0);

        let mut albums: Vec<_> = self.albums.iter().collect();
        albums.sort_by_key(|(_, id)| *id);

        for (row_index, (name, id)) in albums.iter().enumerate() {
            let row_data = self.build_album_row(**id, name, row_index as u16)?;

            if !builder.add_row(&row_data) {
                pages.push(builder.build());
                builder = PageBuilder::new(TableType::Albums, 0);
                builder.add_row(&row_data);
            }
        }

        if !self.albums.is_empty() {
            pages.push(builder.build());
        }

        Ok(pages)
    }

    /// Build an album row
    fn build_album_row(&self, id: u32, name: &str, row_index: u16) -> Result<Vec<u8>> {
        let mut row = Vec::new();

        // Album row structure:
        // 2 bytes: Subtype (0x80)
        // 2 bytes: Index shift
        // 4 bytes: Unknown
        // 4 bytes: Artist ID (0 = no linked artist)
        // 4 bytes: ID
        // 4 bytes: Unknown
        // 2 bytes: Offset array (2 offsets)
        // Variable: Name string

        row.extend_from_slice(&0x0080u16.to_le_bytes());
        row.extend_from_slice(&((row_index * 0x20) as u16).to_le_bytes());
        row.extend_from_slice(&0u32.to_le_bytes()); // Unknown
        row.extend_from_slice(&0u32.to_le_bytes()); // Artist ID
        row.extend_from_slice(&id.to_le_bytes());
        row.extend_from_slice(&0u32.to_le_bytes()); // Unknown

        // Offset array
        let name_str = DeviceSQLString::new(name);
        let name_offset = 22u8; // After fixed header (20 bytes) + 2 offset bytes

        row.push(3); // First offset (convention: always 3 to match rekordbox)
        row.push(name_offset);

        name_str.write(&mut row).unwrap();

        Ok(row)
    }

    /// Build key pages
    fn build_key_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Keys, 0);

        let mut keys: Vec<_> = self.keys.iter().collect();
        keys.sort_by_key(|(_, id)| *id);

        for (name, id) in keys {
            let row_data = self.build_key_row(*id, name)?;

            if !builder.add_row(&row_data) {
                pages.push(builder.build());
                builder = PageBuilder::new(TableType::Keys, 0);
                builder.add_row(&row_data);
            }
        }

        if !self.keys.is_empty() {
            pages.push(builder.build());
        }

        Ok(pages)
    }

    /// Build a key row
    fn build_key_row(&self, id: u32, name: &str) -> Result<Vec<u8>> {
        let mut row = Vec::new();

        // Key row structure:
        // 4 bytes: ID
        // 4 bytes: ID again (weird but that's the format)
        // Variable: DeviceSQLString name

        row.extend_from_slice(&id.to_le_bytes());
        row.extend_from_slice(&id.to_le_bytes());

        let name_str = DeviceSQLString::new(name);
        name_str.write(&mut row).unwrap();

        Ok(row)
    }

    /// Build color pages
    fn build_color_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Colors, 0);

        let mut colors: Vec<_> = self.colors.iter().collect();
        colors.sort_by_key(|(_, id)| *id);

        for (name, id) in colors {
            let row_data = self.build_color_row(*id, name)?;

            if !builder.add_row(&row_data) {
                pages.push(builder.build());
                builder = PageBuilder::new(TableType::Colors, 0);
                builder.add_row(&row_data);
            }
        }

        if !self.colors.is_empty() {
            pages.push(builder.build());
        }

        Ok(pages)
    }

    /// Build a color row
    fn build_color_row(&self, id: u8, name: &str) -> Result<Vec<u8>> {
        let mut row = Vec::new();

        // Color row structure (from rekordcrate):
        // 4 bytes: unknown1 (usually 0)
        // 1 byte: unknown2 (should be the color ID, 1-based index)
        // 1 byte: color (ColorIndex, same value as unknown2)
        // 2 bytes: unknown3 (usually 0)
        // Variable: DeviceSQLString name

        row.extend_from_slice(&0u32.to_le_bytes()); // unknown1
        row.push(id); // unknown2 = color ID (1-based)
        row.push(id); // color = ColorIndex (same as id)
        row.extend_from_slice(&0u16.to_le_bytes()); // unknown3

        let name_str = DeviceSQLString::new(name);
        name_str.write(&mut row).unwrap();

        Ok(row)
    }

    /// Build playlist tree pages
    fn build_playlist_tree_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::PlaylistTree, 0);

        for node in &self.playlist_nodes {
            let row_data = self.build_playlist_tree_row(node)?;

            if !builder.add_row(&row_data) {
                pages.push(builder.build());
                builder = PageBuilder::new(TableType::PlaylistTree, 0);
                builder.add_row(&row_data);
            }
        }

        if !self.playlist_nodes.is_empty() {
            pages.push(builder.build());
        }

        Ok(pages)
    }

    /// Build a playlist tree row
    fn build_playlist_tree_row(&self, node: &PdbPlaylistNode) -> Result<Vec<u8>> {
        let mut row = Vec::new();

        // Playlist tree row structure:
        // 4 bytes: Parent ID
        // 4 bytes: Unknown
        // 4 bytes: Sort order
        // 4 bytes: ID
        // 4 bytes: Is folder flag (u32, not u8!)
        // Variable: DeviceSQLString name

        row.extend_from_slice(&node.parent_id.to_le_bytes());
        row.extend_from_slice(&0u32.to_le_bytes()); // Unknown
        row.extend_from_slice(&node.sort_order.to_le_bytes());
        row.extend_from_slice(&node.id.to_le_bytes());
        // node_is_folder must be u32 (0 = playlist, non-zero = folder)
        let is_folder_val: u32 = if node.is_folder { 1 } else { 0 };
        row.extend_from_slice(&is_folder_val.to_le_bytes());

        let name_str = DeviceSQLString::new(&node.name);
        name_str.write(&mut row).unwrap();

        Ok(row)
    }

    /// Build playlist entries pages
    fn build_playlist_entries_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::PlaylistEntries, 0);

        for entry in &self.playlist_entries {
            let row_data = self.build_playlist_entry_row(entry)?;

            if !builder.add_row(&row_data) {
                pages.push(builder.build());
                builder = PageBuilder::new(TableType::PlaylistEntries, 0);
                builder.add_row(&row_data);
            }
        }

        if !self.playlist_entries.is_empty() {
            pages.push(builder.build());
        }

        Ok(pages)
    }

    /// Build a playlist entry row
    fn build_playlist_entry_row(&self, entry: &PdbPlaylistEntry) -> Result<Vec<u8>> {
        let mut row = Vec::new();

        // Playlist entry row structure:
        // 4 bytes: Entry index
        // 4 bytes: Track ID
        // 4 bytes: Playlist ID

        row.extend_from_slice(&entry.entry_index.to_le_bytes());
        row.extend_from_slice(&entry.track_id.to_le_bytes());
        row.extend_from_slice(&entry.playlist_id.to_le_bytes());

        Ok(row)
    }

    /// Build column pages
    /// Columns define the menu column headings shown on CDJ/XDJ screens
    fn build_column_pages(&self) -> Result<Vec<Vec<u8>>> {
        // Column definitions matching rekordbox export format
        // Format: (id, unknown, name)
        const COLUMN_DEFS: &[(u16, u16, &str)] = &[
            (1, 128, "GENRE"),
            (2, 129, "ARTIST"),
            (3, 130, "ALBUM"),
            (4, 131, "TRACK"),
            (5, 132, "PLAYLIST"),
            (6, 133, "BPM"),
            (7, 134, "RATING"),
            (8, 135, "YEAR"),
            (9, 136, "REMIXER"),
            (10, 137, "LABEL"),
            (11, 138, "ORIGINAL ARTIST"),
            (12, 139, "KEY"),
            (13, 140, "DATE ADDED"),
            (14, 142, "COLOR"),
            (15, 143, "TIME"),
            (16, 144, "BITRATE"),
            (17, 145, "FILENAME"),
            (18, 146, "HISTORY"),
            (19, 141, "COMMENT"),
            (20, 147, "DJ PLAY COUNT"),
            (21, 148, "MY TAG"),
            (22, 149, "HOT CUE BANK"),
            (23, 150, "SEARCH"),
            (24, 151, "FOLDER"),
            (25, 152, "TRACK FILTER"),
            (26, 153, "MASTER TEMPO"),
            (27, 154, "QUANTIZE"),
        ];

        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Columns, 0);

        for &(id, unknown, name) in COLUMN_DEFS {
            let row_data = self.build_column_row(id, unknown, name)?;

            if !builder.add_row(&row_data) {
                pages.push(builder.build());
                builder = PageBuilder::new(TableType::Columns, 0);
                builder.add_row(&row_data);
            }
        }

        if !COLUMN_DEFS.is_empty() {
            pages.push(builder.build());
        }

        Ok(pages)
    }

    /// Build a column row
    fn build_column_row(&self, id: u16, unknown: u16, name: &str) -> Result<Vec<u8>> {
        let mut row = Vec::new();

        // Column row structure:
        // 2 bytes: ID (u16)
        // 2 bytes: Unknown/content pointer (u16)
        // Variable: DeviceSQLString name

        row.extend_from_slice(&id.to_le_bytes());
        row.extend_from_slice(&unknown.to_le_bytes());

        let name_str = DeviceSQLString::new(name);
        name_str.write(&mut row).unwrap();

        Ok(row)
    }
}

impl Default for RekordboxPdbWriter {
    fn default() -> Self {
        Self::new()
    }
}
