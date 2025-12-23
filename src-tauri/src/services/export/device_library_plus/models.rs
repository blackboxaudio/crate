//! Rust models for Device Library Plus database tables.
//!
//! These structs represent the 22 tables in the Device Library Plus database format.
//! Based on pyrekordbox/devicelib_plus/models.py

#![allow(dead_code)]

/// Playlist attribute types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistType {
    Playlist = 0,
    Folder = 1,
    SmartPlaylist = 4,
}

impl From<PlaylistType> for i32 {
    fn from(pt: PlaylistType) -> Self {
        pt as i32
    }
}

/// File type enumeration matching Pioneer's format codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Mp3 = 1,
    M4a = 4,
    Flac = 5,
    Wav = 11,
    Aiff = 12,
}

impl FileType {
    /// Determine file type from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "mp3" => Some(FileType::Mp3),
            "m4a" | "aac" => Some(FileType::M4a),
            "flac" => Some(FileType::Flac),
            "wav" => Some(FileType::Wav),
            "aiff" | "aif" => Some(FileType::Aiff),
            _ => None,
        }
    }
}

impl From<FileType> for i32 {
    fn from(ft: FileType) -> Self {
        ft as i32
    }
}

/// Cue point kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CueKind {
    Cue = 0, // Also Fade-In, Fade-Out
    Load = 3,
    Loop = 4,
}

impl From<CueKind> for i32 {
    fn from(ck: CueKind) -> Self {
        ck as i32
    }
}

// ============================================================================
// Table Models
// ============================================================================

/// Artist table entry
#[derive(Debug, Clone)]
pub struct Artist {
    pub artist_id: Option<i64>,
    pub name: String,
    pub name_for_search: Option<String>,
}

impl Artist {
    pub fn new(name: String) -> Self {
        Self {
            artist_id: None,
            name,
            name_for_search: None,
        }
    }
}

/// Image table entry
#[derive(Debug, Clone)]
pub struct Image {
    pub image_id: Option<i64>,
    pub path: String,
}

impl Image {
    pub fn new(path: String) -> Self {
        Self {
            image_id: None,
            path,
        }
    }
}

/// Album table entry
#[derive(Debug, Clone)]
pub struct Album {
    pub album_id: Option<i64>,
    pub name: String,
    pub artist_id: Option<i64>,
    pub image_id: Option<i64>,
    pub is_compilation: Option<bool>,
    pub name_for_search: Option<String>,
}

impl Album {
    pub fn new(name: String) -> Self {
        Self {
            album_id: None,
            name,
            artist_id: None,
            image_id: None,
            is_compilation: None,
            name_for_search: None,
        }
    }
}

/// Genre table entry
#[derive(Debug, Clone)]
pub struct Genre {
    pub genre_id: Option<i64>,
    pub name: String,
}

impl Genre {
    pub fn new(name: String) -> Self {
        Self {
            genre_id: None,
            name,
        }
    }
}

/// Label (record label) table entry
#[derive(Debug, Clone)]
pub struct Label {
    pub label_id: Option<i64>,
    pub name: String,
}

impl Label {
    pub fn new(name: String) -> Self {
        Self {
            label_id: None,
            name,
        }
    }
}

/// Key (musical key) table entry
#[derive(Debug, Clone)]
pub struct Key {
    pub key_id: Option<i64>,
    pub name: String,
}

impl Key {
    pub fn new(name: String) -> Self {
        Self { key_id: None, name }
    }
}

/// Color table entry
#[derive(Debug, Clone)]
pub struct Color {
    pub color_id: Option<i64>,
    pub name: String,
}

impl Color {
    pub fn new(name: String) -> Self {
        Self {
            color_id: None,
            name,
        }
    }
}

/// Content (track) table entry - the main track table with 40+ columns
#[derive(Debug, Clone)]
pub struct Content {
    pub content_id: Option<i64>,
    pub title: Option<String>,
    pub title_for_search: Option<String>,
    pub subtitle: Option<String>,
    pub bpmx100: Option<i32>,
    pub length: Option<i32>,
    pub track_no: Option<i32>,
    pub disc_no: Option<i32>,
    pub artist_id_artist: Option<i64>,
    pub artist_id_remixer: Option<i64>,
    pub artist_id_original_artist: Option<i64>,
    pub artist_id_composer: Option<i64>,
    pub artist_id_lyricist: Option<i64>,
    pub album_id: Option<i64>,
    pub genre_id: Option<i64>,
    pub label_id: Option<i64>,
    pub key_id: Option<i64>,
    pub color_id: Option<i64>,
    pub image_id: Option<i64>,
    pub dj_comment: Option<String>,
    pub rating: Option<i32>,
    pub release_year: Option<i32>,
    pub release_date: Option<String>,
    pub date_created: Option<String>,
    pub date_added: Option<String>,
    pub path: String,
    pub file_name: String,
    pub file_size: i64,
    pub file_type: i32,
    pub bitrate: i32,
    pub bit_depth: i32,
    pub sampling_rate: i32,
    pub isrc: Option<String>,
    pub is_hot_cue_auto_load_on: Option<i32>,
    pub is_kuvo_deliver_status_on: Option<i32>,
    pub kuvo_delivery_comment: Option<String>,
    pub master_db_id: Option<i64>,
    pub master_content_id: Option<i64>,
    pub analysis_data_file_path: Option<String>,
    pub analysed_bits: Option<i32>,
    pub content_link: Option<i32>,
    pub has_modified: Option<i32>,
    pub cue_update_count: Option<i32>,
    pub analysis_data_update_count: Option<i32>,
    pub information_update_count: Option<i32>,
}

impl Content {
    /// Create a new Content with required fields
    pub fn new(
        path: String,
        file_name: String,
        file_size: i64,
        file_type: i32,
        bitrate: i32,
        bit_depth: i32,
        sampling_rate: i32,
    ) -> Self {
        Self {
            content_id: None,
            title: None,
            title_for_search: None,
            subtitle: None,
            bpmx100: None,
            length: None,
            track_no: None,
            disc_no: None,
            artist_id_artist: None,
            artist_id_remixer: None,
            artist_id_original_artist: None,
            artist_id_composer: None,
            artist_id_lyricist: None,
            album_id: None,
            genre_id: None,
            label_id: None,
            key_id: None,
            color_id: None,
            image_id: None,
            dj_comment: None,
            rating: None,
            release_year: None,
            release_date: None,
            date_created: None,
            date_added: None,
            path,
            file_name,
            file_size,
            file_type,
            bitrate,
            bit_depth,
            sampling_rate,
            isrc: None,
            is_hot_cue_auto_load_on: None,
            is_kuvo_deliver_status_on: None,
            kuvo_delivery_comment: None,
            master_db_id: None,
            master_content_id: None,
            analysis_data_file_path: None,
            analysed_bits: None,
            content_link: None,
            has_modified: Some(0),
            cue_update_count: Some(0),
            analysis_data_update_count: Some(0),
            information_update_count: Some(0),
        }
    }
}

/// Cue point table entry
#[derive(Debug, Clone)]
pub struct Cue {
    pub cue_id: Option<i64>,
    pub content_id: Option<i64>,
    pub kind: Option<i32>,
    pub color_table_index: Option<i32>,
    pub cue_comment: Option<String>,
    pub is_active_loop: Option<i32>,
    pub beat_loop_numerator: Option<i32>,
    pub beat_loop_denominator: Option<i32>,
    pub in_usec: Option<i64>,
    pub out_usec: Option<i64>,
    pub in_150_frame_per_sec: Option<i32>,
    pub out_150_frame_per_sec: Option<i32>,
    pub in_mpeg_frame_number: Option<i32>,
    pub out_mpeg_frame_number: Option<i32>,
    pub in_mpeg_abs: Option<i32>,
    pub out_mpeg_abs: Option<i32>,
    pub in_decoding_start_frame_position: Option<i32>,
    pub out_decoding_start_frame_position: Option<i32>,
    pub in_file_offset_in_block: Option<i32>,
    pub out_file_offset_in_block: Option<i32>,
    pub in_number_of_sample_in_block: Option<i32>,
    pub out_number_of_sample_in_block: Option<i32>,
}

impl Cue {
    pub fn new(content_id: i64, kind: CueKind, in_usec: i64) -> Self {
        Self {
            cue_id: None,
            content_id: Some(content_id),
            kind: Some(kind.into()),
            color_table_index: None,
            cue_comment: None,
            is_active_loop: None,
            beat_loop_numerator: None,
            beat_loop_denominator: None,
            in_usec: Some(in_usec),
            out_usec: None,
            in_150_frame_per_sec: None,
            out_150_frame_per_sec: None,
            in_mpeg_frame_number: None,
            out_mpeg_frame_number: None,
            in_mpeg_abs: None,
            out_mpeg_abs: None,
            in_decoding_start_frame_position: None,
            out_decoding_start_frame_position: None,
            in_file_offset_in_block: None,
            out_file_offset_in_block: None,
            in_number_of_sample_in_block: None,
            out_number_of_sample_in_block: None,
        }
    }
}

/// Playlist table entry
#[derive(Debug, Clone)]
pub struct Playlist {
    pub playlist_id: Option<i64>,
    pub sequence_no: i32,
    pub name: String,
    pub image_id: Option<i64>,
    pub attribute: i32,
    pub playlist_id_parent: Option<i64>,
}

impl Playlist {
    pub fn new(name: String, sequence_no: i32, playlist_type: PlaylistType) -> Self {
        Self {
            playlist_id: None,
            sequence_no,
            name,
            image_id: None,
            attribute: playlist_type.into(),
            playlist_id_parent: None,
        }
    }

    pub fn with_parent(mut self, parent_id: i64) -> Self {
        self.playlist_id_parent = Some(parent_id);
        self
    }
}

/// PlaylistContent table entry (join table)
#[derive(Debug, Clone)]
pub struct PlaylistContent {
    pub playlist_id: i64,
    pub content_id: i64,
    pub sequence_no: i32,
}

/// HotCueBankList table entry
#[derive(Debug, Clone)]
pub struct HotCueBankList {
    pub hot_cue_bank_list_id: Option<i64>,
    pub sequence_no: Option<i32>,
    pub name: Option<String>,
    pub image_id: Option<i64>,
    pub attribute: Option<i32>,
    pub hot_cue_bank_list_id_parent: Option<i64>,
}

/// HotCueBankListCue table entry (join table)
#[derive(Debug, Clone)]
pub struct HotCueBankListCue {
    pub hot_cue_bank_list_id: i64,
    pub cue_id: i64,
    pub sequence_no: Option<i32>,
}

/// History table entry
#[derive(Debug, Clone)]
pub struct History {
    pub history_id: Option<i64>,
    pub sequence_no: Option<i32>,
    pub name: Option<String>,
    pub attribute: Option<String>,
    pub history_id_parent: Option<i64>,
}

/// HistoryContent table entry (join table)
#[derive(Debug, Clone)]
pub struct HistoryContent {
    pub history_id: i64,
    pub content_id: i64,
    pub sequence_no: Option<i32>,
}

/// MyTag table entry (custom tag categories)
#[derive(Debug, Clone)]
pub struct MyTag {
    pub my_tag_id: Option<i64>,
    pub sequence_no: i32,
    pub name: String,
    pub attribute: Option<i32>,
    pub my_tag_id_parent: Option<i64>,
}

/// MyTagContent table entry (join table)
#[derive(Debug, Clone)]
pub struct MyTagContent {
    pub my_tag_id: i64,
    pub content_id: i64,
}

/// Property table entry (database metadata)
#[derive(Debug, Clone)]
pub struct Property {
    pub device_name: String,
    pub db_version: Option<i32>,
    pub number_of_contents: Option<i32>,
    pub created_date: Option<String>,
    pub background_color_type: Option<i32>,
    pub my_tag_master_db_id: Option<i32>,
}

impl Property {
    pub fn new(device_name: String) -> Self {
        Self {
            device_name,
            db_version: Some(1000),
            number_of_contents: Some(0),
            created_date: None,
            background_color_type: Some(0),
            my_tag_master_db_id: None,
        }
    }
}

/// RecommendedLike table entry
#[derive(Debug, Clone)]
pub struct RecommendedLike {
    pub content_id_1: i64,
    pub content_id_2: i64,
    pub rating: Option<i32>,
    pub created_date: Option<String>,
}

/// MenuItem table entry
#[derive(Debug, Clone)]
pub struct MenuItem {
    pub menu_item_id: Option<i64>,
    pub kind: Option<i32>,
    pub name: Option<String>,
}

/// Category table entry
#[derive(Debug, Clone)]
pub struct Category {
    pub category_id: Option<i64>,
    pub menu_item_id: Option<i64>,
    pub sequence_no: Option<i32>,
    pub is_visible: Option<i32>,
}

/// Sort table entry
#[derive(Debug, Clone)]
pub struct Sort {
    pub sort_id: Option<i64>,
    pub menu_item_id: i64,
    pub sequence_no: Option<i32>,
    pub is_visible: Option<i32>,
    pub is_selected_as_sub_column: Option<i32>,
}
