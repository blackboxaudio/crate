use std::path::PathBuf;

use tauri::State;

use crate::error::Result;
use crate::models::{
    DuplicateResolution, FileMatchResult, ImportResult, ImportResultWithDuplicates, Track,
    TrackFilter, TrackUpdate,
};
use crate::services::library::RescanResult;
use crate::services::LibraryService;

#[tauri::command]
pub async fn import_tracks(
    paths: Vec<String>,
    library: State<'_, LibraryService>,
) -> Result<ImportResult> {
    let pathbufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    library.import_tracks(pathbufs)
}

#[tauri::command]
pub async fn get_tracks(
    filter: Option<TrackFilter>,
    library: State<'_, LibraryService>,
) -> Result<Vec<Track>> {
    library.get_tracks(filter)
}

#[tauri::command]
pub async fn get_track(id: String, library: State<'_, LibraryService>) -> Result<Track> {
    library.get_track(&id)
}

#[tauri::command]
pub async fn update_track(
    id: String,
    update: TrackUpdate,
    library: State<'_, LibraryService>,
) -> Result<Track> {
    library.update_track(&id, update)
}

#[tauri::command]
pub async fn delete_tracks(ids: Vec<String>, library: State<'_, LibraryService>) -> Result<()> {
    library.delete_tracks(ids)
}

#[tauri::command]
pub async fn search_tracks(
    query: String,
    library: State<'_, LibraryService>,
) -> Result<Vec<Track>> {
    let filter = TrackFilter {
        search: Some(query),
        ..Default::default()
    };
    library.get_tracks(Some(filter))
}

#[tauri::command]
pub async fn rescan_artwork(library: State<'_, LibraryService>) -> Result<RescanResult> {
    library.rescan_all_artwork()
}

#[tauri::command]
pub async fn rescan_track_artwork(id: String, library: State<'_, LibraryService>) -> Result<bool> {
    library.rescan_track_artwork(&id)
}

#[tauri::command]
pub async fn check_file_exists(
    track_id: String,
    library: State<'_, LibraryService>,
) -> Result<bool> {
    library.check_track_file_exists(&track_id)
}

#[tauri::command]
pub async fn validate_replacement_file(
    track_id: String,
    new_path: String,
    library: State<'_, LibraryService>,
) -> Result<FileMatchResult> {
    library.validate_replacement_file(&track_id, &PathBuf::from(new_path))
}

#[tauri::command]
pub async fn relocate_track(
    track_id: String,
    new_path: String,
    force: bool,
    library: State<'_, LibraryService>,
) -> Result<Track> {
    library.relocate_track(&track_id, &PathBuf::from(new_path), force)
}

#[tauri::command]
pub async fn set_track_colors(
    track_ids: Vec<String>,
    color: Option<String>,
    library: State<'_, LibraryService>,
) -> Result<()> {
    library.set_track_colors(track_ids, color)
}

#[tauri::command]
pub async fn update_tracks(
    ids: Vec<String>,
    update: TrackUpdate,
    library: State<'_, LibraryService>,
) -> Result<Vec<Track>> {
    library.update_tracks(ids, update)
}

#[tauri::command]
pub async fn set_track_artwork(
    track_id: String,
    file_path: String,
    library: State<'_, LibraryService>,
) -> Result<Track> {
    library.set_track_artwork(&track_id, &PathBuf::from(file_path))
}

#[tauri::command]
pub async fn delete_track_artwork(
    track_id: String,
    library: State<'_, LibraryService>,
) -> Result<Track> {
    library.delete_track_artwork(&track_id)
}

#[tauri::command]
pub async fn reextract_track_artwork(
    track_id: String,
    library: State<'_, LibraryService>,
) -> Result<Track> {
    library.reextract_track_artwork(&track_id)
}

#[tauri::command]
pub async fn import_tracks_with_duplicates(
    paths: Vec<String>,
    library: State<'_, LibraryService>,
) -> Result<ImportResultWithDuplicates> {
    let pathbufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    library.import_tracks_with_duplicate_detection(pathbufs)
}

#[tauri::command]
pub async fn resolve_duplicate(
    resolution: DuplicateResolution,
    library: State<'_, LibraryService>,
) -> Result<Option<Track>> {
    library.resolve_duplicate(resolution)
}
