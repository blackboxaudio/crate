use serde::{Deserialize, Serialize};

/// Status of an export operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExportStatus {
    Pending,
    Copying,
    GeneratingDatabase,
    Completed,
    Failed,
}

/// Progress update during export operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportProgress {
    pub status: ExportStatus,
    pub current_file: Option<String>,
    pub files_copied: u32,
    pub files_total: u32,
    pub bytes_copied: u64,
    pub bytes_total: u64,
}

/// Request to export playlists to a device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRequest {
    /// Device identifier (volume UUID)
    pub device_id: String,
    /// Actual filesystem path for file operations
    pub mount_point: String,
    /// Device name for display and recording
    pub device_name: String,
    /// Playlist IDs to export
    pub playlist_ids: Vec<String>,
    /// Whether to enable auto-sync for these playlists
    pub enable_sync: bool,
    /// Whether to use Device Library Plus format (SQLCipher encrypted SQLite)
    /// instead of the legacy PDB format. Device Library Plus is used by newer
    /// Pioneer DJ hardware (OPUS-QUAD, OMNIS-DUO, XDJ-AZ).
    #[serde(default)]
    pub use_device_library_plus: bool,
}

/// Result of an export operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub success: bool,
    pub tracks_copied: u32,
    pub tracks_skipped: u32,
    pub errors: Vec<String>,
}

/// Record of a playlist exported to a device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceExport {
    pub id: String,
    pub device_id: String,
    pub device_name: String,
    pub playlist_id: String,
    pub last_export_at: String,
    pub sync_enabled: bool,
}

/// Record of a track copied to a device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceTrack {
    pub device_id: String,
    pub track_id: String,
    /// Path on USB relative to Contents/ directory
    pub usb_path: String,
    /// Hash of the file at time of export
    pub file_hash: String,
    /// Sequential ID assigned in PDB (for Rekordbox compatibility)
    pub pdb_track_id: Option<i32>,
    pub exported_at: String,
    /// Hash of metadata for detecting changes
    pub metadata_hash: Option<String>,
}

/// State of an export checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CheckpointState {
    /// Currently copying tracks
    Copying {
        current_track_id: Option<String>,
        bytes_copied: u64,
    },
    /// Generating PDB database
    GeneratingPdb,
}

/// Export checkpoint for resumable exports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportCheckpoint {
    pub id: String,
    pub device_id: String,
    pub device_name: String,
    pub started_at: String,
    pub state: CheckpointState,
    pub playlist_ids: Vec<String>,
    pub tracks_completed: Vec<String>,
    pub tracks_failed: Vec<(String, String)>,
    pub last_updated_at: String,
}

impl ExportProgress {
    pub fn new(files_total: u32, bytes_total: u64) -> Self {
        Self {
            status: ExportStatus::Pending,
            current_file: None,
            files_copied: 0,
            files_total,
            bytes_copied: 0,
            bytes_total,
        }
    }

    pub fn copying(mut self, current_file: String) -> Self {
        self.status = ExportStatus::Copying;
        self.current_file = Some(current_file);
        self
    }

    pub fn file_copied(mut self, bytes: u64) -> Self {
        self.files_copied += 1;
        self.bytes_copied += bytes;
        self
    }

    pub fn generating_database(mut self) -> Self {
        self.status = ExportStatus::GeneratingDatabase;
        self.current_file = None;
        self
    }

    pub fn completed(mut self) -> Self {
        self.status = ExportStatus::Completed;
        self.current_file = None;
        self
    }

    #[allow(dead_code)]
    pub fn failed(mut self) -> Self {
        self.status = ExportStatus::Failed;
        self
    }
}
