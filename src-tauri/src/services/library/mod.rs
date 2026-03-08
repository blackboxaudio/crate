mod artwork;
mod duplicates;
mod import;
mod query;
mod relocation;
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

/// Result of processing a single import path
enum ImportPathResult {
    NewTrack(Track),
    Duplicate(DuplicateTrack),
}
