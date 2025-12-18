use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub is_folder: bool,
    pub is_smart: bool,
    pub smart_rules: Option<String>, // JSON string
    pub sort_order: i32,
    pub date_created: String,
    pub date_modified: String,
    #[serde(default)]
    pub track_count: i32,
}

#[allow(dead_code)]
impl Playlist {
    pub fn new(name: String, parent_id: Option<String>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            parent_id,
            is_folder: false,
            is_smart: false,
            smart_rules: None,
            sort_order: 0,
            date_created: now.clone(),
            date_modified: now,
            track_count: 0,
        }
    }

    pub fn new_folder(name: String, parent_id: Option<String>) -> Self {
        let mut playlist = Self::new(name, parent_id);
        playlist.is_folder = true;
        playlist
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistTrack {
    pub playlist_id: String,
    pub track_id: String,
    pub position: i32,
    pub date_added: String,
}
