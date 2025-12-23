//! PDB writer for export.pdb files
//!
//! This module provides the main `PdbWriter` struct for building and writing
//! Rekordbox-compatible PDB files.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::error::{CrateError, Result};
use crate::models::{Playlist, Track};

use super::constants::*;
use super::header::{write_file_header, TableLayout};
use super::index::{build_empty_index_page, build_index_page};
use super::merger::PdbMerger;
use super::page::PageBuilder;
use super::reader::ParsedPdb;
use super::tables::*;

/// Rekordbox PDB file writer
///
/// Use this to build a PDB file by adding tracks and playlists,
/// then write the result to disk.
pub struct PdbWriter {
    // Optional merger for preserving IDs from existing PDB
    merger: Option<PdbMerger>,

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

impl PdbWriter {
    /// Create a new empty PDB writer
    pub fn new() -> Self {
        Self {
            merger: None,
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
    ///
    /// Parses the existing PDB file and uses a merger to preserve
    /// track and entity IDs where possible.
    pub fn from_existing(data: &[u8]) -> Result<Self> {
        match ParsedPdb::from_bytes(data) {
            Ok(parsed) => {
                let merger = PdbMerger::from_parsed(parsed);
                log::info!(
                    "Parsed existing PDB with {} tracks",
                    merger.existing_track_count()
                );
                Ok(Self {
                    merger: Some(merger),
                    artists: HashMap::new(),
                    albums: HashMap::new(),
                    genres: HashMap::new(),
                    keys: HashMap::new(),
                    tracks: Vec::new(),
                    playlist_nodes: Vec::new(),
                    playlist_entries: Vec::new(),
                    next_track_id: 1, // Will be managed by merger
                    next_artist_id: 1,
                    next_album_id: 1,
                    next_genre_id: 1,
                    next_key_id: 1,
                    next_playlist_id: 1,
                    next_entry_index: 1,
                    playlist_id_map: HashMap::new(),
                })
            }
            Err(e) => {
                log::warn!("Failed to parse existing PDB: {e}, starting fresh");
                Ok(Self::new())
            }
        }
    }

    /// Add a track and return its PDB ID
    pub fn add_track(&mut self, track: &Track, usb_path: &str, anlz_path: &str) -> u32 {
        // Build the USB file path (with leading /)
        let file_path = format!("/Contents/{usb_path}");

        // Get track ID - use merger if available for ID preservation
        let id = if let Some(ref mut merger) = self.merger {
            merger.get_or_allocate_track_id(&file_path)
        } else {
            let id = self.next_track_id;
            self.next_track_id += 1;
            id
        };

        // Get or create artist - use merger if available
        let artist_name = track
            .artist
            .clone()
            .unwrap_or_else(|| "Unknown Artist".to_string());
        let artist_id = if let Some(ref mut merger) = self.merger {
            let id = merger.get_or_create_artist(&artist_name);
            self.artists.insert(artist_name.clone(), id);
            id
        } else {
            *self.artists.entry(artist_name.clone()).or_insert_with(|| {
                let id = self.next_artist_id;
                self.next_artist_id += 1;
                id
            })
        };

        // Get or create album (keyed by name + artist_id for proper dedup)
        let album_name = track
            .album
            .clone()
            .unwrap_or_else(|| "Unknown Album".to_string());
        let album_id = if let Some(ref mut merger) = self.merger {
            let id = merger.get_or_create_album(&album_name, artist_id);
            self.albums.insert((album_name.clone(), artist_id), id);
            id
        } else {
            *self
                .albums
                .entry((album_name.clone(), artist_id))
                .or_insert_with(|| {
                    let id = self.next_album_id;
                    self.next_album_id += 1;
                    id
                })
        };

        // Get or create genre - use merger if available
        let genre_name = track.genre.clone().unwrap_or_default();
        let genre_id = if genre_name.is_empty() {
            0
        } else if let Some(ref mut merger) = self.merger {
            let id = merger.get_or_create_genre(&genre_name);
            self.genres.insert(genre_name, id);
            id
        } else {
            *self.genres.entry(genre_name).or_insert_with(|| {
                let id = self.next_genre_id;
                self.next_genre_id += 1;
                id
            })
        };

        // Get or create key - use merger if available
        let key_name = track.key.clone().unwrap_or_default();
        let key_id = if key_name.is_empty() {
            0
        } else if let Some(ref mut merger) = self.merger {
            let id = merger.get_or_create_key(&key_name);
            self.keys.insert(key_name, id);
            id
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
        let date_added = track.date_added.split('T').next().unwrap_or("").to_string();

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
                table_layouts.push(TableLayout::empty(*table_type, current_page));
                current_page += 1;
            } else {
                // Table with data: index page + data pages
                let index_page = current_page;
                let first_data_page = current_page + 1;
                let last_data_page = first_data_page + (data_pages.len() as u32) - 1;

                table_layouts.push(TableLayout::with_data(
                    *table_type,
                    index_page,
                    first_data_page,
                    last_data_page,
                ));
                current_page = last_data_page + 1;
            }
        }

        let total_pages = current_page;

        // Write file header (page 0)
        write_file_header(&mut writer, total_pages, &table_layouts)
            .map_err(|e| CrateError::Device(format!("Failed to write header: {e}")))?;

        // Write table pages
        let sequence = 1u32;
        for (i, (table_type, data_pages)) in table_data.iter().enumerate() {
            let layout = &table_layouts[i];

            if data_pages.is_empty() {
                // Empty table: just write empty index page
                let index_page = build_empty_index_page(*table_type, layout.index_page, sequence);
                writer
                    .write_all(&index_page)
                    .map_err(|e| CrateError::Device(format!("Failed to write page: {e}")))?;
            } else {
                // Table with data: write index page + data pages
                let first_data = layout.first_data_page.unwrap();
                let last_data = layout.last_data_page.unwrap();
                let data_page_indices: Vec<u32> = (first_data..=last_data).collect();

                let index_page = build_index_page(
                    *table_type,
                    layout.index_page,
                    first_data,
                    &data_page_indices,
                    sequence,
                );
                writer
                    .write_all(&index_page)
                    .map_err(|e| CrateError::Device(format!("Failed to write index page: {e}")))?;

                // Write data pages
                for (j, page_data) in data_pages.iter().enumerate() {
                    let page_idx = first_data + j as u32;
                    let next_page = if page_idx < last_data {
                        page_idx + 1
                    } else {
                        NULL_PAGE_MARKER
                    };

                    // Update page indices in the built page
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
            .map_err(|e| CrateError::Device(format!("Failed to flush PDB file: {e}")))?;

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
                TableType::Menu => self.build_menu_pages()?,
                // Empty tables
                _ => Vec::new(),
            };
            result.push((table_type, pages));
        }

        Ok(result)
    }

    fn build_track_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Tracks);

        for (idx, track) in self.tracks.iter().enumerate() {
            let row_data = build_track_row(track, idx as u16);

            if !builder.add_row(&row_data) {
                pages.push(builder.build(0, 0, 1));
                builder = PageBuilder::new(TableType::Tracks);
                builder.add_row(&row_data);
            }
        }

        if builder.total_rows() > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

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

        if builder.total_rows() > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

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

        if builder.total_rows() > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

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

        if builder.total_rows() > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

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

        if builder.total_rows() > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

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

        if builder.total_rows() > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

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

        if builder.total_rows() > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

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

        if builder.total_rows() > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

    fn build_column_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Columns);

        for &(id, content_ptr, name) in COLUMN_DEFS {
            let row_data = build_column_row(id, content_ptr, name);

            if !builder.add_row(&row_data) {
                pages.push(builder.build(0, 0, 1));
                builder = PageBuilder::new(TableType::Columns);
                builder.add_row(&row_data);
            }
        }

        if builder.total_rows() > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }

    fn build_menu_pages(&self) -> Result<Vec<Vec<u8>>> {
        let mut pages = Vec::new();
        let mut builder = PageBuilder::new(TableType::Menu);

        for &(category_id, content_ptr, unknown, visibility, sort_order) in STANDARD_MENUS {
            let row_data =
                build_menu_row(category_id, content_ptr, unknown, visibility, sort_order);

            if !builder.add_row(&row_data) {
                pages.push(builder.build(0, 0, 1));
                builder = PageBuilder::new(TableType::Menu);
                builder.add_row(&row_data);
            }
        }

        if builder.total_rows() > 0 {
            pages.push(builder.build(0, 0, 1));
        }

        Ok(pages)
    }
}

impl Default for PdbWriter {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export as RekordboxPdbWriter for backwards compatibility
pub type RekordboxPdbWriter = PdbWriter;
