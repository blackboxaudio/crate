use super::*;
use crate::services::cloud_sync::pipeline::{buckets, dirty};

impl LibraryService {
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
                    .map_err(|_| CrateError::LockPoisoned)?;

                let hlc = dirty::next_hlc(&conn)?;
                conn.execute(
                    "UPDATE tracks SET artwork_path = ?1, artwork_source = 'extracted', _hlc = ?2 WHERE id = ?3",
                    rusqlite::params![artwork_path, hlc, track_id],
                )?;
                dirty::mark_dirty(&conn, &buckets::bucket_for_track_id(track_id))?;

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
            .map_err(|_| CrateError::LockPoisoned)?;

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
                        let hlc = dirty::next_hlc(&conn).unwrap_or_default();
                        if conn
                            .execute(
                                "UPDATE tracks SET artwork_path = ?1, artwork_source = 'extracted', _hlc = ?2 WHERE id = ?3",
                                rusqlite::params![artwork_path, hlc, track_id],
                            )
                            .is_ok()
                        {
                            let _ =
                                dirty::mark_dirty(&conn, &buckets::bucket_for_track_id(&track_id));
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
            .map_err(|_| CrateError::LockPoisoned)?;

        let now = chrono::Utc::now().to_rfc3339();

        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "UPDATE tracks SET artwork_path = ?1, artwork_source = 'user_provided', date_modified = ?2, _hlc = ?3 WHERE id = ?4",
            rusqlite::params![artwork_path, now, hlc, id],
        )?;
        dirty::mark_dirty(&conn, &buckets::bucket_for_track_id(id))?;

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
            .map_err(|_| CrateError::LockPoisoned)?;

        let now = chrono::Utc::now().to_rfc3339();

        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "UPDATE tracks SET artwork_path = NULL, artwork_source = NULL, date_modified = ?1, _hlc = ?2 WHERE id = ?3",
            rusqlite::params![now, hlc, id],
        )?;
        dirty::mark_dirty(&conn, &buckets::bucket_for_track_id(id))?;

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
                    .map_err(|_| CrateError::LockPoisoned)?;

                let now = chrono::Utc::now().to_rfc3339();

                let hlc = dirty::next_hlc(&conn)?;
                conn.execute(
                    "UPDATE tracks SET artwork_path = ?1, artwork_source = 'extracted', date_modified = ?2, _hlc = ?3 WHERE id = ?4",
                    rusqlite::params![artwork_path, now, hlc, id],
                )?;
                dirty::mark_dirty(&conn, &buckets::bucket_for_track_id(id))?;

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
            .map_err(|_| CrateError::LockPoisoned)?;

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
}
