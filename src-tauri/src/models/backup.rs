use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{Cue, DiscoveryRelease, DiscoveryTrack, Playlist, PlaylistTrack, Tag, TagCategory};

/// A track as stored in a backup — same as Track but without waveform_data
/// (too large; user re-analyzes after restore) and without the `tags` Vec
/// (tag associations are stored separately in `track_tags`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupTrack {
    pub id: String,
    pub file_path: String,
    pub file_hash: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub label: Option<String>,
    pub catalog_number: Option<String>,
    pub duration_ms: i64,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub bitrate: Option<i32>,
    pub sample_rate: Option<i32>,
    pub format: Option<String>,
    pub rating: i32,
    pub play_count: i32,
    pub date_added: String,
    pub date_modified: String,
    pub last_played: Option<String>,
    pub rekordbox_id: Option<String>,
    pub artwork_path: Option<String>,
    pub artwork_source: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupTrackTag {
    pub track_id: String,
    pub tag_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupDiscoveryReleaseTag {
    pub release_id: String,
    pub tag_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPlaylistDiscoveryRelease {
    pub playlist_id: String,
    pub release_id: String,
    pub position: i32,
    pub date_added: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupCounts {
    pub tracks: usize,
    pub cues: usize,
    pub tag_categories: usize,
    pub tags: usize,
    pub playlists: usize,
    pub discovery_releases: usize,
    #[serde(default)]
    pub artwork_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupData {
    pub version: u32,
    pub app_version: String,
    pub created_at: String,
    pub counts: BackupCounts,
    pub tag_categories: Vec<TagCategory>,
    pub tags: Vec<Tag>,
    pub tracks: Vec<BackupTrack>,
    pub cues: Vec<Cue>,
    pub track_tags: Vec<BackupTrackTag>,
    pub playlists: Vec<Playlist>,
    pub playlist_tracks: Vec<PlaylistTrack>,
    pub discovery_releases: Vec<DiscoveryRelease>,
    pub discovery_tracks: Vec<DiscoveryTrack>,
    pub discovery_release_tags: Vec<BackupDiscoveryReleaseTag>,
    pub playlist_discovery_releases: Vec<BackupPlaylistDiscoveryRelease>,
    /// Base64-encoded artwork files keyed by relative path (e.g. "artwork/abc.webp").
    /// `None` for backups created before artwork support was added.
    #[serde(default)]
    pub artwork_files: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub version: u32,
    pub app_version: String,
    pub created_at: String,
    pub counts: BackupCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BackupStatus {
    Pending,
    ReadingData,
    CollectingArtwork,
    WritingFile,
    RestoringData,
    RestoringArtwork,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupProgress {
    pub status: BackupStatus,
}
