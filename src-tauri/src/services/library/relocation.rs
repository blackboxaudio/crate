use super::*;
use crate::services::cloud_sync::pipeline::{buckets, dirty};
use crate::services::cloud_sync::resolution;

impl LibraryService {
    /// Check if a track's file resolves to a playable location on this device.
    ///
    /// For cloud-synced tracks (with `library_root_id` + `relative_path`), this
    /// joins the logical relative path against the device-local mapping; for
    /// device-local tracks it falls back to the absolute `file_path`.
    pub fn check_track_file_exists(&self, id: &str) -> Result<bool> {
        let track = self.get_track(id)?;
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;
        let resolved = resolution::resolve_track_path(
            &conn,
            track.library_root_id.as_deref(),
            track.relative_path.as_deref(),
            &track.file_path,
        )?;
        Ok(matches!(resolved, resolution::ResolvedPath::Playable(_)))
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
            .map_err(|_| CrateError::LockPoisoned)?;

        let hlc = dirty::next_hlc(&conn)?;
        let (library_root_id, relative_path) =
            resolution::assign_root_for_import(&conn, &new_path_str)?;

        conn.execute(
            "UPDATE tracks SET file_path = ?1, file_hash = ?2, date_modified = ?3, \
                _hlc = ?4, library_root_id = ?5, relative_path = ?6 WHERE id = ?7",
            rusqlite::params![
                new_path_str,
                validation.new_hash,
                now,
                hlc,
                library_root_id,
                relative_path,
                id
            ],
        )?;
        dirty::mark_dirty(&conn, &buckets::bucket_for_track_id(id))?;

        drop(conn);
        self.get_track(id)
    }
}
