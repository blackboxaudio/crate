//! Page building for PDB files
//!
//! PDB pages are 4096 bytes and contain:
//! - Page header (32 bytes at 0x00-0x1F)
//! - Data page header (8 bytes at 0x20-0x27)
//! - Heap data (rows, starting at 0x28)
//! - Row groups (at the end of the page, growing backwards)

#![allow(dead_code)]

use super::constants::*;
use super::tables::TableType;

/// A row group contains up to 16 row offsets and presence flags
///
/// Row groups are stored at the end of the page, growing backwards.
/// Each group can hold up to 16 rows.
#[derive(Debug, Clone)]
pub struct RowGroup {
    /// Offsets to rows within the page heap (relative to heap start at 0x28)
    row_offsets: [u16; MAX_ROWS_PER_GROUP],
    /// Bitmask indicating which rows are present
    presence_flags: u16,
    /// Number of rows in this group
    num_rows: usize,
}

impl RowGroup {
    /// Create a new empty row group
    pub fn new() -> Self {
        Self {
            row_offsets: [0; MAX_ROWS_PER_GROUP],
            presence_flags: 0,
            num_rows: 0,
        }
    }

    /// Add a row offset to the group
    ///
    /// Returns true if successful, false if the group is full.
    pub fn add_row(&mut self, offset: u16) -> bool {
        if self.num_rows >= MAX_ROWS_PER_GROUP {
            return false;
        }
        // Store offset at reverse index (rekordbox stores from end)
        // Index 15 = bit 0, Index 14 = bit 1, etc.
        let reverse_idx = MAX_ROWS_PER_GROUP - 1 - self.num_rows;
        self.row_offsets[reverse_idx] = offset;
        // Set presence bit (bit 0 = first row, bit 1 = second row, etc.)
        self.presence_flags |= 1 << self.num_rows;
        self.num_rows += 1;
        true
    }

    /// Check if the group is full
    pub fn is_full(&self) -> bool {
        self.num_rows >= MAX_ROWS_PER_GROUP
    }

    /// Get the number of rows in this group
    pub fn len(&self) -> usize {
        self.num_rows
    }

    /// Check if the group is empty
    pub fn is_empty(&self) -> bool {
        self.num_rows == 0
    }

    /// Calculate the binary size of this row group
    ///
    /// Format: [row_offsets...] [presence_flags: u16] [unknown: u16]
    pub fn binary_size(&self) -> usize {
        let leading_zeros = self.presence_flags.leading_zeros() as usize;
        let num_offsets = 16 - leading_zeros;
        num_offsets * 2 + 4 // offsets + presence_flags + unknown
    }

    /// Write the row group to a buffer at the specified end position
    ///
    /// Row groups are written backwards from the end of the page.
    pub fn write_to(&self, buffer: &mut [u8], end_position: usize) {
        let leading_zeros = self.presence_flags.leading_zeros() as usize;
        let num_offsets = 16 - leading_zeros;
        let group_size = num_offsets * 2 + 4;
        let start_pos = end_position - group_size;

        let mut pos = start_pos;

        // Write offsets from index leading_zeros to 15
        for i in 0..num_offsets {
            let idx = leading_zeros + i;
            buffer[pos..pos + 2].copy_from_slice(&self.row_offsets[idx].to_le_bytes());
            pos += 2;
        }

        // Write presence flags
        buffer[pos..pos + 2].copy_from_slice(&self.presence_flags.to_le_bytes());
        pos += 2;

        // Write unknown field (usually same as presence_flags)
        buffer[pos..pos + 2].copy_from_slice(&self.presence_flags.to_le_bytes());
    }
}

impl Default for RowGroup {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds a data page with proper layout
pub struct PageBuilder {
    /// Page type (table type)
    page_type: TableType,
    /// Row data buffer (heap)
    heap_data: Vec<u8>,
    /// Completed row groups
    row_groups: Vec<RowGroup>,
    /// Current row group being filled
    current_group: RowGroup,
    /// Total row count on this page
    total_rows: u16,
}

impl PageBuilder {
    /// Create a new page builder for the specified table type
    pub fn new(page_type: TableType) -> Self {
        Self {
            page_type,
            heap_data: Vec::new(),
            row_groups: Vec::new(),
            current_group: RowGroup::new(),
            total_rows: 0,
        }
    }

    /// Get the total number of rows in this page
    pub fn total_rows(&self) -> u16 {
        self.total_rows
    }

    /// Calculate the size needed for all row groups
    fn row_groups_size(&self) -> usize {
        let mut size = 0;
        for group in &self.row_groups {
            size += group.binary_size();
        }
        if !self.current_group.is_empty() {
            size += self.current_group.binary_size();
        }
        size
    }

    /// Calculate available space for new row data
    pub fn available_space(&self) -> usize {
        let total_page = PAGE_SIZE as usize;
        let header_space = HEAP_START_OFFSET;
        let heap_used = self.heap_data.len();
        let row_groups_space = self.row_groups_size();

        // Need to account for potential new row group entry (worst case: 36 bytes)
        let potential_index_growth = if self.current_group.is_full() { 36 } else { 2 };

        total_page
            .saturating_sub(header_space)
            .saturating_sub(heap_used)
            .saturating_sub(row_groups_space)
            .saturating_sub(potential_index_growth)
    }

    /// Add a row to the page
    ///
    /// Returns true if successful, false if the page is full.
    pub fn add_row(&mut self, row_data: &[u8]) -> bool {
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
    ///
    /// The page_index and next_page will be set in the header.
    pub fn build(mut self, page_index: u32, next_page: u32, sequence: u32) -> Vec<u8> {
        let mut page = vec![0u8; PAGE_SIZE as usize];

        // Finalize current group if it has rows
        if !self.current_group.is_empty() {
            self.row_groups.push(self.current_group.clone());
        }

        // Calculate sizes
        let used_size = self.heap_data.len() as u16;
        let row_groups_total_size: usize = self.row_groups.iter().map(|g| g.binary_size()).sum();
        let heap_capacity = (PAGE_SIZE as usize) - HEAP_START_OFFSET;
        let free_size = (heap_capacity - self.heap_data.len() - row_groups_total_size) as u16;

        // PackedRowCounts format:
        // bits 0-12 = num_row_groups (13 bits), bits 13-23 = num_rows (11 bits)
        let num_row_groups = self.row_groups.len() as u32;
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
        if self.total_rows == 0 {
            page[0x20..0x22].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());
            page[0x22..0x24].copy_from_slice(&INDEX_EMPTY_MARKER.to_le_bytes());
        } else if self.page_type.uses_menu_header() {
            // Menu/Columns: unknown5=num_rows, unknown_not=0
            page[0x20..0x22].copy_from_slice(&self.total_rows.to_le_bytes());
            page[0x22..0x24].copy_from_slice(&0u16.to_le_bytes());
        } else {
            // Other tables: unknown5=num_rows, unknown_not=num_rows-1
            page[0x20..0x22].copy_from_slice(&self.total_rows.to_le_bytes());
            page[0x22..0x24].copy_from_slice(&self.total_rows.saturating_sub(1).to_le_bytes());
        }
        // 0x24-0x27: Remain zeros (already initialized)

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
