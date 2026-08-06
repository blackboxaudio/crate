use super::*;

impl DiscoveryService {
    /// Get a cached stream for a specific track position, if it exists and hasn't expired.
    /// Playback-hot path (called by the proxy per request) — runs on a pooled reader so
    /// it never queues behind a sync merge or cache write on the writer.
    pub fn get_cached_stream(
        &self,
        release_id: &str,
        track_position: i32,
    ) -> Result<Option<CachedStream>> {
        self.db.read(|conn| {
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
        })
    }

    /// Cache stream URLs for a release, replacing any existing entries. One transaction
    /// for the whole batch — a per-row implicit commit would fsync once per track while
    /// holding the shared connection lock.
    pub fn cache_streams(&self, release_id: &str, streams: &[StreamInfo]) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let tx = conn.unchecked_transaction()?;
        for stream in streams {
            tx.execute(
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
        tx.commit()?;

        Ok(())
    }

    /// Replace the device-local preview-availability rows for a release. `unavailable`
    /// holds the track positions the source currently serves no stream for (e.g. the
    /// unreleased tracks of a Bandcamp pre-order); an empty slice clears the release.
    /// Called after every successful stream extraction, so flags stay in step with the
    /// source and a pre-order self-heals once the album is released.
    pub fn set_preview_availability(&self, release_id: &str, unavailable: &[i32]) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM discovery_preview_unavailable WHERE release_id = ?1",
            [release_id],
        )?;
        let now = chrono::Utc::now().to_rfc3339();
        for position in unavailable {
            tx.execute(
                "INSERT INTO discovery_preview_unavailable (release_id, position, checked_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![release_id, position, now],
            )?;
        }
        tx.commit()?;

        Ok(())
    }

    /// Get the cached SoundCloud client_id, if one exists and was fetched within the last 24 hours.
    pub fn get_cached_sc_client_id(&self) -> Result<Option<String>> {
        self.db.read(|conn| {
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
        })
    }

    /// Invalidate a release's cached stream URLs (URL-level only), forcing re-resolution on next
    /// play. Deliberately never touches on-disk audio bytes: the playback-error auto-retry calls
    /// this, and a transient/offline error must not destroy a downloaded copy. Use
    /// [`Self::purge_release_audio`] when the bytes themselves should go.
    pub fn invalidate_stream_cache(&self, release_id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        conn.execute(
            "DELETE FROM discovery_stream_cache WHERE release_id = ?1",
            [release_id],
        )?;

        Ok(())
    }

    /// Remove a release's downloaded audio bytes AND its cached stream URLs. This is the
    /// destructive path ("Remove Download" / corrupt-content recovery) — the transient-error
    /// retry must use [`Self::invalidate_stream_cache`] instead.
    pub fn purge_release_audio(&self, release_id: &str) -> Result<()> {
        self.delete_cached_audio_files(release_id)?;
        self.invalidate_stream_cache(release_id)
    }

    /// Cache a SoundCloud client_id.
    pub fn cache_sc_client_id(&self, client_id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO discovery_sc_client_id_cache (id, client_id, fetched_at) VALUES (1, ?1, ?2)",
            rusqlite::params![client_id, now],
        )?;

        Ok(())
    }
}
