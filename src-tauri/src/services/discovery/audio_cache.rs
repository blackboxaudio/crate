use super::*;

/// Default cap (in MB) on the total on-disk audio-byte cache when the user hasn't set one.
/// The effective cap is read from the `discovery_audio_cache_limit_mb` setting (device-local),
/// so once exceeded the least-recently-played tracks are evicted (LRU) after each cache write.
const DEFAULT_AUDIO_CACHE_MB: i64 = 500;

impl DiscoveryService {
    pub fn audio_cache_dir(&self) -> PathBuf {
        self.app_data_dir.join("discovery").join("streams")
    }

    pub fn audio_cache_path(&self, release_id: &str, track_position: i32) -> PathBuf {
        self.audio_cache_dir()
            .join(format!("{release_id}_{track_position}"))
    }

    /// Check if audio bytes are cached on disk for a specific track.
    /// Returns `(content_type, file_size)` if cached. Playback-hot path (proxy + the
    /// `fetch_preview_stream` fast path) — runs on a pooled reader.
    pub fn get_cached_audio_meta(
        &self,
        release_id: &str,
        track_position: i32,
    ) -> Result<Option<(String, i64)>> {
        self.db.read(|conn| {
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
        })
    }

    /// Record that audio was cached to disk.
    pub fn save_audio_cache_entry(
        &self,
        release_id: &str,
        track_position: i32,
        content_type: &str,
        file_size: i64,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO discovery_audio_cache
             (release_id, track_position, content_type, file_size, cached_at, last_accessed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            rusqlite::params![release_id, track_position, content_type, file_size, now],
        )?;

        Ok(())
    }

    /// Bump `last_accessed_at` for a cached track so LRU eviction reflects real playback.
    /// Best-effort: a missing row (never cached) is a silent no-op.
    pub fn touch_audio_cache_access(&self, release_id: &str, track_position: i32) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE discovery_audio_cache SET last_accessed_at = ?3
             WHERE release_id = ?1 AND track_position = ?2",
            rusqlite::params![release_id, track_position, now],
        )?;
        Ok(())
    }

    /// Total cached bytes as tracked in the DB (`SUM(file_size)`). Microseconds vs the
    /// full-directory scan of [`Self::get_audio_cache_total_size`] — this runs after
    /// EVERY cache write, so it must not touch the filesystem.
    fn audio_cache_tracked_size(&self) -> Result<i64> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        Ok(conn.query_row(
            "SELECT COALESCE(SUM(file_size), 0) FROM discovery_audio_cache",
            [],
            |r| r.get(0),
        )?)
    }

    /// Evict least-recently-accessed cached tracks until the total on-disk audio cache is
    /// under the configurable cap (`discovery_audio_cache_limit_mb`, default 500 MB). Called
    /// after each successful cache write. Best-effort: deletes the disk file then its DB row
    /// for each victim, oldest access first.
    pub fn enforce_audio_cache_limit(&self) -> Result<()> {
        let cap = self.cache_limit_bytes("discovery_audio_cache_limit_mb", DEFAULT_AUDIO_CACHE_MB);
        let mut total = self.audio_cache_tracked_size()?;
        if total <= cap {
            return Ok(());
        }

        // Oldest-accessed first, bounded — one enforce pass never needs more victims than
        // this, and an unbounded scan over thousands of rows on every write adds up.
        // NULLs (shouldn't occur post-backfill) sort first so they go early.
        let victims: Vec<(String, i32, i64)> = {
            let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
            let mut stmt = conn.prepare(
                "SELECT release_id, track_position, file_size FROM discovery_audio_cache
                 ORDER BY last_accessed_at ASC LIMIT 50",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        for (release_id, track_position, file_size) in victims {
            if total <= cap {
                break;
            }
            let path = self.audio_cache_path(&release_id, track_position);
            if let Err(e) = std::fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("Failed to evict cached audio file {}: {e}", path.display());
                }
            }
            if let Ok(conn) = self.conn.lock() {
                let _ = conn.execute(
                    "DELETE FROM discovery_audio_cache WHERE release_id = ?1 AND track_position = ?2",
                    rusqlite::params![release_id, track_position],
                );
            }
            total -= file_size;
        }

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

    /// Per-release cache state for the "downloaded for offline" indicator:
    /// `(cached_tracks, total_tracks, bytes)`. A release is fully offline-ready when
    /// `total_tracks > 0 && cached_tracks >= total_tracks`.
    pub fn get_release_cache_state(&self, release_id: &str) -> Result<(i64, i64, i64)> {
        self.db.read(|conn| {
            let total_tracks: i64 = conn.query_row(
                "SELECT COUNT(*) FROM discovery_tracks WHERE release_id = ?1",
                [release_id],
                |r| r.get(0),
            )?;
            let (cached_tracks, bytes): (i64, i64) = conn.query_row(
                "SELECT COUNT(*), COALESCE(SUM(file_size), 0) FROM discovery_audio_cache
                 WHERE release_id = ?1",
                [release_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            Ok((cached_tracks, total_tracks, bytes))
        })
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

        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        conn.execute("DELETE FROM discovery_audio_cache", [])?;

        Ok(())
    }
}
