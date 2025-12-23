//! Extended PDB writer for exportExt.pdb files
//!
//! The extended PDB file contains custom tags (My Tags) that can be
//! associated with tracks.

#![allow(dead_code)]

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::error::{CrateError, Result};

use super::constants::*;
use super::header::{write_ext_file_header, ExtTableLayout};
use super::index::{build_empty_ext_index_page, build_ext_index_page};
use super::tables::*;

/// Page builder for extended format (uses ExtTableType)
struct ExtPageBuilder {
    page_type: ExtTableType,
    heap_data: Vec<u8>,
    row_groups: Vec<super::page::RowGroup>,
    current_group: super::page::RowGroup,
    total_rows: u16,
}

impl ExtPageBuilder {
    fn new(page_type: ExtTableType) -> Self {
        Self {
            page_type,
            heap_data: Vec::new(),
            row_groups: Vec::new(),
            current_group: super::page::RowGroup::new(),
            total_rows: 0,
        }
    }

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
            self.current_group = super::page::RowGroup::new();
        }

        self.current_group.add_row(offset);
        self.total_rows += 1;
        self.heap_data.extend_from_slice(row_data);

        true
    }

    fn build(mut self, page_index: u32, next_page: u32, sequence: u32) -> Vec<u8> {
        let mut page = vec![0u8; PAGE_SIZE as usize];

        if !self.current_group.is_empty() {
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
            page[0x22..0x24]
                .copy_from_slice(&((self.total_rows.saturating_sub(1)).min(0x1FFF)).to_le_bytes());
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

/// Extended PDB writer for exportExt.pdb
///
/// This writer handles custom tags (My Tags) that can be associated with tracks.
pub struct ExtPdbWriter {
    tags: Vec<PdbTag>,
    track_tags: Vec<PdbTrackTag>,
    next_tag_id: u32,
    // Maps: category_name -> tag_id
    category_ids: HashMap<String, u32>,
    // Maps: (category_name, tag_name) -> tag_id
    tag_ids: HashMap<(String, String), u32>,
}

impl ExtPdbWriter {
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
    ///
    /// If the category doesn't exist, it will be created automatically.
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
                table_layouts.push(ExtTableLayout::empty(*table_type, current_page));
                current_page += 1;
            } else {
                let index_page = current_page;
                let first_data_page = current_page + 1;
                let last_data_page = first_data_page + (data_pages.len() as u32) - 1;

                table_layouts.push(ExtTableLayout::with_data(
                    *table_type,
                    index_page,
                    first_data_page,
                    last_data_page,
                ));
                current_page = last_data_page + 1;
            }
        }

        let total_pages = current_page;

        // Write file header
        write_ext_file_header(&mut writer, total_pages, &table_layouts)
            .map_err(|e| CrateError::Device(format!("Failed to write header: {e}")))?;

        // Write table pages
        let sequence = 1u32;
        for (i, (table_type, data_pages)) in table_data.iter().enumerate() {
            let layout = &table_layouts[i];

            if data_pages.is_empty() {
                let index_page =
                    build_empty_ext_index_page(*table_type, layout.index_page, sequence);
                writer
                    .write_all(&index_page)
                    .map_err(|e| CrateError::Device(format!("Failed to write page: {e}")))?;
            } else {
                let first_data = layout.first_data_page.unwrap();
                let last_data = layout.last_data_page.unwrap();
                let data_page_indices: Vec<u32> = (first_data..=last_data).collect();

                let index_page = build_ext_index_page(
                    *table_type,
                    layout.index_page,
                    first_data,
                    &data_page_indices,
                    sequence,
                );
                writer
                    .write_all(&index_page)
                    .map_err(|e| CrateError::Device(format!("Failed to write index page: {e}")))?;

                for (j, page_data) in data_pages.iter().enumerate() {
                    let page_idx = first_data + j as u32;
                    let next_page = if page_idx < last_data {
                        page_idx + 1
                    } else {
                        NULL_PAGE_MARKER
                    };

                    let mut page = page_data.clone();
                    page[0x04..0x08].copy_from_slice(&page_idx.to_le_bytes());
                    page[0x0C..0x10].copy_from_slice(&next_page.to_le_bytes());

                    writer.write_all(&page).map_err(|e| {
                        CrateError::Device(format!("Failed to write data page: {e}"))
                    })?;
                }
            }
        }

        writer
            .flush()
            .map_err(|e| CrateError::Device(format!("Failed to flush exportExt.pdb: {e}")))?;

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

impl Default for ExtPdbWriter {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export as RekordboxExtPdbWriter for backwards compatibility
pub type RekordboxExtPdbWriter = ExtPdbWriter;
