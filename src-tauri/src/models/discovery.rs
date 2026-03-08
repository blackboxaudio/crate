use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryTrack {
    pub id: String,
    pub release_id: String,
    pub name: String,
    pub position: i32,
    pub duration_ms: Option<i64>,
    pub video_id: Option<String>,
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
    pub date_added: String,
    pub date_modified: String,
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
    pub page_artist: Option<String>,
    pub page_label: Option<String>,
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
