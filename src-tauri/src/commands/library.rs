use std::path::PathBuf;

use tauri::State;

use crate::error::CrateError;
use crate::models::{FileMatchResult, ImportResult, Track, TrackFilter, TrackUpdate};
use crate::services::library::RescanResult;
use crate::services::LibraryService;

#[tauri::command]
pub async fn import_tracks(
    paths: Vec<String>,
    library: State<'_, LibraryService>,
) -> Result<ImportResult, CrateError> {
    let pathbufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    library.import_tracks(pathbufs)
}

#[tauri::command]
pub async fn get_tracks(
    filter: Option<TrackFilter>,
    library: State<'_, LibraryService>,
) -> Result<Vec<Track>, CrateError> {
    library.get_tracks(filter)
}

#[tauri::command]
pub async fn get_track(
    id: String,
    library: State<'_, LibraryService>,
) -> Result<Track, CrateError> {
    library.get_track(&id)
}

#[tauri::command]
pub async fn update_track(
    id: String,
    update: TrackUpdate,
    library: State<'_, LibraryService>,
) -> Result<Track, CrateError> {
    library.update_track(&id, update)
}

#[tauri::command]
pub async fn delete_tracks(
    ids: Vec<String>,
    library: State<'_, LibraryService>,
) -> Result<(), CrateError> {
    library.delete_tracks(ids)
}

#[tauri::command]
pub async fn search_tracks(
    query: String,
    library: State<'_, LibraryService>,
) -> Result<Vec<Track>, CrateError> {
    let filter = TrackFilter {
        search: Some(query),
        ..Default::default()
    };
    library.get_tracks(Some(filter))
}

#[tauri::command]
pub async fn rescan_artwork(
    library: State<'_, LibraryService>,
) -> Result<RescanResult, CrateError> {
    library.rescan_all_artwork()
}

#[tauri::command]
pub async fn rescan_track_artwork(
    id: String,
    library: State<'_, LibraryService>,
) -> Result<bool, CrateError> {
    library.rescan_track_artwork(&id)
}

#[tauri::command]
pub async fn check_file_exists(
    track_id: String,
    library: State<'_, LibraryService>,
) -> Result<bool, CrateError> {
    library.check_track_file_exists(&track_id)
}

#[tauri::command]
pub async fn validate_replacement_file(
    track_id: String,
    new_path: String,
    library: State<'_, LibraryService>,
) -> Result<FileMatchResult, CrateError> {
    library.validate_replacement_file(&track_id, &PathBuf::from(new_path))
}

#[tauri::command]
pub async fn relocate_track(
    track_id: String,
    new_path: String,
    force: bool,
    library: State<'_, LibraryService>,
) -> Result<Track, CrateError> {
    library.relocate_track(&track_id, &PathBuf::from(new_path), force)
}

#[tauri::command]
pub async fn set_track_colors(
    track_ids: Vec<String>,
    color: Option<String>,
    library: State<'_, LibraryService>,
) -> Result<(), CrateError> {
    library.set_track_colors(track_ids, color)
}
