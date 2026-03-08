use std::fs::File;
use std::io::BufReader;

use lofty::config::{ParseOptions, ParsingMode};
use lofty::file::{AudioFile, TaggedFile};
use lofty::prelude::*;
use lofty::probe::Probe;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;

use super::*;

impl LibraryService {
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

    /// Import a single track with a pre-computed hash
    pub(crate) fn import_single_track_with_hash(
        &self,
        path: &PathBuf,
        file_hash: String,
    ) -> Result<Track> {
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

    /// Attempts to read audio file metadata with lenient parsing options.
    /// Returns None if parsing fails completely.
    pub(crate) fn read_metadata_lenient(&self, path: &PathBuf) -> Option<TaggedFile> {
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

    fn extract_bpm(&self, _tag: &dyn Accessor) -> Option<f64> {
        // BPM is often stored as a text field
        None // Will be populated by Rekordbox import or analysis
    }

    fn extract_key(&self, _tag: &dyn Accessor) -> Option<String> {
        // Key is often stored in a custom tag
        None // Will be populated by Rekordbox import or analysis
    }
}
