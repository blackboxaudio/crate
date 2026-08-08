use serde::{Deserialize, Serialize};

/// Fixed namespace for content-derived discovery-track ids (arbitrary; never change it —
/// every device must mint identical ids from identical inputs, forever).
pub const DISCOVERY_TRACK_ID_NAMESPACE: uuid::Uuid =
    uuid::Uuid::from_u128(0x5f1e_d0aa_9c3b_42d7_8a6e_2b91_c4f7_03d5);

/// Fixed namespace for content-derived discovery-release ids (same never-change rule).
pub const DISCOVERY_RELEASE_ID_NAMESPACE: uuid::Uuid =
    uuid::Uuid::from_u128(0x7c42_d9be_51f0_4aa3_9b0d_d6f8_e2a4_1c77);

/// Resolved playback endpoint for one track: the localhost proxy URL, or — iOS with the
/// audio fully cached on disk — a direct `file://` URL into the audio cache together with
/// the cached file's recorded MIME type (the cache files are extensionless, so AVPlayer
/// needs the type out-of-band).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewStream {
    pub url: String,
    pub mime_type: Option<String>,
}

/// Canonical track-name normalization for identity and dedup: trim + Unicode lowercase.
/// Every producer and consumer (id minting, insert-time dedup, sync merge matching, the
/// startup dedupe sweep) MUST use this — SQL `LOWER()` is ASCII-only and would disagree
/// with it on non-ASCII names.
pub fn normalized_track_name(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Deterministic track id: UUIDv5 over `release_id|normalized_name`. Two devices that
/// independently fetch the same release mint IDENTICAL ids, so the sync merge's
/// `ON CONFLICT(id)` path collapses them instead of unioning duplicate rows.
pub fn deterministic_track_id(release_id: &str, name: &str) -> String {
    uuid::Uuid::new_v5(
        &DISCOVERY_TRACK_ID_NAMESPACE,
        format!("{release_id}|{}", normalized_track_name(name)).as_bytes(),
    )
    .to_string()
}

/// Deterministic release id: UUIDv5 over the normalized URL (the release's natural key —
/// `discovery_releases.url` is UNIQUE). Two devices that independently add the same URL
/// mint the same id, so cloud sync converges on one row instead of skipping each other's
/// copy on the UNIQUE(url) collision and splitting the release id across the fleet.
pub fn deterministic_release_id(normalized_url: &str) -> String {
    uuid::Uuid::new_v5(&DISCOVERY_RELEASE_ID_NAMESPACE, normalized_url.as_bytes()).to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryTrack {
    pub id: String,
    pub release_id: String,
    pub name: String,
    pub position: i32,
    pub duration_ms: Option<i64>,
    pub video_id: Option<String>,
    /// The track's own page URL (Bandcamp track page, SoundCloud permalink) when the source
    /// provides one; `None` otherwise — share/copy consumers fall back to the release URL.
    /// `serde(default)` keeps payloads from older peers/backups deserializable.
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub is_liked: bool,
    /// The source currently serves no preview stream for this track (e.g. an unreleased
    /// track on a Bandcamp pre-order). Populated on read from the device-local
    /// `discovery_preview_unavailable` table — never synced; peers see `false`.
    #[serde(default)]
    pub preview_unavailable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryRelease {
    pub id: String,
    pub url: String,
    pub source_type: String,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub label: Option<String>,
    pub release_date: Option<String>,
    pub artwork_url: Option<String>,
    pub artwork_path: Option<String>,
    /// Relative path to the on-disk cached remote cover ("discovery/artwork/{id}.ext"), or
    /// `None` when not yet cached. Populated on read from `discovery_artwork_cache`; drives
    /// cache-first (offline) artwork rendering on mobile. Device-local — never synced.
    #[serde(default)]
    pub artwork_cache_path: Option<String>,
    pub notes: Option<String>,
    pub parent_url: Option<String>,
    /// The artist/label page this release was discovered from (the scanned page, or a
    /// followed source's URL). Drives one-click "follow this source" so following a
    /// label matches all its releases — even when they live on separate artist
    /// subdomains (the Bandcamp norm). Synced.
    #[serde(default)]
    pub source_page_url: Option<String>,
    pub date_added: String,
    pub date_modified: String,
    /// "New/unreviewed" flag — set when a followed source surfaces the release,
    /// cleared on preview-play or a decisive action. Synced.
    #[serde(default)]
    pub is_new: bool,
    /// When the watcher surfaced this release (RFC3339); `None` for manually-added.
    #[serde(default)]
    pub surfaced_at: Option<String>,
    /// Provenance: ids of the followed sources that surfaced this release (may be
    /// two — an artist follow and a label follow). Empty for manually-added.
    #[serde(default)]
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub tracks: Vec<DiscoveryTrack>,
    #[serde(default)]
    pub tags: Vec<super::Tag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryReleaseCreate {
    pub url: String,
    pub source_type: Option<String>,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub label: Option<String>,
    pub release_date: Option<String>,
    pub artwork_url: Option<String>,
    pub notes: Option<String>,
    pub parent_url: Option<String>,
    #[serde(default)]
    pub source_page_url: Option<String>,
    pub tracks: Option<Vec<DiscoveryTrackCreate>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryTrackCreate {
    pub name: String,
    pub position: i32,
    pub duration_ms: Option<i64>,
    pub video_id: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscoveryReleaseUpdate {
    pub artist: Option<String>,
    pub title: Option<String>,
    pub label: Option<String>,
    pub release_date: Option<String>,
    pub artwork_url: Option<String>,
    pub artwork_path: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscoveryFilter {
    pub search: Option<String>,
    pub tag_ids: Option<Vec<String>>,
    pub tag_filter_mode: Option<String>,
    /// Page size for chunked loading. `None` returns the full set (legacy behavior). The
    /// frontend loads large libraries in pages so no single IPC response carries thousands of
    /// releases at once (a multi-MB payload parsed in one shot can OOM the mobile webview).
    pub limit: Option<u32>,
    /// Row offset for chunked loading; only meaningful together with `limit`.
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedRelease {
    pub url: String,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub artwork_url: Option<String>,
    pub release_date: Option<String>,
    pub already_exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedPage {
    pub source_type: String,
    /// Canonical followable page URL (Bandcamp subdomain origin, SoundCloud profile,
    /// Discogs entity page) for the scanned page; stamped onto imported releases as
    /// `source_page_url` so a label follow matches them all.
    #[serde(default)]
    pub page_url: Option<String>,
    pub page_artist: Option<String>,
    pub page_label: Option<String>,
    /// Profile picture for the artist/label page (Bandcamp og:image, SoundCloud
    /// user avatar, Discogs entity image) — used as the followed source's artwork.
    #[serde(default)]
    pub avatar_url: Option<String>,
    pub releases: Vec<ScannedRelease>,
    pub total_found: usize,
    pub already_in_discovery: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkImportProgress {
    pub current: usize,
    pub total: usize,
    pub current_title: Option<String>,
    pub succeeded: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanPageProgress {
    pub current_page: u32,
    pub total_pages: Option<u32>,
    pub releases_found: usize,
    pub entity_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkImportResult {
    pub succeeded: usize,
    pub failed: usize,
    pub failed_urls: Vec<String>,
}
