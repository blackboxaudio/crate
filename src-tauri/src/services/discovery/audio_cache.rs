use super::*;

impl DiscoveryService {
    pub fn audio_cache_dir(&self) -> PathBuf {
        self.app_data_dir.join("discovery").join("streams")
    }

    pub fn audio_cache_path(&self, release_id: &str, track_position: i32) -> PathBuf {
        self.audio_cache_dir()
            .join(format!("{release_id}_{track_position}"))
    }

    /// Check if audio bytes are cached on disk for a specific track.
    /// Returns `(content_type, file_size)` if cached.
    pub fn get_cached_audio_meta(
        &self,
        release_id: &str,
        track_position: i32,
    ) -> Result<Option<(String, i64)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let result = conn.query_row(
            "SELECT content_type, file_size FROM discovery_audio_cache
             WHERE release_id = ?1 AND track_position = ?2",
            rusqlite::params![release_id, track_position],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        );

        match result {
            Ok(meta) => Ok(Some(meta)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CrateError::Database(e)),
        }
    }

    /// Record that audio was cached to disk.
    pub fn save_audio_cache_entry(
        &self,
        release_id: &str,
        track_position: i32,
        content_type: &str,
        file_size: i64,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO discovery_audio_cache
             (release_id, track_position, content_type, file_size, cached_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![release_id, track_position, content_type, file_size, now],
        )?;

        Ok(())
    }

    /// Delete cached audio files from disk and remove DB entries for a release.
    pub fn delete_cached_audio_files(&self, release_id: &str) -> Result<()> {
        // Delete files from disk by scanning the cache directory for matching filenames
        let cache_dir = self.audio_cache_dir();
        let prefix = format!("{release_id}_");
        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with(&prefix) {
                        if let Err(e) = std::fs::remove_file(entry.path()) {
                            if e.kind() != std::io::ErrorKind::NotFound {
                                log::warn!(
                                    "Failed to delete cached audio file {}: {e}",
                                    entry.path().display()
                                );
                            }
                        }
                    }
                }
            }
        }

        // Clean up DB entries (best-effort, table may not exist yet)
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "DELETE FROM discovery_audio_cache WHERE release_id = ?1",
                [release_id],
            );
        }

        Ok(())
    }

    /// Get total size of all cached audio files in bytes (calculated from disk).
    pub fn get_audio_cache_total_size(&self) -> Result<i64> {
        let cache_dir = self.audio_cache_dir();
        let mut total: i64 = 0;
        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    if let Ok(meta) = entry.metadata() {
                        total += meta.len() as i64;
                    }
                }
            }
        }
        Ok(total)
    }

    /// Delete all cached audio files from disk and clear the DB table.
    pub fn clear_audio_cache(&self) -> Result<()> {
        // Delete all files in the streams directory
        let cache_dir = self.audio_cache_dir();
        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    if let Err(e) = std::fs::remove_file(entry.path()) {
                        log::warn!(
                            "Failed to delete cached audio file {}: {e}",
                            entry.path().display()
                        );
                    }
                }
            }
        }

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        conn.execute("DELETE FROM discovery_audio_cache", [])?;

        Ok(())
    }
}
