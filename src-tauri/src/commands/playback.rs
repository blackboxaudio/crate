use std::path::PathBuf;

use tauri::State;

use crate::error::Result;
use crate::models::AudioDevice;
use crate::services::audio::PlaybackState;
use crate::services::{AudioService, LibraryService, SettingsService};

#[tauri::command]
pub async fn play_track(
    id: String,
    library: State<'_, LibraryService>,
    audio: State<'_, AudioService>,
) -> Result<PlaybackState> {
    let track = library.get_track(&id)?;
    let path = PathBuf::from(&track.file_path);
    audio.play_track(id, path)
}

#[tauri::command]
pub async fn pause(audio: State<'_, AudioService>) -> Result<PlaybackState> {
    audio.pause()
}

#[tauri::command]
pub async fn resume(audio: State<'_, AudioService>) -> Result<PlaybackState> {
    audio.resume()
}

#[tauri::command]
pub async fn stop(audio: State<'_, AudioService>) -> Result<PlaybackState> {
    audio.stop()
}

#[tauri::command]
pub async fn seek(position_ms: u64, audio: State<'_, AudioService>) -> Result<PlaybackState> {
    audio.seek(position_ms)
}

#[tauri::command]
pub async fn set_volume(volume: f32, audio: State<'_, AudioService>) -> Result<PlaybackState> {
    audio.set_volume(volume)
}

#[tauri::command]
pub async fn set_speed(speed: f32, audio: State<'_, AudioService>) -> Result<PlaybackState> {
    audio.set_speed(speed)
}

#[tauri::command]
pub async fn get_playback_state(audio: State<'_, AudioService>) -> Result<PlaybackState> {
    audio.get_state()
}

#[tauri::command]
pub async fn get_audio_devices() -> Result<Vec<AudioDevice>> {
    AudioService::get_output_devices()
}

#[tauri::command]
pub async fn set_audio_device(
    device_name: Option<String>,
    audio: State<'_, AudioService>,
    settings: State<'_, SettingsService>,
) -> Result<()> {
    // Set the device in the audio service
    audio.set_device(device_name.clone())?;

    // Persist the setting
    match device_name {
        Some(name) => settings.set_setting("audio_device", &name)?,
        None => settings.set_setting("audio_device", "")?,
    }

    Ok(())
}
