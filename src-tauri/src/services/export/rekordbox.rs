//! Rekordbox PDB file writer
//!
//! Generates export.pdb and exportExt.pdb files compatible with Pioneer CDJ/XDJ equipment.
//! Based on the Deep Symmetry analysis: https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html
//! and the rekordcrate library: https://github.com/Holzhaus/rekordcrate

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::error::{CrateError, Result};
use crate::models::{Playlist, Track};

// ============================================================================
// Constants
// ============================================================================

/// Page size for PDB files (standard is 4096 bytes)
const PAGE_SIZE: u32 = 4096;

/// Size of the file header (before table descriptors)
const FILE_HEADER_SIZE: usize = 0x1C; // 28 bytes

/// Size of a table descriptor
const TABLE_DESCRIPTOR_SIZE: usize = 16;

/// Offset where heap data begins in a data page
const HEAP_START_OFFSET: usize = 0x28; // 40 bytes

/// Page flags for index pages
const PAGE_FLAGS_INDEX: u8 = 0x64;

/// Page flags for data pages
const PAGE_FLAGS_DATA: u8 = 0x24;

/// Null page marker (indicates no next page)
const NULL_PAGE_MARKER: u32 = 0x03FF_FFFF;

/// Empty index entry marker
const EMPTY_INDEX_ENTRY: u32 = 0x1FFF_FFF8;

/// Maximum rows per row group
const MAX_ROWS_PER_GROUP: usize = 16;

/// Index page magic values
const INDEX_MAGIC: u16 = 0x03EC;
const INDEX_EMPTY_MARKER: u16 = 0x1FFF;
const INDEX_MAGIC2: u64 = 0x0000_0000_03FF_FFFF;

// ============================================================================
// Table Types
// ============================================================================

/// Table types in the PDB format
/// All 20 types (0-19) must be present in numeric order
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TableType {
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
            TableType::Tracks,
            TableType::Genres,
            TableType::Artists,
            TableType::Albums,
            TableType::Labels,
            TableType::Keys,
            TableType::Colors,
            TableType::PlaylistTree,
            TableType::PlaylistEntries,
            TableType::Unknown9,
            TableType::Unknown10,
            TableType::HistoryPlaylists,
            TableType::HistoryEntries,
            TableType::Artwork,
            TableType::Unknown14,
            TableType::Unknown15,
            TableType::Columns,
            TableType::Menu,
            TableType::Unknown18,
            TableType::History,
        ]
    }
}

// ============================================================================
// DeviceSQL String Encoding
// ============================================================================

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
    fn write_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
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
                // Length: content length + 4 (includes header)
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
                // Length: (char count * 2) + 4 (includes header)
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

// ============================================================================
// Row Group Structure
// ============================================================================

/// A row group contains up to 16 row offsets and presence flags
#[derive(Debug, Clone)]
struct RowGroup {
    /// Offsets to rows within the page heap (relative to heap start at 0x28)
    row_offsets: [u16; MAX_ROWS_PER_GROUP],
    /// Bitmask indicating which rows are present
    presence_flags: u16,
    /// Number of rows in this group
    num_rows: usize,
}

impl RowGroup {
    fn new() -> Self {
        Self {
            row_offsets: [0; MAX_ROWS_PER_GROUP],
            presence_flags: 0,
            num_rows: 0,
        }
    }

    fn add_row(&mut self, offset: u16) -> bool {
        if self.num_rows >= MAX_ROWS_PER_GROUP {
            return false;
        }
        // Store offset at the position corresponding to this row
        self.row_offsets[self.num_rows] = offset;
        // Set presence bit
        self.presence_flags |= 1 << self.num_rows;
        self.num_rows += 1;
        true
    }

    fn is_full(&self) -> bool {
        self.num_rows >= MAX_ROWS_PER_GROUP
    }

    /// Calculate the size this row group will take at the end of the page
    fn binary_size(&self) -> usize {
        // Row offsets are stored from the end, skipping leading zeros
        // Format: [present offsets...] [presence_flags: u16] [row_count: u16]
        let leading_zeros = self.presence_flags.leading_zeros() as usize;
        let num_offsets = 16 - leading_zeros;
        num_offsets * 2 + 4 // offsets + presence_flags + row_count
    }

    /// Write the row group to a buffer (at the correct position)
    fn write_to(&self, buffer: &mut [u8], end_position: usize) {
        let leading_zeros = self.presence_flags.leading_zeros() as usize;
        let num_offsets = 16 - leading_zeros;
        let group_size = num_offsets * 2 + 4;
        let start_pos = end_position - group_size;

        let mut pos = start_pos;

        // Write the present offsets (from first present to last)
        for i in 0..num_offsets {
            let idx = leading_zeros + i;
            buffer[pos..pos + 2].copy_from_slice(&self.row_offsets[idx].to_le_bytes());
            pos += 2;
        }

        // Write presence flags
        buffer[pos..pos + 2].copy_from_slice(&self.presence_flags.to_le_bytes());
        pos += 2;

        // Write row count (this is what rekordbox expects)
        buffer[pos..pos + 2].copy_from_slice(&(self.num_rows as u16).to_le_bytes());
    }
}

// ============================================================================
// Page Builder
// ============================================================================

/// Builds a page with proper layout
#[allow(dead_code)]
struct PageBuilder {
    /// Page type (table type)
    page_type: TableType,
    /// Page index (will be set during final write)
    page_index: u32,
    /// Next page index (will be set during final write)
    next_page: u32,
    /// Row data buffer (heap)
    heap_data: Vec<u8>,
    /// Row groups
    row_groups: Vec<RowGroup>,
    /// Current row group being filled
    current_group: RowGroup,
    /// Total row count on this page
    total_rows: u16,
    /// Sequence number for this page
    sequence: u32,
}

impl PageBuilder {
    fn new(page_type: TableType) -> Self {
        Self {
            page_type,
            page_index: 0,
            next_page: 0,
            heap_data: Vec::new(),
            row_groups: Vec::new(),
            current_group: RowGroup::new(),
            total_rows: 0,
            sequence: 1,
        }
    }

    /// Calculate the size needed for all row groups
    fn row_groups_size(&self) -> usize {
        let mut size = 0;
        for group in &self.row_groups {
            size += group.binary_size();
        }
        if self.current_group.num_rows > 0 {
            size += self.current_group.binary_size();
        }
        size
    }

    /// Calculate available space for new row data
    fn available_space(&self) -> usize {
        let total_page = PAGE_SIZE as usize;
        let header_space = HEAP_START_OFFSET;
        let heap_used = self.heap_data.len();
        let row_groups_space = self.row_groups_size();

        // Need to account for potential new row group entry (worst case: 4 bytes)
        let potential_index_growth = if self.current_group.is_full() { 36 } else { 2 };

        total_page
            .saturating_sub(header_space)
            .saturating_sub(heap_used)
            .saturating_sub(row_groups_space)
            .saturating_sub(potential_index_growth)
    }

    /// Add a row to the page, returns false if page is full
    fn add_row(&mut self, row_data: &[u8]) -> bool {
        if row_data.len() > self.available_space() {
            return false;
        }

        // Record the offset where this row starts (relative to heap start)
        let offset = self.heap_data.len() as u16;

        // If current group is full, finalize it and start a new one
        if self.current_group.is_full() {
            self.row_groups.push(self.current_group.clone());
            self.current_group = RowGroup::new();
        }

        // Add the row offset to current group
        self.current_group.add_row(offset);
        self.total_rows += 1;

        // Append row data to heap
        self.heap_data.extend_from_slice(row_data);

        true
    }

    /// Build the complete page as a byte vector
    fn build(mut self, page_index: u32, next_page: u32, sequence: u32) -> Vec<u8> {
        let mut page = vec![0u8; PAGE_SIZE as usize];

        // Finalize current group if it has rows
        if self.current_group.num_rows > 0 {
            self.row_groups.push(self.current_group.clone());
        }

        // Calculate sizes
        let used_size = self.heap_data.len() as u16;
        let row_groups_total_size: usize = self.row_groups.iter().map(|g| g.binary_size()).sum();
        let heap_capacity = (PAGE_SIZE as usize) - HEAP_START_OFFSET;
        let free_size = (heap_capacity - self.heap_data.len() - row_groups_total_size) as u16;

        // Calculate packed row counts
        // num_rows (lower 5 bits of byte at 0x1A) and num_row_groups (13 bits)
        let num_row_groups = self.row_groups.len() as u32;
        // Packed format: bits 0-12 = num_row_groups, bits 13-23 = num_rows
        let packed = (num_row_groups & 0x1FFF) | (((self.total_rows as u32) & 0x7FF) << 13);

        // === Write Page Header (32 bytes at 0x00-0x1F) ===
        // 0x00-0x03: Magic (zeros)
        // 0x04-0x07: Page index
        page[0x04..0x08].copy_from_slice(&page_index.to_le_bytes());
        // 0x08-0x0B: Page type
        page[0x08..0x0C].copy_from_slice(&(self.page_type as u32).to_le_bytes());
        // 0x0C-0x0F: Next page
        page[0x0C..0x10].copy_from_slice(&next_page.to_le_bytes());
        // 0x10-0x13: Unknown1 (sequence/version)
        page[0x10..0x14].copy_from_slice(&sequence.to_le_bytes());
        // 0x14-0x17: Unknown2 (zeros)
        // 0x18-0x1A: Packed row counts (3 bytes)
        page[0x18..0x1B].copy_from_slice(&packed.to_le_bytes()[0..3]);
        // 0x1B: Page flags (0x24 for data pages)
        page[0x1B] = PAGE_FLAGS_DATA;
        // 0x1C-0x1D: Free size
        page[0x1C..0x1E].copy_from_slice(&free_size.to_le_bytes());
        // 0x1E-0x1F: Used size
        page[0x1E..0x20].copy_from_slice(&used_size.to_le_bytes());

        // === Write Data Page Header (8 bytes at 0x20-0x27) ===
        // These values match what rekordbox produces
        if self.total_rows == 0 {
            page[0x20..0x22].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());
            page[0x22..0x24].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());
        } else {
            page[0x20..0x22].copy_from_slice(&(self.total_rows.min(0x1FFF)).to_le_bytes());
            page[0x22..0x24].copy_from_slice(&((self.total_rows - 1).min(0x1FFF)).to_le_bytes());
        }
        // 0x24-0x27: Other unknown fields (zeros)

        // === Write Heap Data (starting at 0x28) ===
        page[HEAP_START_OFFSET..HEAP_START_OFFSET + self.heap_data.len()]
            .copy_from_slice(&self.heap_data);

        // === Write Row Groups (backwards from page end) ===
        let mut end_position = PAGE_SIZE as usize;
        for group in self.row_groups.iter().rev() {
            let group_size = group.binary_size();
            group.write_to(&mut page, end_position);
            end_position -= group_size;
        }

        page
    }
}

// ============================================================================
// Index Page Builder
// ============================================================================

/// Builds an index page for a table
fn build_index_page(
    page_type: TableType,
    page_index: u32,
    next_page: u32,
    data_page_indices: &[u32],
    sequence: u32,
) -> Vec<u8> {
    let mut page = vec![0u8; PAGE_SIZE as usize];

    // === Page Header (32 bytes at 0x00-0x1F) ===
    // 0x04-0x07: Page index
    page[0x04..0x08].copy_from_slice(&page_index.to_le_bytes());
    // 0x08-0x0B: Page type
    page[0x08..0x0C].copy_from_slice(&(page_type as u32).to_le_bytes());
    // 0x0C-0x0F: Next page (points to first data page)
    page[0x0C..0x10].copy_from_slice(&next_page.to_le_bytes());
    // 0x10-0x13: Unknown1 (sequence)
    page[0x10..0x14].copy_from_slice(&sequence.to_le_bytes());
    // 0x1B: Page flags (0x64 for index pages)
    page[0x1B] = PAGE_FLAGS_INDEX;

    // === Index Page Header (starts at 0x20) ===
    // 0x20-0x21: unknown_a (0x1fff)
    page[0x20..0x22].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());
    // 0x22-0x23: unknown_b (0x1fff)
    page[0x22..0x24].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());
    // 0x24-0x25: magic (0x03ec)
    page[0x24..0x26].copy_from_slice(&INDEX_MAGIC.to_le_bytes());
    // 0x26-0x27: next_offset (number of entries)
    page[0x26..0x28].copy_from_slice(&(data_page_indices.len() as u16).to_le_bytes());
    // 0x28-0x2B: page_index (redundant)
    page[0x28..0x2C].copy_from_slice(&page_index.to_le_bytes());
    // 0x2C-0x2F: next_page (redundant)
    page[0x2C..0x30].copy_from_slice(&next_page.to_le_bytes());
    // 0x30-0x37: magic2 (0x0000000003ffffff)
    page[0x30..0x38].copy_from_slice(&INDEX_MAGIC2.to_le_bytes());
    // 0x38-0x39: num_entries
    page[0x38..0x3A].copy_from_slice(&(data_page_indices.len() as u16).to_le_bytes());
    // 0x3A-0x3B: first_empty (0x1fff)
    page[0x3A..0x3C].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());

    // === Index Entries (starting at 0x3C) ===
    // Each entry is 4 bytes: (page_index << 3) | flags
    let mut offset = 0x3C;
    for &data_page_idx in data_page_indices {
        let entry = data_page_idx << 3; // flags = 0
        page[offset..offset + 4].copy_from_slice(&entry.to_le_bytes());
        offset += 4;
    }

    // Fill remaining entries with empty marker (0x1ffffff8)
    let remaining_space = (PAGE_SIZE as usize) - offset - 20; // Leave 20 bytes at end
    let empty_entries = remaining_space / 4;
    for _ in 0..empty_entries {
        page[offset..offset + 4].copy_from_slice(&EMPTY_INDEX_ENTRY.to_le_bytes());
        offset += 4;
    }

    // Last 20 bytes are zeros (already initialized)

    page
}

/// Builds an empty index page for tables with no data
fn build_empty_index_page(page_type: TableType, page_index: u32, sequence: u32) -> Vec<u8> {
    let mut page = vec![0u8; PAGE_SIZE as usize];

    // === Page Header ===
    page[0x04..0x08].copy_from_slice(&page_index.to_le_bytes());
    page[0x08..0x0C].copy_from_slice(&(page_type as u32).to_le_bytes());
    page[0x0C..0x10].copy_from_slice(&NULL_PAGE_MARKER.to_le_bytes()); // No next page
    page[0x10..0x14].copy_from_slice(&sequence.to_le_bytes());
    page[0x1B] = PAGE_FLAGS_INDEX;

    // === Index Page Header ===
    page[0x20..0x22].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());
    page[0x22..0x24].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());
    page[0x24..0x26].copy_from_slice(&INDEX_MAGIC.to_le_bytes());
    page[0x26..0x28].copy_from_slice(&0u16.to_le_bytes()); // next_offset = 0
    page[0x28..0x2C].copy_from_slice(&page_index.to_le_bytes());
    page[0x2C..0x30].copy_from_slice(&NULL_PAGE_MARKER.to_le_bytes());
    page[0x30..0x38].copy_from_slice(&INDEX_MAGIC2.to_le_bytes());
    page[0x38..0x3A].copy_from_slice(&0u16.to_le_bytes()); // num_entries = 0
    page[0x3A..0x3C].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());

    // Fill all index entries with empty marker
    let mut offset = 0x3C;
    let remaining_space = (PAGE_SIZE as usize) - offset - 20;
    let empty_entries = remaining_space / 4;
    for _ in 0..empty_entries {
        page[offset..offset + 4].copy_from_slice(&EMPTY_INDEX_ENTRY.to_le_bytes());
        offset += 4;
    }

    page
}

// ============================================================================
// Internal Data Structures
// ============================================================================

/// Internal track data for PDB generation
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
    #[allow(dead_code)]
    file_type: u16,
    date_added: String,
    comment: String,
    anlz_path: String,
}

/// Internal playlist node for PDB generation
#[derive(Debug, Clone)]
struct PdbPlaylistNode {
    id: u32,
    parent_id: u32,
    name: String,
    is_folder: bool,
    sort_order: u32,
}

/// Internal playlist entry for PDB generation
#[derive(Debug, Clone)]
struct PdbPlaylistEntry {
    entry_index: u32,
    track_id: u32,
    playlist_id: u32,
}

// ============================================================================
// Row Building Functions
// ============================================================================

/// Build a track row (most complex row type)
fn build_track_row(track: &PdbTrack, row_index: u16) -> Vec<u8> {
    let mut row = Vec::new();

    // Fixed header (0x5C = 92 bytes)
    // 0x00-0x01: Subtype (0x24 = U16 offsets)
    row.extend_from_slice(&0x0024u16.to_le_bytes());
    // 0x02-0x03: Index shift (row_index * 0x20)
    row.extend_from_slice(&(row_index.wrapping_mul(0x20)).to_le_bytes());
    // 0x04-0x07: Bitmask (0x000c0700)
    row.extend_from_slice(&0x000c0700u32.to_le_bytes());
    // 0x08-0x0B: Sample rate
    row.extend_from_slice(&track.sample_rate.to_le_bytes());
    // 0x0C-0x0F: Composer ID (0 = none)
    row.extend_from_slice(&0u32.to_le_bytes());
    // 0x10-0x13: File size
    row.extend_from_slice(&track.file_size.to_le_bytes());
    // 0x14-0x17: Unknown2
    row.extend_from_slice(&0u32.to_le_bytes());
    // 0x18-0x1B: Unknown3/4 (2x u16)
    row.extend_from_slice(&0u32.to_le_bytes());
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
    // 0x52-0x53: Sample depth (16 or 24)
    row.extend_from_slice(&16u16.to_le_bytes());
    // 0x54-0x55: Duration (seconds)
    row.extend_from_slice(&track.duration_seconds.to_le_bytes());
    // 0x56-0x57: Unknown5 (0x0029)
    row.extend_from_slice(&0x0029u16.to_le_bytes());
    // 0x58: Color ID
    row.push(track.color_id);
    // 0x59: Rating (0-5)
    row.push(track.rating);
    // 0x5A-0x5B: File type
    row.extend_from_slice(&track.file_type.to_le_bytes());

    // Build the 22 strings
    let strings: [DeviceSQLString; 22] = [
        DeviceSQLString::empty(),                   // 0: ISRC (placeholder)
        DeviceSQLString::empty(),                   // 1: Lyricist
        DeviceSQLString::empty(),                   // 2: Unknown
        DeviceSQLString::empty(),                   // 3: Unknown
        DeviceSQLString::empty(),                   // 4: Unknown
        DeviceSQLString::empty(),                   // 5: Message
        DeviceSQLString::empty(),                   // 6: Publish info
        DeviceSQLString::empty(),                   // 7: Autoload hotcues
        DeviceSQLString::empty(),                   // 8: Unknown
        DeviceSQLString::empty(),                   // 9: Unknown
        DeviceSQLString::new(&track.date_added),    // 10: Date added
        DeviceSQLString::empty(),                   // 11: Release date
        DeviceSQLString::empty(),                   // 12: Mix name
        DeviceSQLString::empty(),                   // 13: Unknown
        DeviceSQLString::new(&track.anlz_path),     // 14: Analyze path
        DeviceSQLString::empty(),                   // 15: Analyze date
        DeviceSQLString::new(&track.comment),       // 16: Comment
        DeviceSQLString::new(&track.title),         // 17: Title
        DeviceSQLString::empty(),                   // 18: Unknown
        DeviceSQLString::new(&track.filename),      // 19: Filename
        DeviceSQLString::new(&track.file_path),     // 20: File path
        DeviceSQLString::empty(),                   // 21: Extra (padding)
    ];

    // Calculate string offsets
    // Fixed header = 0x5C (92 bytes)
    // 22 u16 offsets = 44 bytes
    // String data starts at 92 + 44 = 136
    let fixed_header_size: u16 = 0x5C; // 92 bytes
    let offset_array_size: u16 = 22 * 2; // 44 bytes for 22 u16 offsets
    let string_data_start: u16 = fixed_header_size + offset_array_size; // 136

    let mut offsets: Vec<u16> = Vec::with_capacity(22);
    let mut current_pos = string_data_start;

    // First offset is always 3 (convention)
    offsets.push(3);

    // Calculate remaining offsets
    for (i, s) in strings.iter().enumerate() {
        if i > 0 {
            offsets.push(current_pos);
        }
        current_pos += s.binary_size() as u16;
    }

    // Write the 22 u16 offsets (44 bytes)
    for offset in &offsets {
        row.extend_from_slice(&offset.to_le_bytes());
    }

    // Write the strings
    let mut string_buf = Vec::new();
    for s in &strings {
        s.write_to(&mut string_buf).unwrap();
    }
    row.extend_from_slice(&string_buf);

    row
}

/// Build an artist row (short variant with u8 offsets)
fn build_artist_row(id: u32, name: &str, row_index: u16) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x01: Subtype (0x60 = short variant, u8 offsets)
    row.extend_from_slice(&0x0060u16.to_le_bytes());
    // 0x02-0x03: Index shift
    row.extend_from_slice(&(row_index.wrapping_mul(0x20)).to_le_bytes());
    // 0x04-0x07: ID
    row.extend_from_slice(&id.to_le_bytes());
    // 0x08-0x09: Offset array [3, name_offset]
    // Name starts at offset 10 (after 8 bytes header + 2 bytes offsets)
    row.push(3); // First offset is always 3
    row.push(10); // Name offset

    // Write the name string
    let name_str = DeviceSQLString::new(name);
    name_str.write_to(&mut row).unwrap();

    row
}

/// Build an album row (short variant with u8 offsets)
fn build_album_row(id: u32, name: &str, artist_id: u32, row_index: u16) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x01: Subtype (0x80 = short variant, u8 offsets)
    row.extend_from_slice(&0x0080u16.to_le_bytes());
    // 0x02-0x03: Index shift
    row.extend_from_slice(&(row_index.wrapping_mul(0x20)).to_le_bytes());
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
    name_str.write_to(&mut row).unwrap();

    row
}

/// Build a genre row
fn build_genre_row(id: u32, name: &str) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x03: ID
    row.extend_from_slice(&id.to_le_bytes());

    // Name string
    let name_str = DeviceSQLString::new(name);
    name_str.write_to(&mut row).unwrap();

    row
}

/// Build a key row
fn build_key_row(id: u32, name: &str) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x03: ID
    row.extend_from_slice(&id.to_le_bytes());
    // 0x04-0x07: ID again
    row.extend_from_slice(&id.to_le_bytes());

    // Name string
    let name_str = DeviceSQLString::new(name);
    name_str.write_to(&mut row).unwrap();

    row
}

/// Build a color row
fn build_color_row(id: u8, name: &str) -> Vec<u8> {
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
    name_str.write_to(&mut row).unwrap();

    row
}

/// Build a playlist tree row
fn build_playlist_tree_row(node: &PdbPlaylistNode) -> Vec<u8> {
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
    name_str.write_to(&mut row).unwrap();

    row
}

/// Build a playlist entry row
fn build_playlist_entry_row(entry: &PdbPlaylistEntry) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x03: Entry index
    row.extend_from_slice(&entry.entry_index.to_le_bytes());
    // 0x04-0x07: Track ID
    row.extend_from_slice(&entry.track_id.to_le_bytes());
    // 0x08-0x0B: Playlist ID
    row.extend_from_slice(&entry.playlist_id.to_le_bytes());

    row
}

/// Build a column row (menu column definition)
fn build_column_row(id: u16, unknown: u16, name: &str) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x01: ID
    row.extend_from_slice(&id.to_le_bytes());
    // 0x02-0x03: Unknown/content pointer
    row.extend_from_slice(&unknown.to_le_bytes());

    // Name string
    let name_str = DeviceSQLString::new(name);
    name_str.write_to(&mut row).unwrap();

    row
}

// ============================================================================
// Standard Color Definitions
// ============================================================================

/// Standard Rekordbox color definitions
const STANDARD_COLORS: &[(u8, &str)] = &[
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
fn color_name_to_id(name: &str) -> u8 {
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

// ============================================================================
// Column Definitions
// ============================================================================

/// Standard column definitions for CDJ/XDJ menu
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

// ============================================================================
// Rekordbox PDB Writer
// ============================================================================

/// Rekordbox PDB file writer
pub struct RekordboxPdbWriter {
    // Deduplicated lookup tables
    artists: HashMap<String, u32>,
    albums: HashMap<(String, u32), u32>, // (name, artist_id) -> album_id
    genres: HashMap<String, u32>,
    keys: HashMap<String, u32>,

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
            tracks: Vec::new(),
            playlist_nodes: Vec::new(),
            playlist_entries: Vec::new(),
            next_track_id: 1,
            next_artist_id: 1,
            next_album_id: 1,
            next_genre_id: 1,
            next_key_id: 1,
            next_playlist_id: 1,
            next_entry_index: 1,
            playlist_id_map: HashMap::new(),
        }
    }

    /// Create from existing PDB data (for merging)
    /// Note: Full merging is not yet implemented - this starts fresh
    pub fn from_existing(_data: &[u8]) -> Result<Self> {
        log::warn!("Merging with existing PDB not yet implemented, starting fresh");
        Ok(Self::new())
    }

    /// Add a track and return its PDB ID
    pub fn add_track(&mut self, track: &Track, usb_path: &str, anlz_path: &str) -> u32 {
        let id = self.next_track_id;
        self.next_track_id += 1;

        // Get or create artist
        let artist_name = track
            .artist
            .clone()
            .unwrap_or_else(|| "Unknown Artist".to_string());
        let artist_id = *self.artists.entry(artist_name.clone()).or_insert_with(|| {
            let id = self.next_artist_id;
            self.next_artist_id += 1;
            id
        });

        // Get or create album (keyed by name + artist_id for proper dedup)
        let album_name = track
            .album
            .clone()
            .unwrap_or_else(|| "Unknown Album".to_string());
        let album_id = *self
            .albums
            .entry((album_name.clone(), artist_id))
            .or_insert_with(|| {
                let id = self.next_album_id;
                self.next_album_id += 1;
                id
            });

        // Get or create genre
        let genre_name = track.genre.clone().unwrap_or_default();
        let genre_id = if genre_name.is_empty() {
            0
        } else {
            *self.genres.entry(genre_name).or_insert_with(|| {
                let id = self.next_genre_id;
                self.next_genre_id += 1;
                id
            })
        };

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

        // Get color ID
        let color_id = track
            .color
            .as_ref()
            .map(|c| color_name_to_id(c))
            .unwrap_or(0);

        // Calculate tempo (BPM * 100)
        let tempo = track.bpm.map(|b| (b * 100.0) as u32).unwrap_or(0);

        // Build the USB file path (with leading /)
        let file_path = format!("/Contents/{usb_path}");

        // Extract filename from path
        let filename = std::path::Path::new(&track.file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Get file size if available
        let file_size = std::fs::metadata(&track.file_path)
            .map(|m| m.len() as u32)
            .unwrap_or(0);

        // Detect file type from extension
        let file_type = std::path::Path::new(&track.file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| match ext.to_lowercase().as_str() {
                "mp3" => 0x01,
                "m4a" | "aac" => 0x04,
                "flac" => 0x05,
                "wav" => 0x0B,
                "aiff" | "aif" => 0x0C,
                _ => 0x00,
            })
            .unwrap_or(0x00);

        // Get date added - extract just the date part (YYYY-MM-DD)
        let date_added = track
            .date_added
            .split('T')
            .next()
            .unwrap_or("")
            .to_string();

        let pdb_track = PdbTrack {
            id,
            title: track
                .title
                .clone()
                .unwrap_or_else(|| "Untitled".to_string()),
            artist_id,
            album_id,
            genre_id,
            key_id,
            color_id,
            duration_seconds: (track.duration_ms / 1000) as u16,
            tempo,
            bitrate: track.bitrate.unwrap_or(0) as u32,
            sample_rate: track.sample_rate.unwrap_or(44100) as u32,
            file_size,
            file_path,
            filename,
            rating: track.rating as u8,
            year: track.year.unwrap_or(0) as u16,
            file_type,
            date_added,
            comment: String::new(),
            anlz_path: anlz_path.to_string(),
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
            .map_err(|e| CrateError::Device(format!("Failed to create PDB file: {e}")))?;
        let mut writer = BufWriter::new(file);

        // Build all table data
        let table_data = self.build_all_tables()?;

        // Calculate page layout
        // Page 0 = header
        // Then for each table: index page + data pages
        let mut current_page: u32 = 1;
        let mut table_layouts: Vec<TableLayout> = Vec::new();

        for (table_type, data_pages) in &table_data {
            if data_pages.is_empty() {
                // Empty table: just index page
                table_layouts.push(TableLayout {
                    table_type: *table_type,
                    index_page: current_page,
                    first_data_page: None,
                    last_data_page: None,
                    empty_candidate: current_page, // Points to itself for empty
                });
                current_page += 1;
            } else {
                // Table with data: index page + data pages
                let index_page = current_page;
                let first_data_page = current_page + 1;
                let last_data_page = first_data_page + (data_pages.len() as u32) - 1;

                table_layouts.push(TableLayout {
                    table_type: *table_type,
                    index_page,
                    first_data_page: Some(first_data_page),
                    last_data_page: Some(last_data_page),
                    empty_candidate: last_data_page + 1, // Next available
                });
                current_page = last_data_page + 1;
            }
        }

        let total_pages = current_page;

        // Write file header (page 0)
        self.write_file_header(&mut writer, total_pages, &table_layouts)?;

        // Write table pages
        let sequence = 1u32;
        for (i, (table_type, data_pages)) in table_data.iter().enumerate() {
            let layout = &table_layouts[i];

            if data_pages.is_empty() {
                // Empty table: just write empty index page
                let index_page =
                    build_empty_index_page(*table_type, layout.index_page, sequence);
                writer
                    .write_all(&index_page)
                    .map_err(|e| CrateError::Device(format!("Failed to write page: {e}")))?;
            } else {
                // Table with data: write index page + data pages
                let data_page_indices: Vec<u32> = (layout.first_data_page.unwrap()
                    ..=layout.last_data_page.unwrap())
                    .collect();

                let index_page = build_index_page(
                    *table_type,
                    layout.index_page,
                    layout.first_data_page.unwrap(),
                    &data_page_indices,
                    sequence,
                );
                writer
                    .write_all(&index_page)
                    .map_err(|e| CrateError::Device(format!("Failed to write index page: {e}")))?;

                // Write data pages
                for (j, page_data) in data_pages.iter().enumerate() {
                    let page_idx = layout.first_data_page.unwrap() + j as u32;
                    let next_page = if page_idx < layout.last_data_page.unwrap() {
                        page_idx + 1
                    } else {
                        NULL_PAGE_MARKER
                    };

                    // The page was built with placeholder indices, update them
                    let mut page = page_data.clone();
                    page[0x04..0x08].copy_from_slice(&page_idx.to_le_bytes());
                    page[0x0C..0x10].copy_from_slice(&next_page.to_le_bytes());

                    writer
                        .write_all(&page)
                        .map_err(|e| CrateError::Device(format!("Failed to write data page: {e}")))?;
                }
            }
        }

        writer
            .flush()
            .map_err(|e| CrateError::Device(format!("Failed to flush PDB file: {e}")))?;

        Ok(())
    }

    /// Write the file header (first page)
    fn write_file_header<W: Write>(
        &self,
        writer: &mut W,
        total_pages: u32,
        table_layouts: &[TableLayout],
    ) -> Result<()> {
        let mut page = vec![0u8; PAGE_SIZE as usize];

        // File header structure:
        // 0x00-0x03: Magic (always 0)
        // 0x04-0x07: Page size
        page[0x04..0x08].copy_from_slice(&PAGE_SIZE.to_le_bytes());
        // 0x08-0x0B: Number of tables
        page[0x08..0x0C].copy_from_slice(&(table_layouts.len() as u32).to_le_bytes());
        // 0x0C-0x0F: Next unused page
        page[0x0C..0x10].copy_from_slice(&total_pages.to_le_bytes());
        // 0x10-0x13: Unknown (0)
        // 0x14-0x17: Sequence number
        page[0x14..0x18].copy_from_slice(&1u32.to_le_bytes());
        // 0x18-0x1B: Gap (0)

        // Table descriptors start at 0x1C
        let mut offset = FILE_HEADER_SIZE;
        for layout in table_layouts {
            // Type
            page[offset..offset + 4].copy_from_slice(&(layout.table_type as u32).to_le_bytes());
            // Empty candidate
            page[offset + 4..offset + 8].copy_from_slice(&layout.empty_candidate.to_le_bytes());
            // First page (index page)
            page[offset + 8..offset + 12].copy_from_slice(&layout.index_page.to_le_bytes());
            // Last page (last data page, or index page if empty)
            let last_page = layout.last_data_page.unwrap_or(layout.index_page);
            page[offset + 12..offset + 16].copy_from_slice(&last_page.to_le_bytes());
            offset += TABLE_DESCRIPTOR_SIZE;
        }

        writer
            .write_all(&page)
            .map_err(|e| CrateError::Device(format!("Failed to write header: {e}")))?;

        Ok(())
    }

    /// Build all table data pages
    fn build_all_tables(&self) -> Result<Vec<(TableType, Vec<Vec<u8>>)>> {
        let mut result = Vec::new();

        for &table_type in TableType::all_required() {
            let pages = match table_type {
                TableType::Tracks => self.build_track_pages()?,
                TableType::Genres => self.build_genre_pages()?,
                TableType::Artists => self.build_artist_pages()?,
                TableType::Albums => self.build_album_pages()?,
                TableType::Labels => Vec::new(),
                TableType::Keys => self.build_key_pages()?,
                TableType::Colors => self.build_color_pages()?,
                TableType::PlaylistTree => self.build_playlist_tree_pages()?,
                TableType::PlaylistEntries => self.build_playlist_entries_pages()?,
                TableType::Artwork => Vec::new(),
                TableType::Columns => self.build_column_pages()?,
                _ => Vec::new(), // Empty tables
            };
            result.push((table_type, pages));
        }

        Ok(result)
    }

    /// Build track pages
    fn build_track_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Tracks);

        for (idx, track) in self.tracks.iter().enumerate() {
            let row_data = build_track_row(track, idx as u16);

            if !builder.add_row(&row_data) {
                // Page is full, finalize and start a new one
                pages.push(builder.build(0, 0, 1)); // Indices will be fixed later
                builder = PageBuilder::new(TableType::Tracks);
                builder.add_row(&row_data);
            }
        }

        // Finalize last page if it has data
        if builder.total_rows > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

    /// Build genre pages
    fn build_genre_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Genres);

        let mut genres: Vec<_> = self.genres.iter().collect();
        genres.sort_by_key(|(_, id)| *id);

        for (name, id) in genres {
            let row_data = build_genre_row(*id, name);

            if !builder.add_row(&row_data) {
                pages.push(builder.build(0, 0, 1));
                builder = PageBuilder::new(TableType::Genres);
                builder.add_row(&row_data);
            }
        }

        if builder.total_rows > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

    /// Build artist pages
    fn build_artist_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Artists);

        let mut artists: Vec<_> = self.artists.iter().collect();
        artists.sort_by_key(|(_, id)| *id);

        for (idx, (name, id)) in artists.iter().enumerate() {
            let row_data = build_artist_row(**id, name, idx as u16);

            if !builder.add_row(&row_data) {
                pages.push(builder.build(0, 0, 1));
                builder = PageBuilder::new(TableType::Artists);
                builder.add_row(&row_data);
            }
        }

        if builder.total_rows > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

    /// Build album pages
    fn build_album_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Albums);

        let mut albums: Vec<_> = self.albums.iter().collect();
        albums.sort_by_key(|((_, _), id)| *id);

        for (idx, ((name, artist_id), id)) in albums.iter().enumerate() {
            let row_data = build_album_row(**id, name, *artist_id, idx as u16);

            if !builder.add_row(&row_data) {
                pages.push(builder.build(0, 0, 1));
                builder = PageBuilder::new(TableType::Albums);
                builder.add_row(&row_data);
            }
        }

        if builder.total_rows > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

    /// Build key pages
    fn build_key_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Keys);

        let mut keys: Vec<_> = self.keys.iter().collect();
        keys.sort_by_key(|(_, id)| *id);

        for (name, id) in keys {
            let row_data = build_key_row(*id, name);

            if !builder.add_row(&row_data) {
                pages.push(builder.build(0, 0, 1));
                builder = PageBuilder::new(TableType::Keys);
                builder.add_row(&row_data);
            }
        }

        if builder.total_rows > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

    /// Build color pages
    fn build_color_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Colors);

        // Always write all 8 standard colors
        for &(id, name) in STANDARD_COLORS {
            let row_data = build_color_row(id, name);

            if !builder.add_row(&row_data) {
                pages.push(builder.build(0, 0, 1));
                builder = PageBuilder::new(TableType::Colors);
                builder.add_row(&row_data);
            }
        }

        if builder.total_rows > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

    /// Build playlist tree pages
    fn build_playlist_tree_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::PlaylistTree);

        for node in &self.playlist_nodes {
            let row_data = build_playlist_tree_row(node);

            if !builder.add_row(&row_data) {
                pages.push(builder.build(0, 0, 1));
                builder = PageBuilder::new(TableType::PlaylistTree);
                builder.add_row(&row_data);
            }
        }

        if builder.total_rows > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

    /// Build playlist entries pages
    fn build_playlist_entries_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::PlaylistEntries);

        for entry in &self.playlist_entries {
            let row_data = build_playlist_entry_row(entry);

            if !builder.add_row(&row_data) {
                pages.push(builder.build(0, 0, 1));
                builder = PageBuilder::new(TableType::PlaylistEntries);
                builder.add_row(&row_data);
            }
        }

        if builder.total_rows > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

    /// Build column pages
    fn build_column_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Columns);

        for &(id, unknown, name) in COLUMN_DEFS {
            let row_data = build_column_row(id, unknown, name);

            if !builder.add_row(&row_data) {
                pages.push(builder.build(0, 0, 1));
                builder = PageBuilder::new(TableType::Columns);
                builder.add_row(&row_data);
            }
        }

        if builder.total_rows > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }
}

impl Default for RekordboxPdbWriter {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal struct for tracking table page layout
struct TableLayout {
    table_type: TableType,
    index_page: u32,
    first_data_page: Option<u32>,
    last_data_page: Option<u32>,
    empty_candidate: u32,
}

// ============================================================================
// Extended Format (exportExt.pdb) Support
// ============================================================================

/// Extended table types for exportExt.pdb
#[allow(dead_code)]
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExtTableType {
    Unknown0 = 0,
    Unknown1 = 1,
    Unknown2 = 2,
    Tags = 3,        // Tag/Category definitions
    TrackTags = 4,   // Track-to-tag associations
}

#[allow(dead_code)]
impl ExtTableType {
    fn all_required() -> &'static [ExtTableType] {
        &[
            ExtTableType::Unknown0,
            ExtTableType::Unknown1,
            ExtTableType::Unknown2,
            ExtTableType::Tags,
            ExtTableType::TrackTags,
        ]
    }
}

/// Internal tag data for PDB generation
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct PdbTag {
    id: u32,
    name: String,
    category_id: u32,      // 0 if this is a category
    category_pos: u32,     // Position within category
    is_category: bool,
}

/// Internal track-tag association
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct PdbTrackTag {
    track_id: u32,
    tag_id: u32,
}

/// Build a tag row (short variant with u8 offsets)
#[allow(dead_code)]
fn build_tag_row(tag: &PdbTag, row_index: u16) -> Vec<u8> {
    let mut row = Vec::new();

    // 0x00-0x01: Subtype (0x0680 = short variant, u8 offsets)
    row.extend_from_slice(&0x0680u16.to_le_bytes());
    // 0x02-0x03: Tag index (row_index * 0x20)
    row.extend_from_slice(&(row_index.wrapping_mul(0x20)).to_le_bytes());
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
    name_str.write_to(&mut row).unwrap();

    // Write empty unknown string
    let empty_str = DeviceSQLString::empty();
    empty_str.write_to(&mut row).unwrap();

    row
}

/// Build a track-tag row
#[allow(dead_code)]
fn build_track_tag_row(track_tag: &PdbTrackTag) -> Vec<u8> {
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

/// Extended PDB writer for exportExt.pdb
#[allow(dead_code)]
pub struct RekordboxExtPdbWriter {
    tags: Vec<PdbTag>,
    track_tags: Vec<PdbTrackTag>,
    next_tag_id: u32,
    // Maps: category_name -> tag_id
    category_ids: HashMap<String, u32>,
    // Maps: (category_name, tag_name) -> tag_id
    tag_ids: HashMap<(String, String), u32>,
}

#[allow(dead_code)]
impl RekordboxExtPdbWriter {
    /// Create a new extended PDB writer
    pub fn new() -> Self {
        Self {
            tags: Vec::new(),
            track_tags: Vec::new(),
            next_tag_id: 1,
            category_ids: HashMap::new(),
            tag_ids: HashMap::new(),
        }
    }

    /// Add a tag (with its category) and return the tag ID
    /// If the category doesn't exist, it will be created automatically
    pub fn add_tag(&mut self, category_name: &str, tag_name: &str) -> u32 {
        // Check if tag already exists
        let key = (category_name.to_string(), tag_name.to_string());
        if let Some(&id) = self.tag_ids.get(&key) {
            return id;
        }

        // Get or create category
        let category_id = if let Some(&id) = self.category_ids.get(category_name) {
            id
        } else {
            let id = self.next_tag_id;
            self.next_tag_id += 1;

            // Create category entry
            self.tags.push(PdbTag {
                id,
                name: category_name.to_string(),
                category_id: 0, // Categories have category_id = 0
                category_pos: (self.category_ids.len() + 1) as u32,
                is_category: true,
            });

            self.category_ids.insert(category_name.to_string(), id);
            id
        };

        // Create tag entry
        let tag_id = self.next_tag_id;
        self.next_tag_id += 1;

        // Count existing tags in this category for position
        let tags_in_category = self
            .tags
            .iter()
            .filter(|t| !t.is_category && t.category_id == category_id)
            .count() as u32;

        self.tags.push(PdbTag {
            id: tag_id,
            name: tag_name.to_string(),
            category_id,
            category_pos: tags_in_category + 1,
            is_category: false,
        });

        self.tag_ids.insert(key, tag_id);
        tag_id
    }

    /// Associate a track with a tag
    pub fn add_track_tag(&mut self, track_id: u32, tag_id: u32) {
        self.track_tags.push(PdbTrackTag { track_id, tag_id });
    }

    /// Write the extended PDB file to disk
    pub fn write(&self, path: &Path) -> Result<()> {
        let file = File::create(path)
            .map_err(|e| CrateError::Device(format!("Failed to create exportExt.pdb: {e}")))?;
        let mut writer = BufWriter::new(file);

        // Build all table data
        let table_data = self.build_all_tables()?;

        // Calculate page layout
        let mut current_page: u32 = 1;
        let mut table_layouts: Vec<ExtTableLayout> = Vec::new();

        for (table_type, data_pages) in &table_data {
            if data_pages.is_empty() {
                table_layouts.push(ExtTableLayout {
                    table_type: *table_type,
                    index_page: current_page,
                    first_data_page: None,
                    last_data_page: None,
                    empty_candidate: current_page,
                });
                current_page += 1;
            } else {
                let index_page = current_page;
                let first_data_page = current_page + 1;
                let last_data_page = first_data_page + (data_pages.len() as u32) - 1;

                table_layouts.push(ExtTableLayout {
                    table_type: *table_type,
                    index_page,
                    first_data_page: Some(first_data_page),
                    last_data_page: Some(last_data_page),
                    empty_candidate: last_data_page + 1,
                });
                current_page = last_data_page + 1;
            }
        }

        let total_pages = current_page;

        // Write file header
        self.write_file_header(&mut writer, total_pages, &table_layouts)?;

        // Write table pages
        let sequence = 1u32;
        for (i, (table_type, data_pages)) in table_data.iter().enumerate() {
            let layout = &table_layouts[i];

            if data_pages.is_empty() {
                let index_page = build_empty_ext_index_page(*table_type, layout.index_page, sequence);
                writer
                    .write_all(&index_page)
                    .map_err(|e| CrateError::Device(format!("Failed to write page: {e}")))?;
            } else {
                let data_page_indices: Vec<u32> = (layout.first_data_page.unwrap()
                    ..=layout.last_data_page.unwrap())
                    .collect();

                let index_page = build_ext_index_page(
                    *table_type,
                    layout.index_page,
                    layout.first_data_page.unwrap(),
                    &data_page_indices,
                    sequence,
                );
                writer
                    .write_all(&index_page)
                    .map_err(|e| CrateError::Device(format!("Failed to write index page: {e}")))?;

                for (j, page_data) in data_pages.iter().enumerate() {
                    let page_idx = layout.first_data_page.unwrap() + j as u32;
                    let next_page = if page_idx < layout.last_data_page.unwrap() {
                        page_idx + 1
                    } else {
                        NULL_PAGE_MARKER
                    };

                    let mut page = page_data.clone();
                    page[0x04..0x08].copy_from_slice(&page_idx.to_le_bytes());
                    page[0x0C..0x10].copy_from_slice(&next_page.to_le_bytes());

                    writer
                        .write_all(&page)
                        .map_err(|e| CrateError::Device(format!("Failed to write data page: {e}")))?;
                }
            }
        }

        writer
            .flush()
            .map_err(|e| CrateError::Device(format!("Failed to flush exportExt.pdb: {e}")))?;

        Ok(())
    }

    fn write_file_header<W: Write>(
        &self,
        writer: &mut W,
        total_pages: u32,
        table_layouts: &[ExtTableLayout],
    ) -> Result<()> {
        let mut page = vec![0u8; PAGE_SIZE as usize];

        page[0x04..0x08].copy_from_slice(&PAGE_SIZE.to_le_bytes());
        page[0x08..0x0C].copy_from_slice(&(table_layouts.len() as u32).to_le_bytes());
        page[0x0C..0x10].copy_from_slice(&total_pages.to_le_bytes());
        page[0x14..0x18].copy_from_slice(&1u32.to_le_bytes());

        let mut offset = FILE_HEADER_SIZE;
        for layout in table_layouts {
            page[offset..offset + 4].copy_from_slice(&(layout.table_type as u32).to_le_bytes());
            page[offset + 4..offset + 8].copy_from_slice(&layout.empty_candidate.to_le_bytes());
            page[offset + 8..offset + 12].copy_from_slice(&layout.index_page.to_le_bytes());
            let last_page = layout.last_data_page.unwrap_or(layout.index_page);
            page[offset + 12..offset + 16].copy_from_slice(&last_page.to_le_bytes());
            offset += TABLE_DESCRIPTOR_SIZE;
        }

        writer
            .write_all(&page)
            .map_err(|e| CrateError::Device(format!("Failed to write header: {e}")))?;

        Ok(())
    }

    fn build_all_tables(&self) -> Result<Vec<(ExtTableType, Vec<Vec<u8>>)>> {
        let mut result = Vec::new();

        for &table_type in ExtTableType::all_required() {
            let pages = match table_type {
                ExtTableType::Tags => self.build_tag_pages()?,
                ExtTableType::TrackTags => self.build_track_tag_pages()?,
                _ => Vec::new(),
            };
            result.push((table_type, pages));
        }

        Ok(result)
    }

    fn build_tag_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = ExtPageBuilder::new(ExtTableType::Tags);

        for (idx, tag) in self.tags.iter().enumerate() {
            let row_data = build_tag_row(tag, idx as u16);

            if !builder.add_row(&row_data) {
                pages.push(builder.build(0, 0, 1));
                builder = ExtPageBuilder::new(ExtTableType::Tags);
                builder.add_row(&row_data);
            }
        }

        if builder.total_rows > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

    fn build_track_tag_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = ExtPageBuilder::new(ExtTableType::TrackTags);

        for track_tag in &self.track_tags {
            let row_data = build_track_tag_row(track_tag);

            if !builder.add_row(&row_data) {
                pages.push(builder.build(0, 0, 1));
                builder = ExtPageBuilder::new(ExtTableType::TrackTags);
                builder.add_row(&row_data);
            }
        }

        if builder.total_rows > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }
}

impl Default for RekordboxExtPdbWriter {
    fn default() -> Self {
        Self::new()
    }
}

/// Table layout for extended format
#[allow(dead_code)]
struct ExtTableLayout {
    table_type: ExtTableType,
    index_page: u32,
    first_data_page: Option<u32>,
    last_data_page: Option<u32>,
    empty_candidate: u32,
}

/// Page builder for extended format
#[allow(dead_code)]
struct ExtPageBuilder {
    page_type: ExtTableType,
    heap_data: Vec<u8>,
    row_groups: Vec<RowGroup>,
    current_group: RowGroup,
    total_rows: u16,
}

#[allow(dead_code)]
impl ExtPageBuilder {
    fn new(page_type: ExtTableType) -> Self {
        Self {
            page_type,
            heap_data: Vec::new(),
            row_groups: Vec::new(),
            current_group: RowGroup::new(),
            total_rows: 0,
        }
    }

    fn row_groups_size(&self) -> usize {
        let mut size = 0;
        for group in &self.row_groups {
            size += group.binary_size();
        }
        if self.current_group.num_rows > 0 {
            size += self.current_group.binary_size();
        }
        size
    }

    fn available_space(&self) -> usize {
        let total_page = PAGE_SIZE as usize;
        let header_space = HEAP_START_OFFSET;
        let heap_used = self.heap_data.len();
        let row_groups_space = self.row_groups_size();
        let potential_index_growth = if self.current_group.is_full() { 36 } else { 2 };

        total_page
            .saturating_sub(header_space)
            .saturating_sub(heap_used)
            .saturating_sub(row_groups_space)
            .saturating_sub(potential_index_growth)
    }

    fn add_row(&mut self, row_data: &[u8]) -> bool {
        if row_data.len() > self.available_space() {
            return false;
        }

        let offset = self.heap_data.len() as u16;

        if self.current_group.is_full() {
            self.row_groups.push(self.current_group.clone());
            self.current_group = RowGroup::new();
        }

        self.current_group.add_row(offset);
        self.total_rows += 1;
        self.heap_data.extend_from_slice(row_data);

        true
    }

    fn build(mut self, page_index: u32, next_page: u32, sequence: u32) -> Vec<u8> {
        let mut page = vec![0u8; PAGE_SIZE as usize];

        if self.current_group.num_rows > 0 {
            self.row_groups.push(self.current_group.clone());
        }

        let used_size = self.heap_data.len() as u16;
        let row_groups_total_size: usize = self.row_groups.iter().map(|g| g.binary_size()).sum();
        let heap_capacity = (PAGE_SIZE as usize) - HEAP_START_OFFSET;
        let free_size = (heap_capacity - self.heap_data.len() - row_groups_total_size) as u16;

        let num_row_groups = self.row_groups.len() as u32;
        let packed = (num_row_groups & 0x1FFF) | (((self.total_rows as u32) & 0x7FF) << 13);

        page[0x04..0x08].copy_from_slice(&page_index.to_le_bytes());
        page[0x08..0x0C].copy_from_slice(&(self.page_type as u32).to_le_bytes());
        page[0x0C..0x10].copy_from_slice(&next_page.to_le_bytes());
        page[0x10..0x14].copy_from_slice(&sequence.to_le_bytes());
        page[0x18..0x1B].copy_from_slice(&packed.to_le_bytes()[0..3]);
        page[0x1B] = PAGE_FLAGS_DATA;
        page[0x1C..0x1E].copy_from_slice(&free_size.to_le_bytes());
        page[0x1E..0x20].copy_from_slice(&used_size.to_le_bytes());

        if self.total_rows == 0 {
            page[0x20..0x22].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());
            page[0x22..0x24].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());
        } else {
            page[0x20..0x22].copy_from_slice(&(self.total_rows.min(0x1FFF)).to_le_bytes());
            page[0x22..0x24].copy_from_slice(&((self.total_rows - 1).min(0x1FFF)).to_le_bytes());
        }

        page[HEAP_START_OFFSET..HEAP_START_OFFSET + self.heap_data.len()]
            .copy_from_slice(&self.heap_data);

        let mut end_position = PAGE_SIZE as usize;
        for group in self.row_groups.iter().rev() {
            let group_size = group.binary_size();
            group.write_to(&mut page, end_position);
            end_position -= group_size;
        }

        page
    }
}

/// Build an index page for extended format
#[allow(dead_code)]
fn build_ext_index_page(
    page_type: ExtTableType,
    page_index: u32,
    next_page: u32,
    data_page_indices: &[u32],
    sequence: u32,
) -> Vec<u8> {
    let mut page = vec![0u8; PAGE_SIZE as usize];

    page[0x04..0x08].copy_from_slice(&page_index.to_le_bytes());
    page[0x08..0x0C].copy_from_slice(&(page_type as u32).to_le_bytes());
    page[0x0C..0x10].copy_from_slice(&next_page.to_le_bytes());
    page[0x10..0x14].copy_from_slice(&sequence.to_le_bytes());
    page[0x1B] = PAGE_FLAGS_INDEX;

    page[0x20..0x22].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());
    page[0x22..0x24].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());
    page[0x24..0x26].copy_from_slice(&INDEX_MAGIC.to_le_bytes());
    page[0x26..0x28].copy_from_slice(&(data_page_indices.len() as u16).to_le_bytes());
    page[0x28..0x2C].copy_from_slice(&page_index.to_le_bytes());
    page[0x2C..0x30].copy_from_slice(&next_page.to_le_bytes());
    page[0x30..0x38].copy_from_slice(&INDEX_MAGIC2.to_le_bytes());
    page[0x38..0x3A].copy_from_slice(&(data_page_indices.len() as u16).to_le_bytes());
    page[0x3A..0x3C].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());

    let mut offset = 0x3C;
    for &data_page_idx in data_page_indices {
        let entry = data_page_idx << 3;
        page[offset..offset + 4].copy_from_slice(&entry.to_le_bytes());
        offset += 4;
    }

    let remaining_space = (PAGE_SIZE as usize) - offset - 20;
    let empty_entries = remaining_space / 4;
    for _ in 0..empty_entries {
        page[offset..offset + 4].copy_from_slice(&EMPTY_INDEX_ENTRY.to_le_bytes());
        offset += 4;
    }

    page
}

/// Build an empty index page for extended format
#[allow(dead_code)]
fn build_empty_ext_index_page(page_type: ExtTableType, page_index: u32, sequence: u32) -> Vec<u8> {
    let mut page = vec![0u8; PAGE_SIZE as usize];

    page[0x04..0x08].copy_from_slice(&page_index.to_le_bytes());
    page[0x08..0x0C].copy_from_slice(&(page_type as u32).to_le_bytes());
    page[0x0C..0x10].copy_from_slice(&NULL_PAGE_MARKER.to_le_bytes());
    page[0x10..0x14].copy_from_slice(&sequence.to_le_bytes());
    page[0x1B] = PAGE_FLAGS_INDEX;

    page[0x20..0x22].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());
    page[0x22..0x24].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());
    page[0x24..0x26].copy_from_slice(&INDEX_MAGIC.to_le_bytes());
    page[0x26..0x28].copy_from_slice(&0u16.to_le_bytes());
    page[0x28..0x2C].copy_from_slice(&page_index.to_le_bytes());
    page[0x2C..0x30].copy_from_slice(&NULL_PAGE_MARKER.to_le_bytes());
    page[0x30..0x38].copy_from_slice(&INDEX_MAGIC2.to_le_bytes());
    page[0x38..0x3A].copy_from_slice(&0u16.to_le_bytes());
    page[0x3A..0x3C].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());

    let mut offset = 0x3C;
    let remaining_space = (PAGE_SIZE as usize) - offset - 20;
    let empty_entries = remaining_space / 4;
    for _ in 0..empty_entries {
        page[offset..offset + 4].copy_from_slice(&EMPTY_INDEX_ENTRY.to_le_bytes());
        offset += 4;
    }

    page
}
