//! Database operations for Device Library Plus export.
//!
//! Provides DeviceLibraryPlusWriter for creating and populating encrypted
//! Device Library Plus databases used by newer Pioneer DJ hardware.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;

use crate::error::{CrateError, Result};

use super::encryption::get_sqlcipher_key;
use super::models::*;
use super::schema::create_all_tables;

/// Writer for creating Device Library Plus (SQLCipher encrypted) databases.
///
/// This writer creates exportLibrary.db files compatible with Pioneer DJ hardware
/// like OPUS-QUAD, OMNIS-DUO, and XDJ-AZ.
pub struct DeviceLibraryPlusWriter {
    conn: Connection,
    // Deduplication caches
    artists: HashMap<String, i64>,
    albums: HashMap<String, i64>,
    genres: HashMap<String, i64>,
    labels: HashMap<String, i64>,
    keys: HashMap<String, i64>,
    colors: HashMap<String, i64>,
    images: HashMap<String, i64>,
    // ID counters for playlists
    playlist_id_map: HashMap<String, i64>,
    next_content_id: i64,
}

impl DeviceLibraryPlusWriter {
    /// Create a new encrypted Device Library Plus database at the given path.
    ///
    /// If a file already exists at the path, it will be overwritten.
    pub fn create(path: &Path) -> Result<Self> {
        // Remove existing file if present
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| {
                CrateError::Export(format!("Failed to remove existing database: {e}"))
            })?;
        }

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CrateError::Export(format!("Failed to create database directory: {e}"))
            })?;
        }

        // Get SQLCipher key
        let key = get_sqlcipher_key()?;

        // Open connection with SQLCipher
        let conn = Connection::open(path)?;

        // Set the encryption key
        conn.pragma_update(None, "key", &key)?;

        // Create all tables
        create_all_tables(&conn)?;

        Ok(Self {
            conn,
            artists: HashMap::new(),
            albums: HashMap::new(),
            genres: HashMap::new(),
            labels: HashMap::new(),
            keys: HashMap::new(),
            colors: HashMap::new(),
            images: HashMap::new(),
            playlist_id_map: HashMap::new(),
            next_content_id: 1,
        })
    }

    // ========================================================================
    // Artist operations
    // ========================================================================

    /// Get or create an artist by name. Returns the artist_id.
    pub fn get_or_create_artist(&mut self, name: &str) -> Result<i64> {
        if let Some(&id) = self.artists.get(name) {
            return Ok(id);
        }

        self.conn.execute(
            "INSERT INTO artist (name, nameForSearch) VALUES (?1, ?2)",
            rusqlite::params![name, name.to_lowercase()],
        )?;

        let id = self.conn.last_insert_rowid();
        self.artists.insert(name.to_string(), id);
        Ok(id)
    }

    // ========================================================================
    // Album operations
    // ========================================================================

    /// Get or create an album by name. Returns the album_id.
    pub fn get_or_create_album(&mut self, name: &str, artist_id: Option<i64>) -> Result<i64> {
        if let Some(&id) = self.albums.get(name) {
            return Ok(id);
        }

        self.conn.execute(
            "INSERT INTO album (name, artist_id, nameForSearch) VALUES (?1, ?2, ?3)",
            rusqlite::params![name, artist_id, name.to_lowercase()],
        )?;

        let id = self.conn.last_insert_rowid();
        self.albums.insert(name.to_string(), id);
        Ok(id)
    }

    // ========================================================================
    // Genre operations
    // ========================================================================

    /// Get or create a genre by name. Returns the genre_id.
    pub fn get_or_create_genre(&mut self, name: &str) -> Result<i64> {
        if let Some(&id) = self.genres.get(name) {
            return Ok(id);
        }

        self.conn.execute(
            "INSERT INTO genre (name) VALUES (?1)",
            rusqlite::params![name],
        )?;

        let id = self.conn.last_insert_rowid();
        self.genres.insert(name.to_string(), id);
        Ok(id)
    }

    // ========================================================================
    // Label operations
    // ========================================================================

    /// Get or create a label by name. Returns the label_id.
    pub fn get_or_create_label(&mut self, name: &str) -> Result<i64> {
        if let Some(&id) = self.labels.get(name) {
            return Ok(id);
        }

        self.conn.execute(
            "INSERT INTO label (name) VALUES (?1)",
            rusqlite::params![name],
        )?;

        let id = self.conn.last_insert_rowid();
        self.labels.insert(name.to_string(), id);
        Ok(id)
    }

    // ========================================================================
    // Key operations
    // ========================================================================

    /// Get or create a musical key by name. Returns the key_id.
    pub fn get_or_create_key(&mut self, name: &str) -> Result<i64> {
        if let Some(&id) = self.keys.get(name) {
            return Ok(id);
        }

        self.conn.execute(
            "INSERT INTO key (name) VALUES (?1)",
            rusqlite::params![name],
        )?;

        let id = self.conn.last_insert_rowid();
        self.keys.insert(name.to_string(), id);
        Ok(id)
    }

    // ========================================================================
    // Color operations
    // ========================================================================

    /// Get or create a color by name. Returns the color_id.
    pub fn get_or_create_color(&mut self, name: &str) -> Result<i64> {
        if let Some(&id) = self.colors.get(name) {
            return Ok(id);
        }

        self.conn.execute(
            "INSERT INTO color (name) VALUES (?1)",
            rusqlite::params![name],
        )?;

        let id = self.conn.last_insert_rowid();
        self.colors.insert(name.to_string(), id);
        Ok(id)
    }

    // ========================================================================
    // Image operations
    // ========================================================================

    /// Get or create an image by path. Returns the image_id.
    pub fn get_or_create_image(&mut self, path: &str) -> Result<i64> {
        if let Some(&id) = self.images.get(path) {
            return Ok(id);
        }

        self.conn.execute(
            "INSERT INTO image (path) VALUES (?1)",
            rusqlite::params![path],
        )?;

        let id = self.conn.last_insert_rowid();
        self.images.insert(path.to_string(), id);
        Ok(id)
    }

    // ========================================================================
    // Content operations
    // ========================================================================

    /// Add a content (track) entry. Returns the content_id.
    pub fn add_content(&mut self, content: &Content) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO content (
                title, titleForSearch, subtitle, bpmx100, length, trackNo, discNo,
                artist_id_artist, artist_id_remixer, artist_id_originalArtist,
                artist_id_composer, artist_id_lyricist, album_id, genre_id, label_id,
                key_id, color_id, image_id, djComment, rating, releaseYear, releaseDate,
                dateCreated, dateAdded, path, fileName, fileSize, fileType, bitrate,
                bitDepth, samplingRate, isrc, isHotCueAutoLoadOn, isKuvoDeliverStatusOn,
                kuvoDeliveryComment, masterDbId, masterContentId, analysisDataFilePath,
                analysedBits, contentLink, hasModified, cueUpdateCount,
                analysisDataUpdateCount, informationUpdateCount
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28,
                ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41,
                ?42, ?43, ?44
            )
            "#,
            rusqlite::params![
                content.title,
                content.title_for_search,
                content.subtitle,
                content.bpmx100,
                content.length,
                content.track_no,
                content.disc_no,
                content.artist_id_artist,
                content.artist_id_remixer,
                content.artist_id_original_artist,
                content.artist_id_composer,
                content.artist_id_lyricist,
                content.album_id,
                content.genre_id,
                content.label_id,
                content.key_id,
                content.color_id,
                content.image_id,
                content.dj_comment,
                content.rating,
                content.release_year,
                content.release_date,
                content.date_created,
                content.date_added,
                content.path,
                content.file_name,
                content.file_size,
                content.file_type,
                content.bitrate,
                content.bit_depth,
                content.sampling_rate,
                content.isrc,
                content.is_hot_cue_auto_load_on,
                content.is_kuvo_deliver_status_on,
                content.kuvo_delivery_comment,
                content.master_db_id,
                content.master_content_id,
                content.analysis_data_file_path,
                content.analysed_bits,
                content.content_link,
                content.has_modified,
                content.cue_update_count,
                content.analysis_data_update_count,
                content.information_update_count,
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        self.next_content_id = id + 1;
        Ok(id)
    }

    // ========================================================================
    // Cue operations
    // ========================================================================

    /// Add a cue point. Returns the cue_id.
    pub fn add_cue(&mut self, cue: &Cue) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO cue (
                content_id, kind, colorTableIndex, cueComment, isActiveLoop,
                beatLoopNumerator, beatLoopDenominator, inUsec, outUsec,
                in150FramePerSec, out150FramePerSec, inMpegFrameNumber,
                outMpegFrameNumber, inMpegAbs, outMpegAbs,
                inDecodingStartFramePosition, outDecodingStartFramePosition,
                inFileOffsetInBlock, outFileOffsetInBlock,
                inNumberOfSampleInBlock, outNumberOfSampleInBlock
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19, ?20, ?21
            )
            "#,
            rusqlite::params![
                cue.content_id,
                cue.kind,
                cue.color_table_index,
                cue.cue_comment,
                cue.is_active_loop,
                cue.beat_loop_numerator,
                cue.beat_loop_denominator,
                cue.in_usec,
                cue.out_usec,
                cue.in_150_frame_per_sec,
                cue.out_150_frame_per_sec,
                cue.in_mpeg_frame_number,
                cue.out_mpeg_frame_number,
                cue.in_mpeg_abs,
                cue.out_mpeg_abs,
                cue.in_decoding_start_frame_position,
                cue.out_decoding_start_frame_position,
                cue.in_file_offset_in_block,
                cue.out_file_offset_in_block,
                cue.in_number_of_sample_in_block,
                cue.out_number_of_sample_in_block,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    // ========================================================================
    // Playlist operations
    // ========================================================================

    /// Add a playlist. Returns the playlist_id.
    /// The crate_playlist_id is used for mapping back to the source playlist.
    pub fn add_playlist(&mut self, playlist: &Playlist, crate_playlist_id: &str) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO playlist (sequenceNo, name, image_id, attribute, playlist_id_parent)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            rusqlite::params![
                playlist.sequence_no,
                playlist.name,
                playlist.image_id,
                playlist.attribute,
                playlist.playlist_id_parent,
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        self.playlist_id_map
            .insert(crate_playlist_id.to_string(), id);
        Ok(id)
    }

    /// Get the Device Library Plus playlist ID for a Crate playlist ID.
    pub fn get_playlist_id(&self, crate_playlist_id: &str) -> Option<i64> {
        self.playlist_id_map.get(crate_playlist_id).copied()
    }

    /// Add a track to a playlist.
    pub fn add_playlist_content(
        &mut self,
        playlist_id: i64,
        content_id: i64,
        sequence_no: i32,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO playlist_content (playlist_id, content_id, sequenceNo) VALUES (?1, ?2, ?3)",
            rusqlite::params![playlist_id, content_id, sequence_no],
        )?;
        Ok(())
    }

    // ========================================================================
    // Property operations
    // ========================================================================

    /// Set the database property (metadata).
    pub fn set_property(&mut self, property: &Property) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO property (
                deviceName, dbVersion, numberOfContents, createdDate,
                backGroundColorType, myTagMasterDBID
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            rusqlite::params![
                property.device_name,
                property.db_version,
                property.number_of_contents,
                property.created_date,
                property.background_color_type,
                property.my_tag_master_db_id,
            ],
        )?;
        Ok(())
    }

    /// Update the content count in the property table.
    pub fn update_content_count(&mut self) -> Result<()> {
        let count: i32 = self
            .conn
            .query_row("SELECT COUNT(*) FROM content", [], |row| row.get(0))?;

        self.conn.execute(
            "UPDATE property SET numberOfContents = ?1",
            rusqlite::params![count],
        )?;

        Ok(())
    }

    // ========================================================================
    // Other table operations (for completeness)
    // ========================================================================

    /// Add a MyTag entry.
    pub fn add_my_tag(&mut self, my_tag: &MyTag) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO myTag (sequenceNo, name, attribute, myTag_id_parent) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                my_tag.sequence_no,
                my_tag.name,
                my_tag.attribute,
                my_tag.my_tag_id_parent,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Add content to a MyTag.
    pub fn add_my_tag_content(&mut self, my_tag_id: i64, content_id: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO myTag_content (myTag_id, content_id) VALUES (?1, ?2)",
            rusqlite::params![my_tag_id, content_id],
        )?;
        Ok(())
    }

    /// Add a history entry.
    pub fn add_history(&mut self, history: &History) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO history (sequenceNo, name, attribute, history_id_parent) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                history.sequence_no,
                history.name,
                history.attribute,
                history.history_id_parent,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Add content to history.
    pub fn add_history_content(
        &mut self,
        history_id: i64,
        content_id: i64,
        sequence_no: Option<i32>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO history_content (history_id, content_id, sequenceNo) VALUES (?1, ?2, ?3)",
            rusqlite::params![history_id, content_id, sequence_no],
        )?;
        Ok(())
    }

    /// Add a menu item.
    pub fn add_menu_item(&mut self, menu_item: &MenuItem) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO menuItem (kind, name) VALUES (?1, ?2)",
            rusqlite::params![menu_item.kind, menu_item.name],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Add a category.
    pub fn add_category(&mut self, category: &Category) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO category (menuItem_id, sequenceNo, isVisible) VALUES (?1, ?2, ?3)",
            rusqlite::params![
                category.menu_item_id,
                category.sequence_no,
                category.is_visible,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Add a sort entry.
    pub fn add_sort(&mut self, sort: &Sort) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO sort (menuItem_id, sequenceNo, isVisible, isSelectedAsSubColumn) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                sort.menu_item_id,
                sort.sequence_no,
                sort.is_visible,
                sort.is_selected_as_sub_column,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    // ========================================================================
    // Transaction management
    // ========================================================================

    /// Commit all changes to the database.
    pub fn commit(&self) -> Result<()> {
        // SQLite auto-commits by default, but we can ensure
        // all data is flushed by running a checkpoint
        self.conn.execute_batch("PRAGMA wal_checkpoint(FULL)")?;
        Ok(())
    }
}
