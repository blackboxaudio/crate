mod artwork_cache;
mod audio_cache;
mod release_crud;
mod release_ops;
mod stream_cache;

pub mod metadata;
pub mod n_transform;
pub mod streams;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::db::Db;
use crate::error::{CrateError, Result};
use crate::models::{
    DiscoveryFilter, DiscoveryRelease, DiscoveryReleaseCreate, DiscoveryReleaseUpdate,
    DiscoveryTrack, DiscoveryTrackCreate, Tag,
};
use crate::services::cloud_sync::pipeline::{buckets, dirty};
use crate::services::ArtworkService;

use metadata::FetchedTrack;
use streams::StreamInfo;

pub struct CachedStream {
    pub stream_url: String,
    pub proxy_ua: Option<String>,
}

pub struct DiscoveryService {
    /// Read/write-split handle: hot read paths go through `db.read` (pooled snapshot
    /// readers under WAL), writes through the shared writer.
    db: Db,
    /// The legacy writer Arc (`== db.writer()`), kept so write paths and not-yet-
    /// migrated reads stay untouched.
    conn: Arc<Mutex<Connection>>,
    artwork_service: ArtworkService,
    app_data_dir: PathBuf,
    /// Shared client for artwork downloads — building a fresh client (connection pool, TLS
    /// context) per cover fetch is wasteful when the feed requests dozens while scrolling.
    artwork_http: reqwest::Client,
    /// Caps concurrent artwork download+decode work: a fast flick through a large uncached feed
    /// mounts many rows at once, and an unbounded fan-out of fetches and image decodes can
    /// pressure a mobile device into killing the webview process.
    artwork_fetch_permits: tokio::sync::Semaphore,
    /// Release ids with an artwork download currently in flight (dedup — the virtualized feed
    /// can remount a row while its first request is still running).
    artwork_in_flight: Mutex<HashSet<String>>,
}

impl DiscoveryService {
    /// Construct from the legacy writer Arc alone (no reader pool — reads fall back to
    /// the writer mutex). Kept so background construction sites (follow watch, spawned
    /// enrichment tasks) compile unchanged; prefer [`Self::with_db`].
    pub fn new(conn: Arc<Mutex<Connection>>, app_data_dir: PathBuf) -> Self {
        Self::with_db(Db::from_writer(conn), app_data_dir)
    }

    pub fn with_db(db: Db, app_data_dir: PathBuf) -> Self {
        let artwork_service = ArtworkService::new(app_data_dir.clone());

        let streams_dir = app_data_dir.join("discovery").join("streams");
        if let Err(e) = std::fs::create_dir_all(&streams_dir) {
            log::warn!("Failed to create audio cache directory: {e}");
        }

        let artwork_cache_dir = app_data_dir.join("discovery").join("artwork");
        if let Err(e) = std::fs::create_dir_all(&artwork_cache_dir) {
            log::warn!("Failed to create artwork cache directory: {e}");
        }

        let artwork_http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            conn: db.writer(),
            db,
            artwork_service,
            app_data_dir,
            artwork_http,
            artwork_fetch_permits: tokio::sync::Semaphore::new(3),
            artwork_in_flight: Mutex::new(HashSet::new()),
        }
    }

    pub fn app_data_dir(&self) -> PathBuf {
        self.app_data_dir.clone()
    }

    /// Read a device-local cache-size cap (in MB) from the `settings` table, converted to
    /// bytes. Falls back to `default_mb` when the setting is unset, unparseable, or non-positive.
    /// Shared by the audio and artwork LRU sweeps so their caps are user-configurable.
    fn cache_limit_bytes(&self, key: &str, default_mb: i64) -> i64 {
        let mb = self
            .db
            .read(|conn| {
                Ok(conn
                    .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                        row.get::<_, String>(0)
                    })
                    .ok())
            })
            .ok()
            .flatten()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|mb| *mb > 0)
            .unwrap_or(default_mb);
        mb * 1024 * 1024
    }

    /// Get a clone of the database connection Arc for use in background tasks.
    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    /// The read/write-split handle, for spawning sibling services in background tasks
    /// without losing the reader pool.
    pub fn db(&self) -> Db {
        self.db.clone()
    }

    pub fn assign_tags(&self, release_ids: Vec<String>, tag_ids: Vec<String>) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let hlc = dirty::next_hlc(&conn)?;
        for release_id in &release_ids {
            for tag_id in &tag_ids {
                conn.execute(
                    "INSERT OR IGNORE INTO discovery_release_tags (release_id, tag_id, _hlc) VALUES (?1, ?2, ?3)",
                    rusqlite::params![release_id, tag_id, hlc],
                )?;
            }
        }
        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASE_TAGS)?;

        Ok(())
    }

    pub fn remove_tags(&self, release_ids: Vec<String>, tag_ids: Vec<String>) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let hlc = dirty::next_hlc(&conn)?;
        for release_id in &release_ids {
            for tag_id in &tag_ids {
                let deleted = conn.execute(
                    "DELETE FROM discovery_release_tags WHERE release_id = ?1 AND tag_id = ?2",
                    rusqlite::params![release_id, tag_id],
                )?;
                if deleted > 0 {
                    dirty::record_tombstone(
                        &conn,
                        buckets::DISCOVERY_RELEASE_TAGS,
                        &dirty::junction_entity_id(release_id, tag_id),
                        &hlc,
                    )?;
                }
            }
        }
        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASE_TAGS)?;

        Ok(())
    }
}

/// Derive the canonical "followable page" URL from a scanned artist/label page URL,
/// mirroring the frontend `deriveFollowUrl` host logic. Bandcamp → the subdomain
/// origin; SoundCloud → the profile (first path segment); everything else → the
/// normalized URL. Stored on imported releases as `source_page_url` so a label follow
/// matches every release scanned from the page, even those on other artist subdomains.
pub(crate) fn followable_page_url(page_url: &str, source_type: &str) -> Option<String> {
    let normalized = normalize_url(page_url);
    let (scheme, rest) = normalized.split_once("://")?;
    let (authority, path) = match rest.split_once('/') {
        Some((a, p)) => (a, p),
        None => (rest, ""),
    };
    match source_type {
        "bandcamp" => Some(format!("{scheme}://{authority}")),
        "soundcloud" => {
            let seg = path.split('/').find(|s| !s.is_empty())?;
            Some(format!("{scheme}://{authority}/{seg}"))
        }
        _ => Some(normalized),
    }
}

/// Normalize a URL for consistent storage and deduplication.
/// - Lowercases the domain (not path)
/// - Strips trailing slashes
/// - Removes common tracking query parameters
pub(crate) fn normalize_url(url: &str) -> String {
    let url = url.trim();

    // Parse into parts: scheme, domain, path+query
    let (scheme, rest) = match url.find("://") {
        Some(i) => (&url[..i], &url[i + 3..]),
        None => return url.to_string(),
    };

    let (authority, path_and_query) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };

    // Lowercase the authority (domain + optional port)
    let authority_lower = authority.to_lowercase();

    // Split path from query
    let (path, query) = match path_and_query.find('?') {
        Some(i) => (&path_and_query[..i], Some(&path_and_query[i + 1..])),
        None => (path_and_query, None),
    };

    // Strip trailing slashes from path (but keep at least "/")
    let path = path.trim_end_matches('/');
    let path = if path.is_empty() { "" } else { path };

    // Filter out tracking query params
    let tracking_params: &[&str] = &[
        "utm_source",
        "utm_medium",
        "utm_campaign",
        "utm_content",
        "utm_term",
        "ref",
        "fbclid",
        "si",
        "feature",
    ];

    let filtered_query = query
        .map(|q| {
            q.split('&')
                .filter(|param| {
                    let key = param.split('=').next().unwrap_or("");
                    !tracking_params.contains(&key)
                })
                .collect::<Vec<_>>()
                .join("&")
        })
        .filter(|q| !q.is_empty());

    match filtered_query {
        Some(q) => format!("{scheme}://{authority_lower}{path}?{q}"),
        None => format!("{scheme}://{authority_lower}{path}"),
    }
}

pub(crate) fn detect_source_type(url: &str) -> String {
    let url_lower = url.to_lowercase();
    if url_lower.contains("bandcamp.com") {
        "bandcamp".to_string()
    } else if url_lower.contains("soundcloud.com") {
        "soundcloud".to_string()
    } else if url_lower.contains("youtube.com") || url_lower.contains("youtu.be") {
        "youtube".to_string()
    } else if url_lower.contains("discogs.com") {
        "discogs".to_string()
    } else {
        "other".to_string()
    }
}
