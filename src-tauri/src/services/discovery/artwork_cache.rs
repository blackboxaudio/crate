use super::*;
use image::{imageops::FilterType, ImageFormat};

/// Default cap (in MB) on the on-disk artwork cache when the user hasn't set one. The
/// effective cap is read from the `discovery_artwork_cache_limit_mb` setting (device-local),
/// so once exceeded the least-recently-shown covers are evicted (LRU) after each cache write.
const DEFAULT_ARTWORK_CACHE_MB: i64 = 250;

/// Edge (px) of the small thumbnail written alongside each 500px cached cover. The mobile feed
/// renders covers in 44–48 CSS px slots (~144 device px at 3×), so decoding the full 500px WEBP
/// per visible row wastes ~12× the pixels — and the decode cost lands exactly while the list is
/// scrolling. Thumbs are derived files (not tracked in `discovery_artwork_cache`): they live in
/// a `thumbs/` subdirectory keyed by release id, are regenerated on demand from the cached full
/// cover, and are removed together with it.
const THUMB_SIZE: u32 = 160;

impl DiscoveryService {
    pub fn artwork_cache_dir(&self) -> PathBuf {
        self.app_data_dir.join("discovery").join("artwork")
    }

    /// Relative path stored in the DB and returned to the frontend, e.g.
    /// "discovery/artwork/{release_id}.webp". Relative so the frontend composes it with the
    /// app-data dir via `convertFileSrc` (same shape as `artwork_path`). Pure string work —
    /// safe to call while the DB connection is locked.
    pub fn artwork_cache_rel_path(&self, release_id: &str, ext: &str) -> String {
        format!("discovery/artwork/{release_id}.{ext}")
    }

    pub fn artwork_cache_path(&self, release_id: &str, ext: &str) -> PathBuf {
        self.artwork_cache_dir().join(format!("{release_id}.{ext}"))
    }

    /// Thumbnails always encode as WEBP regardless of the full cover's extension (they only
    /// exist when a decode succeeded). The frontend derives this path from the full cover's
    /// cache path by convention — see `discoveryArtworkThumbPath` in `shared/utils/artwork.ts`.
    pub fn artwork_thumb_path(&self, release_id: &str) -> PathBuf {
        self.artwork_cache_dir()
            .join("thumbs")
            .join(format!("{release_id}.webp"))
    }

    /// Backfill the small thumbnail from an already-cached full cover (best-effort). Exists for
    /// covers cached before thumbnails shipped: the frontend calls `cache_release_artwork` when
    /// its thumb 404s, which lands on the idempotent fast path and heals the thumb here.
    async fn ensure_artwork_thumb(&self, release_id: &str, ext: &str) {
        let thumb_path = self.artwork_thumb_path(release_id);
        if thumb_path.exists() {
            return;
        }
        let full_path = self.artwork_cache_path(release_id, ext);
        let result = tokio::task::spawn_blocking(move || {
            if let Some(dir) = thumb_path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let img = image::open(&full_path)
                .map_err(|e| std::io::Error::other(format!("decode failed: {e}")))?;
            img.thumbnail(THUMB_SIZE, THUMB_SIZE)
                .save_with_format(&thumb_path, ImageFormat::WebP)
                .map_err(|e| std::io::Error::other(format!("encode failed: {e}")))
        })
        .await;
        match result {
            Ok(Err(e)) => log::warn!("Artwork thumb backfill for {release_id} failed: {e}"),
            Err(e) => log::warn!("Artwork thumb backfill task for {release_id} failed: {e}"),
            Ok(Ok(())) => {}
        }
    }

    /// `(ext, file_size)` if a cover is cached on disk for this release. Called per
    /// visible row while scrolling — runs on a pooled reader.
    pub fn get_cached_artwork_meta(&self, release_id: &str) -> Result<Option<(String, i64)>> {
        self.db.read(|conn| {
            let result = conn.query_row(
                "SELECT ext, file_size FROM discovery_artwork_cache WHERE release_id = ?1",
                [release_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            );

            match result {
                Ok(meta) => Ok(Some(meta)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(CrateError::Database(e)),
            }
        })
    }

    /// Record that a cover was cached to disk (sets `cached_at` + `last_accessed_at` to now).
    pub fn save_artwork_cache_entry(
        &self,
        release_id: &str,
        ext: &str,
        file_size: i64,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO discovery_artwork_cache
             (release_id, ext, file_size, cached_at, last_accessed_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            rusqlite::params![release_id, ext, file_size, now],
        )?;

        Ok(())
    }

    /// Bump `last_accessed_at` for a cached cover so LRU eviction reflects real display.
    /// Best-effort: a missing row (never cached) is a silent no-op.
    pub fn touch_artwork_cache_access(&self, release_id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE discovery_artwork_cache SET last_accessed_at = ?2 WHERE release_id = ?1",
            rusqlite::params![release_id, now],
        )?;
        Ok(())
    }

    /// Total cached bytes as tracked in the DB (`SUM(file_size)`). Microseconds vs the
    /// full-directory scan of [`Self::get_artwork_cache_total_size`] — this runs after
    /// EVERY cover write (i.e. while scrolling an uncached feed), so it must not touch
    /// the filesystem.
    fn artwork_cache_tracked_size(&self) -> Result<i64> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        Ok(conn.query_row(
            "SELECT COALESCE(SUM(file_size), 0) FROM discovery_artwork_cache",
            [],
            |r| r.get(0),
        )?)
    }

    /// Evict least-recently-accessed covers until the total on-disk artwork cache is under the
    /// configurable cap (`discovery_artwork_cache_limit_mb`, default 250 MB). Called after each
    /// successful cache write. Best-effort: deletes the disk file then its DB row for each
    /// victim, oldest access first.
    pub fn enforce_artwork_cache_limit(&self) -> Result<()> {
        let cap =
            self.cache_limit_bytes("discovery_artwork_cache_limit_mb", DEFAULT_ARTWORK_CACHE_MB);
        let mut total = self.artwork_cache_tracked_size()?;
        if total <= cap {
            return Ok(());
        }

        // Oldest-accessed first, bounded — one enforce pass never needs more victims
        // than this (covers are small), and an unbounded full-table scan per write
        // amplifies exactly the scroll that triggers it.
        let victims: Vec<(String, String, i64)> = {
            let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
            let mut stmt = conn.prepare(
                "SELECT release_id, ext, file_size FROM discovery_artwork_cache
                 ORDER BY last_accessed_at ASC LIMIT 50",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        for (release_id, ext, file_size) in victims {
            if total <= cap {
                break;
            }
            let path = self.artwork_cache_path(&release_id, &ext);
            if let Err(e) = std::fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("Failed to evict cached artwork {}: {e}", path.display());
                }
            }
            let _ = std::fs::remove_file(self.artwork_thumb_path(&release_id));
            if let Ok(conn) = self.conn.lock() {
                let _ = conn.execute(
                    "DELETE FROM discovery_artwork_cache WHERE release_id = ?1",
                    [release_id.as_str()],
                );
            }
            total -= file_size;
        }

        Ok(())
    }

    /// Delete the cached cover file and DB row for a single release. Best-effort (called on
    /// release delete and when a release's `artwork_url` changes so a stale cover re-downloads).
    pub fn delete_cached_artwork_file(&self, release_id: &str) -> Result<()> {
        // Resolve the stored extension so the right file is removed (may be jpg/png if the
        // WEBP re-encode was skipped). Defaults to webp if the row is already gone.
        let ext = self
            .get_cached_artwork_meta(release_id)
            .ok()
            .flatten()
            .map(|(ext, _)| ext)
            .unwrap_or_else(|| "webp".to_string());

        let path = self.artwork_cache_path(release_id, &ext);
        if let Err(e) = std::fs::remove_file(&path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                log::warn!("Failed to delete cached artwork {}: {e}", path.display());
            }
        }
        let _ = std::fs::remove_file(self.artwork_thumb_path(release_id));

        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "DELETE FROM discovery_artwork_cache WHERE release_id = ?1",
                [release_id],
            );
        }

        Ok(())
    }

    /// Total size of all cached cover files in bytes (calculated from disk).
    pub fn get_artwork_cache_total_size(&self) -> Result<i64> {
        let cache_dir = self.artwork_cache_dir();
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

    /// Delete all cached cover files from disk and clear the DB table.
    pub fn clear_artwork_cache(&self) -> Result<()> {
        let cache_dir = self.artwork_cache_dir();
        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    if let Err(e) = std::fs::remove_file(entry.path()) {
                        log::warn!(
                            "Failed to delete cached artwork {}: {e}",
                            entry.path().display()
                        );
                    }
                }
            }
        }
        // Derived thumbnails live in a subdirectory the file loop above doesn't descend into.
        if let Err(e) = std::fs::remove_dir_all(cache_dir.join("thumbs")) {
            if e.kind() != std::io::ErrorKind::NotFound {
                log::warn!("Failed to clear artwork thumbnails: {e}");
            }
        }

        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        conn.execute("DELETE FROM discovery_artwork_cache", [])?;

        Ok(())
    }

    /// Download-to-disk workhorse for offline album art. Idempotent: if a cover is already
    /// cached, just bumps its access time and returns the relative path. Otherwise fetches the
    /// release's remote `artwork_url` via reqwest, re-encodes to 500x500 WEBP (falling back to
    /// the original bytes if decode/encode fails), writes it under `discovery/artwork/`, records
    /// the entry, and enforces the cache cap. Returns the relative cache path, or `None` when the
    /// release has no `artwork_url` or the fetch fails — the UI then falls back to the remote URL.
    pub async fn cache_release_artwork(&self, release_id: &str) -> Result<Option<String>> {
        // Fast path: already cached. Heals a missing thumbnail on the way (covers cached before
        // thumbnails existed).
        if let Some((ext, _)) = self.get_cached_artwork_meta(release_id)? {
            let _ = self.touch_artwork_cache_access(release_id);
            self.ensure_artwork_thumb(release_id, &ext).await;
            return Ok(Some(self.artwork_cache_rel_path(release_id, &ext)));
        }

        // Dedup: if this release's download is already in flight, soft-fail (`None`) like the
        // other failure paths — the UI keeps showing the remote URL and picks up the cached copy
        // on a later load.
        {
            let mut in_flight = self
                .artwork_in_flight
                .lock()
                .map_err(|_| CrateError::LockPoisoned)?;
            if !in_flight.insert(release_id.to_string()) {
                return Ok(None);
            }
        }

        let result = self.download_and_cache_artwork(release_id).await;

        if let Ok(mut in_flight) = self.artwork_in_flight.lock() {
            in_flight.remove(release_id);
        }

        result
    }

    /// Body of [`Self::cache_release_artwork`], separated so the in-flight entry is always
    /// removed regardless of which path returns. Callers must hold the in-flight entry.
    async fn download_and_cache_artwork(&self, release_id: &str) -> Result<Option<String>> {
        // Cap concurrent download+decode work. `acquire` only errs if the semaphore is closed,
        // which never happens here.
        let Ok(_permit) = self.artwork_fetch_permits.acquire().await else {
            return Ok(None);
        };

        // Re-check after waiting on the permit — the cover may have been cached meanwhile (e.g.
        // by an explicit offline-download while this request sat in the queue).
        if let Some((ext, _)) = self.get_cached_artwork_meta(release_id)? {
            let _ = self.touch_artwork_cache_access(release_id);
            self.ensure_artwork_thumb(release_id, &ext).await;
            return Ok(Some(self.artwork_cache_rel_path(release_id, &ext)));
        }

        // Look up the remote URL (skip silently if the release is gone or has none).
        let artwork_url: Option<String> = self.db.read(|conn| {
            match conn.query_row(
                "SELECT artwork_url FROM discovery_releases WHERE id = ?1",
                [release_id],
                |row| row.get::<_, Option<String>>(0),
            ) {
                Ok(v) => Ok(v),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(CrateError::Database(e)),
            }
        })?;
        let Some(artwork_url) = artwork_url.filter(|u| !u.is_empty()) else {
            return Ok(None);
        };

        // Fetch the bytes server-side (JS can't persist to app-data, and the WebView CSP
        // `connect-src` won't allow the fetch).
        let resp = match self.artwork_http.get(&artwork_url).send().await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                log::warn!("Artwork fetch for {release_id} returned {}", r.status());
                return Ok(None);
            }
            Err(e) => {
                log::warn!("Artwork fetch for {release_id} failed: {e}");
                return Ok(None);
            }
        };
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let bytes = match resp.bytes().await {
            Ok(b) => b.to_vec(),
            Err(e) => {
                log::warn!("Artwork read for {release_id} failed: {e}");
                return Ok(None);
            }
        };

        let cache_dir = self.artwork_cache_dir();
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            log::warn!("Failed to create artwork cache dir: {e}");
            return Ok(None);
        }

        // Preferred path: decode + resize to 500x500 + save as WEBP (mirrors ArtworkService,
        // which is proven cross-platform). On any failure, fall back to the original bytes.
        // Decode/resize/encode is CPU-bound, so it runs on the blocking pool instead of stalling
        // the async runtime while several covers cache concurrently. The small feed thumbnail is
        // cut from the same decode (best-effort — a missing thumb just means the frontend falls
        // back to the full cover and re-requests the thumb, which heals via `ensure_artwork_thumb`).
        let webp_path = self.artwork_cache_path(release_id, "webp");
        let thumb_path = self.artwork_thumb_path(release_id);
        let (bytes, encoded) = match tokio::task::spawn_blocking(move || {
            let encoded = image::load_from_memory(&bytes).ok().and_then(|img| {
                let thumb = img.thumbnail(THUMB_SIZE, THUMB_SIZE);
                let img = if img.width() > 500 || img.height() > 500 {
                    img.resize(500, 500, FilterType::Lanczos3)
                } else {
                    img
                };
                let saved = img.save_with_format(&webp_path, ImageFormat::WebP).ok();
                if saved.is_some() {
                    if let Some(dir) = thumb_path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    let _ = thumb.save_with_format(&thumb_path, ImageFormat::WebP);
                }
                saved
            });
            (bytes, encoded)
        })
        .await
        {
            Ok(v) => v,
            Err(e) => {
                log::warn!("Artwork encode task for {release_id} failed: {e}");
                return Ok(None);
            }
        };

        let (ext, file_size) = if encoded.is_some() {
            let path = self.artwork_cache_path(release_id, "webp");
            let size = std::fs::metadata(&path)
                .map(|m| m.len() as i64)
                .unwrap_or(bytes.len() as i64);
            ("webp".to_string(), size)
        } else {
            let ext = ext_from_content_type(content_type.as_deref());
            let path = self.artwork_cache_path(release_id, &ext);
            if let Err(e) = std::fs::write(&path, &bytes) {
                log::warn!("Failed to write cached artwork {}: {e}", path.display());
                return Ok(None);
            }
            (ext, bytes.len() as i64)
        };

        self.save_artwork_cache_entry(release_id, &ext, file_size)?;
        if let Err(e) = self.enforce_artwork_cache_limit() {
            log::warn!("Artwork cache eviction failed: {e}");
        }

        Ok(Some(self.artwork_cache_rel_path(release_id, &ext)))
    }
}

/// Map a `Content-Type` header to a file extension for the raw-bytes fallback path.
fn ext_from_content_type(content_type: Option<&str>) -> String {
    match content_type {
        Some(ct) if ct.contains("png") => "png",
        Some(ct) if ct.contains("webp") => "webp",
        Some(ct) if ct.contains("gif") => "gif",
        _ => "jpg",
    }
    .to_string()
}
