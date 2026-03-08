use super::*;

impl DiscoveryService {
    /// Get a cached stream for a specific track position, if it exists and hasn't expired.
    pub fn get_cached_stream(
        &self,
        release_id: &str,
        track_position: i32,
    ) -> Result<Option<CachedStream>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();
        let result = conn.query_row(
            "SELECT stream_url, proxy_ua FROM discovery_stream_cache
             WHERE release_id = ?1 AND track_position = ?2 AND expires_at > ?3",
            rusqlite::params![release_id, track_position, now],
            |row| {
                Ok(CachedStream {
                    stream_url: row.get(0)?,
                    proxy_ua: row.get(1)?,
                })
            },
        );

        match result {
            Ok(cached) => Ok(Some(cached)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CrateError::Database(e)),
        }
    }

    /// Cache stream URLs for a release, replacing any existing entries.
    pub fn cache_streams(&self, release_id: &str, streams: &[StreamInfo]) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        for stream in streams {
            conn.execute(
                "INSERT OR REPLACE INTO discovery_stream_cache (release_id, track_position, stream_url, expires_at, proxy_ua)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    release_id,
                    stream.track_position,
                    stream.stream_url,
                    stream.expires_at,
                    stream.proxy_ua,
                ],
            )?;
        }

        Ok(())
    }

    /// Get the cached SoundCloud client_id, if one exists and was fetched within the last 24 hours.
    pub fn get_cached_sc_client_id(&self) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let cutoff = (chrono::Utc::now() - chrono::Duration::hours(24)).to_rfc3339();
        let result = conn.query_row(
            "SELECT client_id FROM discovery_sc_client_id_cache WHERE id = 1 AND fetched_at > ?1",
            rusqlite::params![cutoff],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(cid) => Ok(Some(cid)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CrateError::Database(e)),
        }
    }

    /// Invalidate cached stream URLs and audio files for a release, forcing re-fetch on next play.
    pub fn invalidate_stream_cache(&self, release_id: &str) -> Result<()> {
        // Also clear disk-cached audio so the retry re-downloads fresh
        if let Err(e) = self.delete_cached_audio_files(release_id) {
            log::warn!("Failed to clean up cached audio during invalidation for {release_id}: {e}");
        }

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        conn.execute(
            "DELETE FROM discovery_stream_cache WHERE release_id = ?1",
            [release_id],
        )?;

        Ok(())
    }

    /// Cache a SoundCloud client_id.
    pub fn cache_sc_client_id(&self, client_id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO discovery_sc_client_id_cache (id, client_id, fetched_at) VALUES (1, ?1, ?2)",
            rusqlite::params![client_id, now],
        )?;

        Ok(())
    }
}
