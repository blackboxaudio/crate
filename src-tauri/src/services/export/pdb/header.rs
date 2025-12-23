//! File header building for PDB files

#![allow(dead_code)]

use std::io::Write;

use super::constants::*;
use super::tables::{ExtTableType, TableType};

/// Layout information for a table in the PDB file
#[derive(Debug, Clone)]
pub struct TableLayout {
    /// Table type
    pub table_type: TableType,
    /// Index page number
    pub index_page: u32,
    /// First data page number (None if empty)
    pub first_data_page: Option<u32>,
    /// Last data page number (None if empty)
    pub last_data_page: Option<u32>,
    /// Next available page for this table
    pub empty_candidate: u32,
}

impl TableLayout {
    /// Create a layout for an empty table
    pub fn empty(table_type: TableType, index_page: u32) -> Self {
        Self {
            table_type,
            index_page,
            first_data_page: None,
            last_data_page: None,
            empty_candidate: index_page,
        }
    }

    /// Create a layout for a table with data
    pub fn with_data(
        table_type: TableType,
        index_page: u32,
        first_data_page: u32,
        last_data_page: u32,
    ) -> Self {
        Self {
            table_type,
            index_page,
            first_data_page: Some(first_data_page),
            last_data_page: Some(last_data_page),
            empty_candidate: last_data_page + 1,
        }
    }

    /// Get the last page number (data or index)
    pub fn last_page(&self) -> u32 {
        self.last_data_page.unwrap_or(self.index_page)
    }
}

/// Layout information for an extended table
#[derive(Debug, Clone)]
pub struct ExtTableLayout {
    pub table_type: ExtTableType,
    pub index_page: u32,
    pub first_data_page: Option<u32>,
    pub last_data_page: Option<u32>,
    pub empty_candidate: u32,
}

impl ExtTableLayout {
    pub fn empty(table_type: ExtTableType, index_page: u32) -> Self {
        Self {
            table_type,
            index_page,
            first_data_page: None,
            last_data_page: None,
            empty_candidate: index_page,
        }
    }

    pub fn with_data(
        table_type: ExtTableType,
        index_page: u32,
        first_data_page: u32,
        last_data_page: u32,
    ) -> Self {
        Self {
            table_type,
            index_page,
            first_data_page: Some(first_data_page),
            last_data_page: Some(last_data_page),
            empty_candidate: last_data_page + 1,
        }
    }

    pub fn last_page(&self) -> u32 {
        self.last_data_page.unwrap_or(self.index_page)
    }
}

/// Write the file header (page 0) for a PDB file
///
/// The header contains:
/// - File metadata (page size, number of tables, etc.)
/// - Table descriptors (16 bytes each)
pub fn write_file_header<W: Write>(
    writer: &mut W,
    total_pages: u32,
    table_layouts: &[TableLayout],
) -> std::io::Result<()> {
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
        // Last page
        page[offset + 12..offset + 16].copy_from_slice(&layout.last_page().to_le_bytes());
        offset += TABLE_DESCRIPTOR_SIZE;
    }

    writer.write_all(&page)
}

/// Write the file header for an extended PDB file
pub fn write_ext_file_header<W: Write>(
    writer: &mut W,
    total_pages: u32,
    table_layouts: &[ExtTableLayout],
) -> std::io::Result<()> {
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
        page[offset + 12..offset + 16].copy_from_slice(&layout.last_page().to_le_bytes());
        offset += TABLE_DESCRIPTOR_SIZE;
    }

    writer.write_all(&page)
}
