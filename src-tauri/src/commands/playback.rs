use std::path::PathBuf;

use tauri::State;

use crate::error::CrateError;
use crate::services::audio::PlaybackState;
use crate::services::{AudioService, LibraryService};

#[tauri::command]
pub async fn play_track(
    id: String,
    library: State<'_, LibraryService>,
    audio: State<'_, AudioService>,
) -> Result<PlaybackState, CrateError> {
    let track = library.get_track(&id)?;
    let path = PathBuf::from(&track.file_path);
    audio.play_track(id, path)
}

#[tauri::command]
pub async fn pause(audio: State<'_, AudioService>) -> Result<PlaybackState, CrateError> {
    audio.pause()
}

#[tauri::command]
pub async fn resume(audio: State<'_, AudioService>) -> Result<PlaybackState, CrateError> {
    audio.resume()
}

#[tauri::command]
pub async fn stop(audio: State<'_, AudioService>) -> Result<PlaybackState, CrateError> {
    audio.stop()
}

#[tauri::command]
pub async fn seek(
    position_ms: u64,
    audio: State<'_, AudioService>,
) -> Result<PlaybackState, CrateError> {
    audio.seek(position_ms)
}

#[tauri::command]
pub async fn set_volume(
    volume: f32,
    audio: State<'_, AudioService>,
) -> Result<PlaybackState, CrateError> {
    audio.set_volume(volume)
}

#[tauri::command]
pub async fn get_playback_state(
    audio: State<'_, AudioService>,
) -> Result<PlaybackState, CrateError> {
    audio.get_state()
}
