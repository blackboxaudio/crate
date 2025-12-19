use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub file_path: String,
    pub file_hash: Option<String>,

    // Metadata
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub label: Option<String>,
    pub catalog_number: Option<String>,

    // Audio properties
    pub duration_ms: i64,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub bitrate: Option<i32>,
    pub sample_rate: Option<i32>,
    pub format: String,

    // Analysis metadata
    pub analysis_source: Option<String>,
    pub waveform_data: Option<Vec<u8>>,

    // User data
    pub rating: i32,
    pub play_count: i32,

    // Timestamps
    pub date_added: String,
    pub date_modified: String,
    pub last_played: Option<String>,

    // External references
    pub rekordbox_id: Option<String>,

    // Album artwork
    pub artwork_path: Option<String>,

    // Tags (populated when fetching tracks)
    #[serde(default)]
    pub tags: Vec<super::Tag>,
}

impl Track {
    pub fn new(file_path: String, format: String, duration_ms: i64) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: Uuid::new_v4().to_string(),
            file_path,
            file_hash: None,
            title: None,
            artist: None,
            album: None,
            year: None,
            genre: None,
            label: None,
            catalog_number: None,
            duration_ms,
            bpm: None,
            key: None,
            bitrate: None,
            sample_rate: None,
            format,
            analysis_source: None,
            waveform_data: None,
            rating: 0,
            play_count: 0,
            date_added: now.clone(),
            date_modified: now,
            last_played: None,
            rekordbox_id: None,
            artwork_path: None,
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrackFilter {
    pub search: Option<String>,
    pub tag_ids: Option<Vec<String>>,
    pub tag_filter_mode: Option<String>,
    pub playlist_id: Option<String>,
    pub bpm_min: Option<f64>,
    pub bpm_max: Option<f64>,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrackUpdate {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub label: Option<String>,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub rating: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub tracks: Vec<Track>,
    pub failed_count: usize,
    pub errors: Vec<String>,
}
