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
use crate::models::{ImportResult, Tag, Track, TrackFilter, TrackUpdate};

pub struct LibraryService {
    conn: Arc<Mutex<Connection>>,
}

impl LibraryService {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
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

    fn extract_bpm(&self, _tag: &dyn Accessor) -> Option<f64> {
        // BPM is often stored as a text field
        None // Will be populated by Rekordbox import or analysis
    }

    fn extract_key(&self, _tag: &dyn Accessor) -> Option<String> {
        // Key is often stored in a custom tag
        None // Will be populated by Rekordbox import or analysis
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
                rekordbox_id
            ) VALUES (
                ?1, ?2, ?3,
                ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16,
                ?17, ?18,
                ?19, ?20,
                ?21, ?22, ?23,
                ?24
            )
            ON CONFLICT(file_path) DO UPDATE SET
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                year = excluded.year,
                genre = excluded.genre,
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
                t.rekordbox_id
            FROM tracks t
            "#,
        );

        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref filter) = filter {
            if let Some(ref search) = filter.search {
                let search_param = format!("%{search}%");
                conditions
                    .push("(t.title LIKE ?1 OR t.artist LIKE ?1 OR t.album LIKE ?1)".to_string());
                params.push(Box::new(search_param));
            }

            if let Some(ref tag_ids) = filter.tag_ids {
                if !tag_ids.is_empty() {
                    let placeholders: Vec<String> = tag_ids
                        .iter()
                        .enumerate()
                        .map(|(i, _)| format!("?{}", params.len() + i + 1))
                        .collect();
                    conditions.push(format!(
                        "t.id IN (SELECT track_id FROM track_tags WHERE tag_id IN ({}))",
                        placeholders.join(", ")
                    ));
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
                rekordbox_id
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
                    tags: Vec::new(),
                })
            },
        )?;

        // Fetch tags
        let tracks_with_tags = self.fetch_tags_for_tracks(&conn, vec![track])?;
        Ok(tracks_with_tags.into_iter().next().unwrap())
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
}
