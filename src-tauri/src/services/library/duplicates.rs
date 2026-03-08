use super::*;

impl LibraryService {
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
