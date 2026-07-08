use super::*;
use image::{imageops::FilterType, ImageFormat};

/// Default cap (in MB) on the on-disk artwork cache when the user hasn't set one. The
/// effective cap is read from the `discovery_artwork_cache_limit_mb` setting (device-local),
/// so once exceeded the least-recently-shown covers are evicted (LRU) after each cache write.
const DEFAULT_ARTWORK_CACHE_MB: i64 = 250;

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
        self.artwork_cache_dir()
            .join(format!("{release_id}.{ext}"))
    }

    /// `(ext, file_size)` if a cover is cached on disk for this release.
    pub fn get_cached_artwork_meta(&self, release_id: &str) -> Result<Option<(String, i64)>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

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
    }

    /// Record that a cover was cached to disk (sets `cached_at` + `last_accessed_at` to now).
    pub fn save_artwork_cache_entry(&self, release_id: &str, ext: &str, file_size: i64) -> Result<()> {
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

    /// Evict least-recently-accessed covers until the total on-disk artwork cache is under the
    /// configurable cap (`discovery_artwork_cache_limit_mb`, default 250 MB). Called after each
    /// successful cache write. Best-effort: deletes the disk file then its DB row for each
    /// victim, oldest access first.
    pub fn enforce_artwork_cache_limit(&self) -> Result<()> {
        let cap = self.cache_limit_bytes("discovery_artwork_cache_limit_mb", DEFAULT_ARTWORK_CACHE_MB);
        let mut total = self.get_artwork_cache_total_size()?;
        if total <= cap {
            return Ok(());
        }

        // Oldest-accessed first.
        let victims: Vec<(String, String, i64)> = {
            let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
            let mut stmt = conn.prepare(
                "SELECT release_id, ext, file_size FROM discovery_artwork_cache
                 ORDER BY last_accessed_at ASC",
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
        // Fast path: already cached.
        if let Some((ext, _)) = self.get_cached_artwork_meta(release_id)? {
            let _ = self.touch_artwork_cache_access(release_id);
            return Ok(Some(self.artwork_cache_rel_path(release_id, &ext)));
        }

        // Look up the remote URL (skip silently if the release is gone or has none).
        let artwork_url: Option<String> = {
            let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
            match conn.query_row(
                "SELECT artwork_url FROM discovery_releases WHERE id = ?1",
                [release_id],
                |row| row.get::<_, Option<String>>(0),
            ) {
                Ok(v) => v,
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(CrateError::Database(e)),
            }
        };
        let Some(artwork_url) = artwork_url.filter(|u| !u.is_empty()) else {
            return Ok(None);
        };

        // Fetch the bytes server-side (JS can't persist to app-data, and the WebView CSP
        // `connect-src` won't allow the fetch).
        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Artwork client build failed: {e}");
                return Ok(None);
            }
        };
        let resp = match client.get(&artwork_url).send().await {
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
        let encoded = image::load_from_memory(&bytes).ok().and_then(|img| {
            let img = if img.width() > 500 || img.height() > 500 {
                img.resize(500, 500, FilterType::Lanczos3)
            } else {
                img
            };
            img.save_with_format(self.artwork_cache_path(release_id, "webp"), ImageFormat::WebP)
                .ok()
        });

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
