use std::time::Duration;

use tauri::State;

use crate::error::Result;
use crate::services::media_controls::{NowPlayingMetadata, PlaybackStatus};
use crate::services::MediaControlsService;

#[tauri::command]
pub async fn update_now_playing(
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    artwork_path: Option<String>,
    duration_ms: Option<u64>,
    media_controls: State<'_, MediaControlsService>,
) -> Result<()> {
    let cover_url = artwork_path.map(|p| {
        if p.starts_with("http://") || p.starts_with("https://") {
            p
        } else {
            format!("file://{p}")
        }
    });

    media_controls.set_metadata(&NowPlayingMetadata {
        title,
        artist,
        album,
        cover_url,
        duration: duration_ms.map(Duration::from_millis),
    });

    Ok(())
}

#[tauri::command]
pub async fn update_playback_state(
    is_playing: bool,
    media_controls: State<'_, MediaControlsService>,
) -> Result<()> {
    let status = if is_playing {
        PlaybackStatus::Playing
    } else {
        PlaybackStatus::Paused
    };
    media_controls.set_playback(status, None);

    Ok(())
}

#[tauri::command]
pub async fn clear_now_playing(media_controls: State<'_, MediaControlsService>) -> Result<()> {
    media_controls.clear();

    Ok(())
}
