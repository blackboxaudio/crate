use std::time::Duration;

use souvlaki::MediaPlayback;
use tauri::State;

use crate::error::Result;
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
    let cover_url = artwork_path.map(|p| format!("file://{p}"));
    let duration = duration_ms.map(Duration::from_millis);

    media_controls.set_metadata(
        title.as_deref(),
        artist.as_deref(),
        album.as_deref(),
        cover_url.as_deref(),
        duration,
    );

    Ok(())
}

#[tauri::command]
pub async fn update_playback_state(
    is_playing: bool,
    media_controls: State<'_, MediaControlsService>,
) -> Result<()> {
    let playback = if is_playing {
        MediaPlayback::Playing { progress: None }
    } else {
        MediaPlayback::Paused { progress: None }
    };
    media_controls.set_playback(playback);

    Ok(())
}

#[tauri::command]
pub async fn clear_now_playing(media_controls: State<'_, MediaControlsService>) -> Result<()> {
    media_controls.set_playback(MediaPlayback::Stopped);
    media_controls.set_metadata(None, None, None, None, None);

    Ok(())
}
