mod audio_cache;
mod release_crud;
mod release_ops;
mod stream_cache;

pub mod metadata;
pub mod n_transform;
pub mod streams;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

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
    conn: Arc<Mutex<Connection>>,
    artwork_service: ArtworkService,
    app_data_dir: PathBuf,
}

impl DiscoveryService {
    pub fn new(conn: Arc<Mutex<Connection>>, app_data_dir: PathBuf) -> Self {
        let artwork_service = ArtworkService::new(app_data_dir.clone());

        let streams_dir = app_data_dir.join("discovery").join("streams");
        if let Err(e) = std::fs::create_dir_all(&streams_dir) {
            log::warn!("Failed to create audio cache directory: {e}");
        }

        Self {
            conn,
            artwork_service,
            app_data_dir,
        }
    }

    pub fn app_data_dir(&self) -> PathBuf {
        self.app_data_dir.clone()
    }

    /// Get a clone of the database connection Arc for use in background tasks.
    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
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
