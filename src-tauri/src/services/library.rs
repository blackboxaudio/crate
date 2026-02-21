use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use lofty::config::{ParseOptions, ParsingMode};
use lofty::file::{AudioFile, TaggedFile};
use lofty::prelude::*;
use lofty::probe::Probe;
use rusqlite::Connection;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;

use crate::error::{CrateError, Result};
use crate::models::{
    DuplicateResolution, DuplicateTrack, FileMatchResult, ImportResult, ImportResultWithDuplicates,
    Tag, Track, TrackFilter, TrackUpdate,
};
use crate::services::hash::compute_audio_hash;
use crate::services::ArtworkService;

pub struct LibraryService {
    conn: Arc<Mutex<Connection>>,
    artwork_service: ArtworkService,
}

impl LibraryService {
    pub fn new(conn: Arc<Mutex<Connection>>, app_data_dir: PathBuf) -> Self {
        Self {
            conn,
            artwork_service: ArtworkService::new(app_data_dir),
        }
    }

    pub fn import_tracks(&self, paths: Vec<PathBuf>) -> Result<ImportResult> {
        let mut tracks = Vec::new();
        let mut errors = Vec::new();

        for path in paths {
            match self.import_single_track(&path) {
                Ok(track) => tracks.push(track),
                Err(e) => {
                    let error_msg = format!("{}: {}", path.display(), e);
                    log::warn!("Failed to import {error_msg}");
                    errors.push(error_msg);
                }
            }
        }

        Ok(ImportResult {
            tracks,
            failed_count: errors.len(),
            errors,
        })
    }

    fn import_single_track(&self, path: &PathBuf) -> Result<Track> {
        if !path.exists() {
            return Err(CrateError::FileNotFound(path.clone()));
        }

        // Determine format from extension
        let format = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        // Check if supported format
        let supported_formats = ["mp3", "wav", "aiff", "aif", "flac", "m4a", "aac"];
        if !supported_formats.contains(&format.as_str()) {
            return Err(CrateError::Import(format!("Unsupported format: {format}")));
        }

        // Try to read metadata with lenient parsing first
        let mut track = Track::new(
            path.to_string_lossy().to_string(),
            format.clone(),
            0, // Duration will be set below
        );

        if let Some(tagged_file) = self.read_metadata_lenient(path) {
            // Successfully read with lofty
            let properties = tagged_file.properties();
            track.duration_ms = properties.duration().as_millis() as i64;
            track.bitrate = properties.audio_bitrate().map(|b| b as i32);
            track.sample_rate = properties.sample_rate().map(|s| s as i32);

            // Extract tags if available
            if let Some(tag) = tagged_file
                .primary_tag()
                .or_else(|| tagged_file.first_tag())
            {
                track.title = tag.title().map(|s| s.to_string());
                track.artist = tag.artist().map(|s| s.to_string());
                track.album = tag.album().map(|s| s.to_string());
                track.year = tag.year().map(|y| y as i32);
                track.genre = tag.genre().map(|s| s.to_string());

                // Try to get BPM from various tag formats
                track.bpm = self.extract_bpm(tag);

                // Try to get key
                track.key = self.extract_key(tag);
            }

            // Extract album artwork
            if let Some(artwork_path) = self
                .artwork_service
                .extract_and_save(&tagged_file, &track.id)
            {
                track.artwork_path = Some(artwork_path);
                track.artwork_source = Some("extracted".to_string());
            }
        } else {
            // Lofty failed completely, use symphonia fallback
            log::warn!(
                "Metadata extraction failed for {}, falling back to symphonia",
                path.display()
            );

            let (dur, sr, br) = self.read_audio_properties_symphonia(path)?;
            track.duration_ms = dur;
            track.sample_rate = sr;
            track.bitrate = br;
        }

        // Compute file hash for future relocation matching
        if let Ok(hash) = compute_audio_hash(path) {
            track.file_hash = Some(hash);
        }

        // Insert into database
        self.insert_track(&track)?;

        Ok(track)
    }

    fn extract_bpm(&self, _tag: &dyn Accessor) -> Option<f64> {
        // BPM is often stored as a text field
        None // Will be populated by Rekordbox import or analysis
    }

    fn extract_key(&self, _tag: &dyn Accessor) -> Option<String> {
        // Key is often stored in a custom tag
        None // Will be populated by Rekordbox import or analysis
    }

    /// Find an existing track by its file hash
    pub fn find_track_by_hash(&self, file_hash: &str) -> Result<Option<Track>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let result = conn.query_row(
            r#"
            SELECT
                id, file_path, file_hash,
                title, artist, album, year, genre, label, catalog_number,
                duration_ms, bpm, key, bitrate, sample_rate, format,
                analysis_source, waveform_data,
                rating, play_count,
                date_added, date_modified, last_played,
                rekordbox_id, artwork_path, artwork_source, color
            FROM tracks WHERE file_hash = ?1
            "#,
            [file_hash],
            |row| {
                Ok(Track {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    file_hash: row.get(2)?,
                    title: row.get(3)?,
                    artist: row.get(4)?,
                    album: row.get(5)?,
                    year: row.get(6)?,
                    genre: row.get(7)?,
                    label: row.get(8)?,
                    catalog_number: row.get(9)?,
                    duration_ms: row.get(10)?,
                    bpm: row.get(11)?,
                    key: row.get(12)?,
                    bitrate: row.get(13)?,
                    sample_rate: row.get(14)?,
                    format: row.get(15)?,
                    analysis_source: row.get(16)?,
                    waveform_data: row.get(17)?,
                    rating: row.get(18)?,
                    play_count: row.get(19)?,
                    date_added: row.get(20)?,
                    date_modified: row.get(21)?,
                    last_played: row.get(22)?,
                    rekordbox_id: row.get(23)?,
                    artwork_path: row.get(24)?,
                    artwork_source: row.get(25)?,
                    color: row.get(26)?,
                    tags: Vec::new(),
                })
            },
        );

        match result {
            Ok(track) => {
                // Fetch tags for the track
                let tracks_with_tags = self.fetch_tags_for_tracks(&conn, vec![track])?;
                Ok(tracks_with_tags.into_iter().next())
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CrateError::Database(e)),
        }
    }

    /// Import tracks with duplicate detection based on content hash
    pub fn import_tracks_with_duplicate_detection(
        &self,
        paths: Vec<PathBuf>,
    ) -> Result<ImportResultWithDuplicates> {
        let mut tracks = Vec::new();
        let mut errors = Vec::new();
        let mut duplicates = Vec::new();

        for path in paths {
            match self.process_import_path(&path) {
                Ok(ImportPathResult::NewTrack(track)) => tracks.push(track),
                Ok(ImportPathResult::Duplicate(dup)) => duplicates.push(dup),
                Err(e) => {
                    let error_msg = format!("{}: {}", path.display(), e);
                    log::warn!("Failed to import {error_msg}");
                    errors.push(error_msg);
                }
            }
        }

        Ok(ImportResultWithDuplicates {
            tracks,
            failed_count: errors.len(),
            errors,
            duplicates,
        })
    }

    /// Process a single import path, checking for duplicates first
    fn process_import_path(&self, path: &PathBuf) -> Result<ImportPathResult> {
        if !path.exists() {
            return Err(CrateError::FileNotFound(path.clone()));
        }

        // Check format
        let format = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        let supported_formats = ["mp3", "wav", "aiff", "aif", "flac", "m4a", "aac"];
        if !supported_formats.contains(&format.as_str()) {
            return Err(CrateError::Import(format!("Unsupported format: {format}")));
        }

        // Compute hash first to check for duplicates
        let file_hash = compute_audio_hash(path)?;

        // Check if a track with this hash already exists
        if let Some(existing_track) = self.find_track_by_hash(&file_hash)? {
            return Ok(ImportPathResult::Duplicate(DuplicateTrack {
                new_file_path: path.to_string_lossy().to_string(),
                new_file_hash: file_hash,
                existing_track,
            }));
        }

        // No duplicate - proceed with normal import
        let track = self.import_single_track_with_hash(path, file_hash)?;
        Ok(ImportPathResult::NewTrack(track))
    }

    /// Import a single track with a pre-computed hash
    fn import_single_track_with_hash(&self, path: &PathBuf, file_hash: String) -> Result<Track> {
        // Determine format from extension
        let format = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        // Create track with pre-computed hash
        let mut track = Track::new(
            path.to_string_lossy().to_string(),
            format.clone(),
            0, // Duration will be set below
        );
        track.file_hash = Some(file_hash);

        if let Some(tagged_file) = self.read_metadata_lenient(path) {
            // Successfully read with lofty
            let properties = tagged_file.properties();
            track.duration_ms = properties.duration().as_millis() as i64;
            track.bitrate = properties.audio_bitrate().map(|b| b as i32);
            track.sample_rate = properties.sample_rate().map(|s| s as i32);

            // Extract tags if available
            if let Some(tag) = tagged_file
                .primary_tag()
                .or_else(|| tagged_file.first_tag())
            {
                track.title = tag.title().map(|s| s.to_string());
                track.artist = tag.artist().map(|s| s.to_string());
                track.album = tag.album().map(|s| s.to_string());
                track.year = tag.year().map(|y| y as i32);
                track.genre = tag.genre().map(|s| s.to_string());
                track.bpm = self.extract_bpm(tag);
                track.key = self.extract_key(tag);
            }

            // Extract album artwork
            if let Some(artwork_path) = self
                .artwork_service
                .extract_and_save(&tagged_file, &track.id)
            {
                track.artwork_path = Some(artwork_path);
                track.artwork_source = Some("extracted".to_string());
            }
        } else {
            // Lofty failed completely, use symphonia fallback
            log::warn!(
                "Metadata extraction failed for {}, falling back to symphonia",
                path.display()
            );

            let (dur, sr, br) = self.read_audio_properties_symphonia(path)?;
            track.duration_ms = dur;
            track.sample_rate = sr;
            track.bitrate = br;
        }

        // Insert into database
        self.insert_track(&track)?;

        Ok(track)
    }

    /// Attempts to read audio file metadata with lenient parsing options.
    /// Returns None if parsing fails completely.
    fn read_metadata_lenient(&self, path: &PathBuf) -> Option<TaggedFile> {
        let file = File::open(path).ok()?;
        let reader = BufReader::new(file);

        let parse_options = ParseOptions::new()
            .parsing_mode(ParsingMode::Relaxed)
            .max_junk_bytes(4096);

        Probe::new(reader)
            .options(parse_options)
            .guess_file_type()
            .ok()?
            .read()
            .ok()
    }

    /// Fallback to extract audio properties using symphonia when lofty fails.
    /// Returns (duration_ms, sample_rate, bitrate).
    fn read_audio_properties_symphonia(
        &self,
        path: &PathBuf,
    ) -> Result<(i64, Option<i32>, Option<i32>)> {
        let file =
            File::open(path).map_err(|e| CrateError::Metadata(format!("Failed to open: {e}")))?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &Default::default(), &Default::default())
            .map_err(|e| CrateError::Metadata(format!("Symphonia probe failed: {e}")))?;

        let track = probed
            .format
            .default_track()
            .ok_or_else(|| CrateError::Metadata("No audio track found".to_string()))?;

        let params = &track.codec_params;

        let duration_ms = match (params.n_frames, params.sample_rate) {
            (Some(frames), Some(sample_rate)) => {
                ((frames as f64 / sample_rate as f64) * 1000.0) as i64
            }
            _ => 0,
        };

        let sample_rate = params.sample_rate.map(|s| s as i32);
        let bitrate = params.bits_per_sample.map(|b| b as i32);

        Ok((duration_ms, sample_rate, bitrate))
    }

    fn insert_track(&self, track: &Track) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        conn.execute(
            r#"
            INSERT INTO tracks (
                id, file_path, file_hash,
                title, artist, album, year, genre, label, catalog_number,
                duration_ms, bpm, key, bitrate, sample_rate, format,
                analysis_source, waveform_data,
                rating, play_count,
                date_added, date_modified, last_played,
                rekordbox_id, artwork_path, artwork_source, color
            ) VALUES (
                ?1, ?2, ?3,
                ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16,
                ?17, ?18,
                ?19, ?20,
                ?21, ?22, ?23,
                ?24, ?25, ?26, ?27
            )
            ON CONFLICT(file_path) DO UPDATE SET
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                year = excluded.year,
                genre = excluded.genre,
                artwork_path = excluded.artwork_path,
                artwork_source = excluded.artwork_source,
                date_modified = excluded.date_modified
            "#,
            rusqlite::params![
                track.id,
                track.file_path,
                track.file_hash,
                track.title,
                track.artist,
                track.album,
                track.year,
                track.genre,
                track.label,
                track.catalog_number,
                track.duration_ms,
                track.bpm,
                track.key,
                track.bitrate,
                track.sample_rate,
                track.format,
                track.analysis_source,
                track.waveform_data,
                track.rating,
                track.play_count,
                track.date_added,
                track.date_modified,
                track.last_played,
                track.rekordbox_id,
                track.artwork_path,
                track.artwork_source,
                track.color,
            ],
        )?;

        Ok(())
    }

    pub fn get_tracks(&self, filter: Option<TrackFilter>) -> Result<Vec<Track>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut sql = String::from(
            r#"
            SELECT
                t.id, t.file_path, t.file_hash,
                t.title, t.artist, t.album, t.year, t.genre, t.label, t.catalog_number,
                t.duration_ms, t.bpm, t.key, t.bitrate, t.sample_rate, t.format,
                t.analysis_source, t.waveform_data,
                t.rating, t.play_count,
                t.date_added, t.date_modified, t.last_played,
                t.rekordbox_id, t.artwork_path, t.artwork_source, t.color
            FROM tracks t
            "#,
        );

        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref filter) = filter {
            if let Some(ref search) = filter.search {
                let escaped = search.replace('%', "\\%").replace('_', "\\_");
                let search_param = format!("%{escaped}%");
                conditions.push(
                    "(t.title LIKE ?1 ESCAPE '\\' OR t.artist LIKE ?1 ESCAPE '\\' OR t.album LIKE ?1 ESCAPE '\\')"
                        .to_string(),
                );
                params.push(Box::new(search_param));
            }

            if let Some(ref tag_ids) = filter.tag_ids {
                if !tag_ids.is_empty() {
                    let placeholders: Vec<String> = tag_ids
                        .iter()
                        .enumerate()
                        .map(|(i, _)| format!("?{}", params.len() + i + 1))
                        .collect();

                    // Check filter mode: "and" requires all tags, "or" (default) requires any tag
                    let is_and_mode = filter
                        .tag_filter_mode
                        .as_ref()
                        .map(|m| m == "and")
                        .unwrap_or(false);

                    if is_and_mode {
                        // AND mode: track must have ALL selected tags
                        conditions.push(format!(
                            "t.id IN (SELECT track_id FROM track_tags WHERE tag_id IN ({}) GROUP BY track_id HAVING COUNT(DISTINCT tag_id) = {})",
                            placeholders.join(", "),
                            tag_ids.len()
                        ));
                    } else {
                        // OR mode: track must have ANY of the selected tags
                        conditions.push(format!(
                            "t.id IN (SELECT track_id FROM track_tags WHERE tag_id IN ({}))",
                            placeholders.join(", ")
                        ));
                    }

                    for tag_id in tag_ids {
                        params.push(Box::new(tag_id.clone()));
                    }
                }
            }

            if let Some(ref playlist_id) = filter.playlist_id {
                conditions.push(format!(
                    "t.id IN (SELECT track_id FROM playlist_tracks WHERE playlist_id = ?{})",
                    params.len() + 1
                ));
                params.push(Box::new(playlist_id.clone()));
            }

            if let Some(bpm_min) = filter.bpm_min {
                conditions.push(format!("t.bpm >= ?{}", params.len() + 1));
                params.push(Box::new(bpm_min));
            }

            if let Some(bpm_max) = filter.bpm_max {
                conditions.push(format!("t.bpm <= ?{}", params.len() + 1));
                params.push(Box::new(bpm_max));
            }

            if let Some(ref key) = filter.key {
                conditions.push(format!("t.key = ?{}", params.len() + 1));
                params.push(Box::new(key.clone()));
            }
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY t.date_added DESC");

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let tracks = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(Track {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    file_hash: row.get(2)?,
                    title: row.get(3)?,
                    artist: row.get(4)?,
                    album: row.get(5)?,
                    year: row.get(6)?,
                    genre: row.get(7)?,
                    label: row.get(8)?,
                    catalog_number: row.get(9)?,
                    duration_ms: row.get(10)?,
                    bpm: row.get(11)?,
                    key: row.get(12)?,
                    bitrate: row.get(13)?,
                    sample_rate: row.get(14)?,
                    format: row.get(15)?,
                    analysis_source: row.get(16)?,
                    waveform_data: row.get(17)?,
                    rating: row.get(18)?,
                    play_count: row.get(19)?,
                    date_added: row.get(20)?,
                    date_modified: row.get(21)?,
                    last_played: row.get(22)?,
                    rekordbox_id: row.get(23)?,
                    artwork_path: row.get(24)?,
                    artwork_source: row.get(25)?,
                    color: row.get(26)?,
                    tags: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Fetch tags for all tracks
        let tracks_with_tags = self.fetch_tags_for_tracks(&conn, tracks)?;

        Ok(tracks_with_tags)
    }

    fn fetch_tags_for_tracks(
        &self,
        conn: &Connection,
        mut tracks: Vec<Track>,
    ) -> Result<Vec<Track>> {
        if tracks.is_empty() {
            return Ok(tracks);
        }

        let track_ids: Vec<String> = tracks.iter().map(|t| t.id.clone()).collect();
        let placeholders: Vec<String> = track_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();

        let sql = format!(
            r#"
            SELECT tt.track_id, t.id, t.category_id, t.name, t.color, t.sort_order
            FROM track_tags tt
            JOIN tags t ON tt.tag_id = t.id
            WHERE tt.track_id IN ({})
            "#,
            placeholders.join(", ")
        );

        let params_refs: Vec<&dyn rusqlite::ToSql> = track_ids
            .iter()
            .map(|s| s as &dyn rusqlite::ToSql)
            .collect();

        let mut stmt = conn.prepare(&sql)?;
        let tag_rows = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    Tag {
                        id: row.get(1)?,
                        category_id: row.get(2)?,
                        name: row.get(3)?,
                        color: row.get(4)?,
                        sort_order: row.get(5)?,
                    },
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Group tags by track_id
        let mut tags_by_track: std::collections::HashMap<String, Vec<Tag>> =
            std::collections::HashMap::new();
        for (track_id, tag) in tag_rows {
            tags_by_track.entry(track_id).or_default().push(tag);
        }

        // Assign tags to tracks
        for track in &mut tracks {
            if let Some(tags) = tags_by_track.remove(&track.id) {
                track.tags = tags;
            }
        }

        Ok(tracks)
    }

    pub fn get_track(&self, id: &str) -> Result<Track> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let track = conn.query_row(
            r#"
            SELECT
                id, file_path, file_hash,
                title, artist, album, year, genre, label, catalog_number,
                duration_ms, bpm, key, bitrate, sample_rate, format,
                analysis_source, waveform_data,
                rating, play_count,
                date_added, date_modified, last_played,
                rekordbox_id, artwork_path, artwork_source, color
            FROM tracks WHERE id = ?1
            "#,
            [id],
            |row| {
                Ok(Track {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    file_hash: row.get(2)?,
                    title: row.get(3)?,
                    artist: row.get(4)?,
                    album: row.get(5)?,
                    year: row.get(6)?,
                    genre: row.get(7)?,
                    label: row.get(8)?,
                    catalog_number: row.get(9)?,
                    duration_ms: row.get(10)?,
                    bpm: row.get(11)?,
                    key: row.get(12)?,
                    bitrate: row.get(13)?,
                    sample_rate: row.get(14)?,
                    format: row.get(15)?,
                    analysis_source: row.get(16)?,
                    waveform_data: row.get(17)?,
                    rating: row.get(18)?,
                    play_count: row.get(19)?,
                    date_added: row.get(20)?,
                    date_modified: row.get(21)?,
                    last_played: row.get(22)?,
                    rekordbox_id: row.get(23)?,
                    artwork_path: row.get(24)?,
                    artwork_source: row.get(25)?,
                    color: row.get(26)?,
                    tags: Vec::new(),
                })
            },
        )?;

        // Fetch tags
        let tracks_with_tags = self.fetch_tags_for_tracks(&conn, vec![track])?;
        tracks_with_tags
            .into_iter()
            .next()
            .ok_or_else(|| CrateError::TrackNotFound(id.to_string()))
    }

    pub fn update_track(&self, id: &str, update: TrackUpdate) -> Result<Track> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();

        // Build update query dynamically based on provided fields
        let mut updates: Vec<String> = vec!["date_modified = ?1".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
        let mut param_idx = 2;

        if let Some(ref title) = update.title {
            updates.push(format!("title = ?{param_idx}"));
            params.push(Box::new(title.clone()));
            param_idx += 1;
        }
        if let Some(ref artist) = update.artist {
            updates.push(format!("artist = ?{param_idx}"));
            params.push(Box::new(artist.clone()));
            param_idx += 1;
        }
        if let Some(ref album) = update.album {
            updates.push(format!("album = ?{param_idx}"));
            params.push(Box::new(album.clone()));
            param_idx += 1;
        }
        if let Some(year) = update.year {
            updates.push(format!("year = ?{param_idx}"));
            params.push(Box::new(year));
            param_idx += 1;
        }
        if let Some(ref genre) = update.genre {
            updates.push(format!("genre = ?{param_idx}"));
            params.push(Box::new(genre.clone()));
            param_idx += 1;
        }
        if let Some(ref label) = update.label {
            updates.push(format!("label = ?{param_idx}"));
            params.push(Box::new(label.clone()));
            param_idx += 1;
        }
        if let Some(bpm) = update.bpm {
            updates.push(format!("bpm = ?{param_idx}"));
            params.push(Box::new(bpm));
            param_idx += 1;
        }
        if let Some(ref key) = update.key {
            updates.push(format!("key = ?{param_idx}"));
            params.push(Box::new(key.clone()));
            param_idx += 1;
        }
        if let Some(rating) = update.rating {
            updates.push(format!("rating = ?{param_idx}"));
            params.push(Box::new(rating));
            param_idx += 1;
        }

        params.push(Box::new(id.to_string()));

        let sql = format!(
            "UPDATE tracks SET {} WHERE id = ?{}",
            updates.join(", "),
            param_idx
        );

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        conn.execute(&sql, params_refs.as_slice())?;

        drop(conn);
        self.get_track(id)
    }

    pub fn delete_tracks(&self, ids: Vec<String>) -> Result<()> {
        // Delete artwork files for each track
        for id in &ids {
            self.artwork_service.delete(id);
        }

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let placeholders: Vec<String> = ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();

        let sql = format!(
            "DELETE FROM tracks WHERE id IN ({})",
            placeholders.join(", ")
        );

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        conn.execute(&sql, params_refs.as_slice())?;

        Ok(())
    }

    /// Rescan artwork for a single track by re-reading the audio file
    pub fn rescan_track_artwork(&self, track_id: &str) -> Result<bool> {
        let track = self.get_track(track_id)?;
        let path = std::path::PathBuf::from(&track.file_path);

        // Try to read metadata and extract artwork
        if let Some(tagged_file) = self.read_metadata_lenient(&path) {
            if let Some(artwork_path) = self
                .artwork_service
                .extract_and_save(&tagged_file, track_id)
            {
                // Update database with new artwork path and source
                let conn = self
                    .conn
                    .lock()
                    .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

                conn.execute(
                    "UPDATE tracks SET artwork_path = ?1, artwork_source = 'extracted' WHERE id = ?2",
                    rusqlite::params![artwork_path, track_id],
                )?;

                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Rescan artwork for all tracks that don't have artwork yet
    pub fn rescan_all_artwork(&self) -> Result<RescanResult> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        // Get all tracks without artwork
        let mut stmt =
            conn.prepare("SELECT id, file_path FROM tracks WHERE artwork_path IS NULL")?;

        let tracks: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        drop(stmt);
        drop(conn);

        let mut updated_count = 0;
        let mut failed_count = 0;

        for (track_id, file_path) in tracks {
            let path = std::path::PathBuf::from(&file_path);

            if let Some(tagged_file) = self.read_metadata_lenient(&path) {
                if let Some(artwork_path) = self
                    .artwork_service
                    .extract_and_save(&tagged_file, &track_id)
                {
                    // Update database with artwork path and source
                    if let Ok(conn) = self.conn.lock() {
                        if conn
                            .execute(
                                "UPDATE tracks SET artwork_path = ?1, artwork_source = 'extracted' WHERE id = ?2",
                                rusqlite::params![artwork_path, track_id],
                            )
                            .is_ok()
                        {
                            updated_count += 1;
                            continue;
                        }
                    }
                }
            }
            failed_count += 1;
        }

        Ok(RescanResult {
            updated_count,
            failed_count,
        })
    }

    /// Check if a track's file still exists on disk
    pub fn check_track_file_exists(&self, id: &str) -> Result<bool> {
        let track = self.get_track(id)?;
        let path = std::path::Path::new(&track.file_path);
        Ok(path.exists())
    }

    /// Validate if a replacement file matches the original track
    pub fn validate_replacement_file(
        &self,
        id: &str,
        new_path: &std::path::Path,
    ) -> Result<FileMatchResult> {
        let track = self.get_track(id)?;

        // Check if file exists
        if !new_path.exists() {
            return Err(CrateError::FileNotFound(new_path.to_path_buf()));
        }

        // Check format validity
        let new_format = new_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        let supported_formats = ["mp3", "wav", "aiff", "aif", "flac", "m4a", "aac"];
        let format_valid = supported_formats.contains(&new_format.as_str());

        // Compute hash of new file
        let new_hash = compute_audio_hash(new_path)?;

        // Check if hashes match
        let matches = track
            .file_hash
            .as_ref()
            .map(|h| h == &new_hash)
            .unwrap_or(false);

        Ok(FileMatchResult {
            matches,
            original_hash: track.file_hash,
            new_hash,
            format_valid,
        })
    }

    /// Relocate a track to a new file path
    pub fn relocate_track(
        &self,
        id: &str,
        new_path: &std::path::Path,
        force: bool,
    ) -> Result<Track> {
        // Validate the replacement file first
        let validation = self.validate_replacement_file(id, new_path)?;

        // If not forcing and hashes don't match, return error
        if !force && !validation.matches && validation.original_hash.is_some() {
            return Err(CrateError::Import(
                "File content does not match original. Use force=true to override.".to_string(),
            ));
        }

        // Check format is valid
        if !validation.format_valid {
            return Err(CrateError::Import("Unsupported audio format".to_string()));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let new_path_str = new_path.to_string_lossy().to_string();

        // Update the database with new path and hash
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        conn.execute(
            "UPDATE tracks SET file_path = ?1, file_hash = ?2, date_modified = ?3 WHERE id = ?4",
            rusqlite::params![new_path_str, validation.new_hash, now, id],
        )?;

        drop(conn);
        self.get_track(id)
    }

    /// Set color for multiple tracks (bulk operation)
    pub fn set_track_colors(&self, track_ids: Vec<String>, color: Option<String>) -> Result<()> {
        if track_ids.is_empty() {
            return Ok(());
        }

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();

        let placeholders: Vec<String> = track_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 3))
            .collect();

        let sql = format!(
            "UPDATE tracks SET color = ?1, date_modified = ?2 WHERE id IN ({})",
            placeholders.join(", ")
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(color), Box::new(now)];

        for id in track_ids {
            params.push(Box::new(id));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())?;

        Ok(())
    }

    /// Set artwork for a track from a user-provided file
    pub fn set_track_artwork(&self, id: &str, file_path: &std::path::Path) -> Result<Track> {
        // Validate file exists
        if !file_path.exists() {
            return Err(CrateError::FileNotFound(file_path.to_path_buf()));
        }

        // Save the artwork using ArtworkService
        let artwork_path = self
            .artwork_service
            .save_from_file(file_path, id)
            .ok_or_else(|| CrateError::Artwork("Failed to save artwork".to_string()))?;

        // Update database with new artwork path and source
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE tracks SET artwork_path = ?1, artwork_source = 'user_provided', date_modified = ?2 WHERE id = ?3",
            rusqlite::params![artwork_path, now, id],
        )?;

        drop(conn);
        self.get_track(id)
    }

    /// Delete artwork for a track
    pub fn delete_track_artwork(&self, id: &str) -> Result<Track> {
        // Delete the artwork file
        self.artwork_service.delete(id);

        // Update database to clear artwork columns
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE tracks SET artwork_path = NULL, artwork_source = NULL, date_modified = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        )?;

        drop(conn);
        self.get_track(id)
    }

    /// Re-extract artwork from the audio file (replaces user-provided artwork)
    pub fn reextract_track_artwork(&self, id: &str) -> Result<Track> {
        let track = self.get_track(id)?;
        let path = std::path::PathBuf::from(&track.file_path);

        // Check if file exists
        if !path.exists() {
            return Err(CrateError::FileNotFound(path));
        }

        // Try to read metadata and extract artwork
        if let Some(tagged_file) = self.read_metadata_lenient(&path) {
            if let Some(artwork_path) = self.artwork_service.extract_and_save(&tagged_file, id) {
                // Update database with new artwork path and source
                let conn = self
                    .conn
                    .lock()
                    .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

                let now = chrono::Utc::now().to_rfc3339();

                conn.execute(
                    "UPDATE tracks SET artwork_path = ?1, artwork_source = 'extracted', date_modified = ?2 WHERE id = ?3",
                    rusqlite::params![artwork_path, now, id],
                )?;

                drop(conn);
                return self.get_track(id);
            }
        }

        Err(CrateError::Artwork(
            "No artwork found in audio file".to_string(),
        ))
    }

    /// Compare artwork files for multiple tracks to check if they are identical.
    /// Returns the shared artwork path if all tracks have identical artwork, or None otherwise.
    pub fn compare_track_artworks(&self, track_ids: &[String]) -> Result<Option<String>> {
        if track_ids.len() < 2 {
            return Ok(None);
        }

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut artwork_paths: Vec<String> = Vec::new();

        for id in track_ids {
            let path: Option<String> = conn
                .query_row(
                    "SELECT artwork_path FROM tracks WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )
                .map_err(CrateError::Database)?;

            match path {
                Some(p) => artwork_paths.push(p),
                None => return Ok(None),
            }
        }

        drop(conn);

        if self.artwork_service.are_artworks_identical(&artwork_paths) {
            Ok(artwork_paths.into_iter().next())
        } else {
            Ok(None)
        }
    }

    /// Update multiple tracks with the same update data (bulk operation)
    pub fn update_tracks(&self, ids: Vec<String>, update: TrackUpdate) -> Result<Vec<Track>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();

        // Build update query dynamically based on provided fields
        let mut updates: Vec<String> = vec!["date_modified = ?1".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
        let mut param_idx = 2;

        if let Some(ref title) = update.title {
            updates.push(format!("title = ?{param_idx}"));
            params.push(Box::new(title.clone()));
            param_idx += 1;
        }
        if let Some(ref artist) = update.artist {
            updates.push(format!("artist = ?{param_idx}"));
            params.push(Box::new(artist.clone()));
            param_idx += 1;
        }
        if let Some(ref album) = update.album {
            updates.push(format!("album = ?{param_idx}"));
            params.push(Box::new(album.clone()));
            param_idx += 1;
        }
        if let Some(year) = update.year {
            updates.push(format!("year = ?{param_idx}"));
            params.push(Box::new(year));
            param_idx += 1;
        }
        if let Some(ref genre) = update.genre {
            updates.push(format!("genre = ?{param_idx}"));
            params.push(Box::new(genre.clone()));
            param_idx += 1;
        }
        if let Some(ref label) = update.label {
            updates.push(format!("label = ?{param_idx}"));
            params.push(Box::new(label.clone()));
            param_idx += 1;
        }
        if let Some(bpm) = update.bpm {
            updates.push(format!("bpm = ?{param_idx}"));
            params.push(Box::new(bpm));
            param_idx += 1;
        }
        if let Some(ref key) = update.key {
            updates.push(format!("key = ?{param_idx}"));
            params.push(Box::new(key.clone()));
            param_idx += 1;
        }
        if let Some(rating) = update.rating {
            updates.push(format!("rating = ?{param_idx}"));
            params.push(Box::new(rating));
            param_idx += 1;
        }

        // Build placeholders for track IDs
        let placeholders: Vec<String> = ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", param_idx + i))
            .collect();

        for id in &ids {
            params.push(Box::new(id.clone()));
        }

        let sql = format!(
            "UPDATE tracks SET {} WHERE id IN ({})",
            updates.join(", "),
            placeholders.join(", ")
        );

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())?;

        drop(conn);

        // Return all updated tracks
        let mut updated_tracks = Vec::new();
        for id in ids {
            if let Ok(track) = self.get_track(&id) {
                updated_tracks.push(track);
            }
        }

        Ok(updated_tracks)
    }

    /// Resolve a duplicate by applying the user's chosen action
    pub fn resolve_duplicate(&self, resolution: DuplicateResolution) -> Result<Option<Track>> {
        match resolution {
            DuplicateResolution::Skip => Ok(None),
            DuplicateResolution::UpdatePath { new_path } => {
                // Find the track by the new path's hash
                let path = PathBuf::from(&new_path);
                let file_hash = compute_audio_hash(&path)?;

                if let Some(existing_track) = self.find_track_by_hash(&file_hash)? {
                    let track =
                        self.resolve_duplicate_update_path(&existing_track.id, &new_path)?;
                    Ok(Some(track))
                } else {
                    Err(CrateError::TrackNotFound(
                        "No existing track found with matching hash".to_string(),
                    ))
                }
            }
            DuplicateResolution::Replace { new_path, new_hash } => {
                if let Some(existing_track) = self.find_track_by_hash(&new_hash)? {
                    let path = PathBuf::from(&new_path);
                    let track =
                        self.resolve_duplicate_replace(&existing_track.id, &path, &new_hash)?;
                    Ok(Some(track))
                } else {
                    Err(CrateError::TrackNotFound(
                        "No existing track found with matching hash".to_string(),
                    ))
                }
            }
        }
    }

    /// Resolve a duplicate by updating the existing track's file path only
    fn resolve_duplicate_update_path(
        &self,
        existing_track_id: &str,
        new_path: &str,
    ) -> Result<Track> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();

        // Update file_path and date_modified only
        conn.execute(
            "UPDATE tracks SET file_path = ?1, date_modified = ?2 WHERE id = ?3",
            rusqlite::params![new_path, now, existing_track_id],
        )?;

        drop(conn);
        self.get_track(existing_track_id)
    }

    /// Resolve a duplicate by replacing: fresh import keeping only playlist memberships
    fn resolve_duplicate_replace(
        &self,
        existing_track_id: &str,
        new_path: &PathBuf,
        new_file_hash: &str,
    ) -> Result<Track> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        // 1. Get existing playlist memberships
        let playlist_memberships: Vec<(String, i32, String)> = {
            let mut stmt = conn.prepare(
                "SELECT playlist_id, position, date_added FROM playlist_tracks WHERE track_id = ?1",
            )?;
            let rows = stmt.query_map([existing_track_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        // 2. Delete related data (cues, tags)
        conn.execute("DELETE FROM cues WHERE track_id = ?1", [existing_track_id])?;
        conn.execute(
            "DELETE FROM track_tags WHERE track_id = ?1",
            [existing_track_id],
        )?;

        // 3. Delete playlist_tracks entries (we'll restore them after)
        conn.execute(
            "DELETE FROM playlist_tracks WHERE track_id = ?1",
            [existing_track_id],
        )?;

        // 4. Delete the old track
        conn.execute("DELETE FROM tracks WHERE id = ?1", [existing_track_id])?;

        // Delete old artwork
        self.artwork_service.delete(existing_track_id);

        drop(conn);

        // 5. Import fresh track
        let track = self.import_single_track_with_hash(new_path, new_file_hash.to_string())?;

        // 6. Restore playlist memberships with the new track ID
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        for (playlist_id, position, date_added) in playlist_memberships {
            conn.execute(
                "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position, date_added) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![playlist_id, track.id, position, date_added],
            )?;
        }

        drop(conn);
        self.get_track(&track.id)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RescanResult {
    pub updated_count: usize,
    pub failed_count: usize,
}

/// Result of processing a single import path
enum ImportPathResult {
    NewTrack(Track),
    Duplicate(DuplicateTrack),
}
