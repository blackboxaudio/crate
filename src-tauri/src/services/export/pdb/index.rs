//! Index page building for PDB files
//!
//! Each table in a PDB file starts with an index page that points to the data pages.

#![allow(dead_code)]

use super::constants::*;
use super::tables::{ExtTableType, TableType};

/// Build an index page for a table
///
/// Index pages contain pointers to the data pages that follow.
pub fn build_index_page(
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

/// Build an empty index page for tables with no data
pub fn build_empty_index_page(page_type: TableType, page_index: u32, sequence: u32) -> Vec<u8> {
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

/// Build an index page for extended format (exportExt.pdb)
pub fn build_ext_index_page(
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
pub fn build_empty_ext_index_page(
    page_type: ExtTableType,
    page_index: u32,
    sequence: u32,
) -> Vec<u8> {
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
