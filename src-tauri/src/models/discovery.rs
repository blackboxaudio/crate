use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryTrack {
    pub id: String,
    pub release_id: String,
    pub name: String,
    pub position: i32,
    pub duration_ms: Option<i64>,
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
    pub tracks: Option<Vec<DiscoveryTrackCreate>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryTrackCreate {
    pub name: String,
    pub position: i32,
    pub duration_ms: Option<i64>,
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
