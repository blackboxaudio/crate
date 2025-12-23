//! Core binary types for PDB files
//!
//! PDB files use little-endian byte order throughout.

#![allow(dead_code)]

use binrw::{BinRead, BinWrite};

use super::constants::*;

/// Page header structure (32 bytes at 0x00-0x1F)
///
/// This header appears at the start of every page in the PDB file.
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
pub struct PageHeader {
    /// Magic bytes (always 0)
    pub magic: u32,
    /// Index of this page within the file
    pub page_index: u32,
    /// Table type (see TableType enum)
    pub page_type: u32,
    /// Index of the next page in the chain, or NULL_PAGE_MARKER if last
    pub next_page: u32,
    /// Sequence/version number
    pub sequence: u32,
    /// Unknown (always 0)
    pub unknown2: u32,
    /// Packed row counts (3 bytes)
    /// bits 0-12: num_row_groups (13 bits)
    /// bits 13-23: num_rows (11 bits)
    pub packed_row_counts: [u8; 3],
    /// Page flags (0x64 for index, 0x24 for data)
    pub page_flags: u8,
    /// Free space remaining in page heap
    pub free_size: u16,
    /// Used space in page heap
    pub used_size: u16,
}

impl PageHeader {
    /// Create a new page header for a data page
    pub fn new_data_page(page_index: u32, page_type: u32, next_page: u32, sequence: u32) -> Self {
        Self {
            magic: 0,
            page_index,
            page_type,
            next_page,
            sequence,
            unknown2: 0,
            packed_row_counts: [0, 0, 0],
            page_flags: PAGE_FLAGS_DATA,
            free_size: 0,
            used_size: 0,
        }
    }

    /// Create a new page header for an index page
    pub fn new_index_page(page_index: u32, page_type: u32, next_page: u32, sequence: u32) -> Self {
        Self {
            magic: 0,
            page_index,
            page_type,
            next_page,
            sequence,
            unknown2: 0,
            packed_row_counts: [0, 0, 0],
            page_flags: PAGE_FLAGS_INDEX,
            free_size: 0,
            used_size: 0,
        }
    }

    /// Pack row counts into the 3-byte field
    /// bits 0-12: num_row_groups (13 bits)
    /// bits 13-23: num_rows (11 bits)
    pub fn set_packed_row_counts(&mut self, num_row_groups: u32, num_rows: u32) {
        let packed = (num_row_groups & 0x1FFF) | ((num_rows & 0x7FF) << 13);
        self.packed_row_counts[0] = packed as u8;
        self.packed_row_counts[1] = (packed >> 8) as u8;
        self.packed_row_counts[2] = (packed >> 16) as u8;
    }

    /// Get the number of row groups from packed field
    pub fn num_row_groups(&self) -> u32 {
        let packed = u32::from_le_bytes([
            self.packed_row_counts[0],
            self.packed_row_counts[1],
            self.packed_row_counts[2],
            0,
        ]);
        packed & 0x1FFF
    }

    /// Get the number of rows from packed field
    pub fn num_rows(&self) -> u32 {
        let packed = u32::from_le_bytes([
            self.packed_row_counts[0],
            self.packed_row_counts[1],
            self.packed_row_counts[2],
            0,
        ]);
        (packed >> 13) & 0x7FF
    }
}

/// Data page header (8 bytes at 0x20-0x27)
///
/// Follows the page header on data pages.
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
pub struct DataPageHeader {
    /// Usually num_rows for most tables, or 0x1FFF if empty
    pub unknown5: u16,
    /// Usually num_rows - 1 for most tables, 0 for Menu/Columns, or 0x1FFF if empty
    pub unknown_not: u16,
    /// Reserved (always 0)
    pub reserved1: u16,
    /// Reserved (always 0)
    pub reserved2: u16,
}

impl DataPageHeader {
    /// Create an empty data page header
    pub fn empty() -> Self {
        Self {
            unknown5: INDEX_EMPTY_MARKER,
            unknown_not: INDEX_EMPTY_MARKER,
            reserved1: 0,
            reserved2: 0,
        }
    }

    /// Create a data page header for normal tables
    pub fn for_table(num_rows: u16) -> Self {
        Self {
            unknown5: num_rows,
            unknown_not: num_rows.saturating_sub(1),
            reserved1: 0,
            reserved2: 0,
        }
    }

    /// Create a data page header for Menu/Columns tables
    pub fn for_menu_table(num_rows: u16) -> Self {
        Self {
            unknown5: num_rows,
            unknown_not: 0,
            reserved1: 0,
            reserved2: 0,
        }
    }
}

/// Index page header (starts at 0x20)
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
pub struct IndexPageHeader {
    /// Unknown (0x1FFF if empty)
    pub unknown_a: u16,
    /// Unknown (0x1FFF if empty)
    pub unknown_b: u16,
    /// Magic (0x03EC)
    pub magic: u16,
    /// Next offset / number of entries
    pub next_offset: u16,
    /// Page index (redundant)
    pub page_index: u32,
    /// Next page (redundant)
    pub next_page: u32,
    /// Magic2 (0x0000_0000_03FF_FFFF)
    pub magic2: u64,
    /// Number of entries
    pub num_entries: u16,
    /// First empty slot (0x1FFF if none)
    pub first_empty: u16,
}

impl IndexPageHeader {
    /// Create a new index page header
    pub fn new(page_index: u32, next_page: u32, num_entries: u16) -> Self {
        Self {
            unknown_a: INDEX_EMPTY_MARKER,
            unknown_b: INDEX_EMPTY_MARKER,
            magic: INDEX_MAGIC,
            next_offset: num_entries,
            page_index,
            next_page,
            magic2: INDEX_MAGIC2,
            num_entries,
            first_empty: INDEX_EMPTY_MARKER,
        }
    }

    /// Create an empty index page header
    pub fn empty(page_index: u32) -> Self {
        Self::new(page_index, NULL_PAGE_MARKER, 0)
    }
}

/// Table descriptor in the file header (16 bytes each)
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
pub struct TableDescriptor {
    /// Table type (see TableType enum)
    pub table_type: u32,
    /// Next available page for this table
    pub empty_candidate: u32,
    /// First page of this table (index page)
    pub first_page: u32,
    /// Last page of this table
    pub last_page: u32,
}

impl TableDescriptor {
    /// Create a new table descriptor
    pub fn new(table_type: u32, first_page: u32, last_page: u32, empty_candidate: u32) -> Self {
        Self {
            table_type,
            empty_candidate,
            first_page,
            last_page,
        }
    }

    /// Create a descriptor for an empty table
    pub fn empty(table_type: u32, index_page: u32) -> Self {
        Self {
            table_type,
            empty_candidate: index_page,
            first_page: index_page,
            last_page: index_page,
        }
    }
}

/// File header (first page, offset 0x00-0x1B)
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
pub struct FileHeader {
    /// Magic (always 0)
    pub magic: u32,
    /// Page size (always 4096)
    pub page_size: u32,
    /// Number of tables
    pub num_tables: u32,
    /// Next unused page index
    pub next_unused_page: u32,
    /// Unknown (always 0)
    pub unknown: u32,
    /// Sequence number
    pub sequence: u32,
    /// Gap/padding
    pub gap: u32,
}

impl FileHeader {
    /// Create a new file header
    pub fn new(num_tables: u32, next_unused_page: u32) -> Self {
        Self {
            magic: 0,
            page_size: PAGE_SIZE,
            num_tables,
            next_unused_page,
            unknown: 0,
            sequence: 1,
            gap: 0,
        }
    }
}
