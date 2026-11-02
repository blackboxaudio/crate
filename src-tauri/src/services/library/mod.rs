mod artwork;
mod duplicates;
mod import;
mod query;
mod relocation;
mod scan;
mod update;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{CrateError, Result};
use crate::models::{
    DuplicateResolution, DuplicateTrack, FileMatchResult, ImportResult, ImportResultWithDuplicates,
    Tag, Track, TrackFilter, TrackUpdate,
};
use crate::services::hash::compute_audio_hash;
use crate::services::ArtworkService;

/// Single source of truth for the audio file extensions the backend accepts when
/// importing or validating a track. The extension is always lowercased before it is
/// compared against this list.
///
/// The desktop frontend (`apps/desktop`) keeps its own copy of this list to build the
/// file-dialog filter; that copy lives in TypeScript and is deliberately not wired to
/// this constant.
pub const SUPPORTED_AUDIO_EXTENSIONS: &[&str] =
    &["mp3", "wav", "aiff", "aif", "flac", "m4a", "aac"];

pub struct LibraryService {
    conn: Arc<Mutex<Connection>>,
    artwork_service: ArtworkService,
}

impl LibraryService {
    pub fn new(conn: Arc<Mutex<Connection>>, app_data_dir: PathBuf) -> Self {
        Self {
            conn,
            artwork_service: ArtworkService::new(app_data_dir),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RescanResult {
    pub updated_count: usize,
    pub failed_count: usize,
}

/// Outcome of a recursive scan of the device's music folder.
///
/// Imported tracks are reported as ids rather than full `Track` payloads so the
/// frontend can still honor auto-analyze-on-import without shipping thousands of
/// objects over IPC. Serializes as snake_case like the rest of the library results.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LibraryFolderScanResult {
    pub scanned_count: usize,
    pub imported_count: usize,
    pub skipped_existing_count: usize,
    pub failed_count: usize,
    pub imported_track_ids: Vec<String>,
    pub errors: Vec<String>,
}

/// Result of processing a single import path
enum ImportPathResult {
    NewTrack(Track),
    Duplicate(DuplicateTrack),
}
