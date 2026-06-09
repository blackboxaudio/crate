use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryTrack {
    pub id: String,
    pub release_id: String,
    pub name: String,
    pub position: i32,
    pub duration_ms: Option<i64>,
    pub video_id: Option<String>,
    #[serde(default)]
    pub is_liked: bool,
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
