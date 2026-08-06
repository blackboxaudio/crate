//! iOS-only commands driving the native preview playback engine (#54).
//!
//! The frontend resolves every track's proxy URL up front (via `fetch_preview_stream`) and hands the
//! whole release here; the engine plays through `AVPlayer` and owns the lock screen via
//! `MPRemoteCommandCenter` / `MPNowPlayingInfoCenter`, so prev/next/scrubber keep working while the
//! WebView's JavaScript is suspended on lock. State is pushed back via `native-preview-*` events.

use tauri::State;

use crate::error::Result;
use crate::services::media_controls::{NativePreviewEngine, NativeTrackEntry};

/// `load_id` identifies this load; the engine stamps it on every track-changed/ended event it emits
/// for this playlist so the frontend can drop events that belong to a superseded load (events and
/// invoke responses race each other across the IPC bridge).
#[tauri::command]
pub async fn native_preview_play(
    tracks: Vec<NativeTrackEntry>,
    start_index: usize,
    start_position_ms: u64,
    load_id: u64,
    engine: State<'_, NativePreviewEngine>,
) -> Result<()> {
    engine.play(tracks, start_index, start_position_ms, load_id);
    Ok(())
}

#[tauri::command]
pub async fn native_preview_set_upcoming(
    tracks: Vec<NativeTrackEntry>,
    engine: State<'_, NativePreviewEngine>,
) -> Result<()> {
    engine.set_upcoming(tracks);
    Ok(())
}

#[tauri::command]
pub async fn native_preview_pause(engine: State<'_, NativePreviewEngine>) -> Result<()> {
    engine.pause();
    Ok(())
}

#[tauri::command]
pub async fn native_preview_resume(engine: State<'_, NativePreviewEngine>) -> Result<()> {
    engine.resume();
    Ok(())
}

#[tauri::command]
pub async fn native_preview_seek(
    position_ms: u64,
    engine: State<'_, NativePreviewEngine>,
) -> Result<()> {
    engine.seek(position_ms);
    Ok(())
}

#[tauri::command]
pub async fn native_preview_next(engine: State<'_, NativePreviewEngine>) -> Result<()> {
    engine.next();
    Ok(())
}

#[tauri::command]
pub async fn native_preview_previous(engine: State<'_, NativePreviewEngine>) -> Result<()> {
    engine.previous();
    Ok(())
}

#[tauri::command]
pub async fn native_preview_stop(engine: State<'_, NativePreviewEngine>) -> Result<()> {
    engine.stop();
    Ok(())
}

#[tauri::command]
pub async fn native_preview_set_volume(
    volume: f64,
    engine: State<'_, NativePreviewEngine>,
) -> Result<()> {
    engine.set_volume(volume);
    Ok(())
}

#[tauri::command]
pub async fn native_preview_set_rate(
    rate: f64,
    engine: State<'_, NativePreviewEngine>,
) -> Result<()> {
    engine.set_rate(rate);
    Ok(())
}

/// Reflect an in-app like toggle on the native engine (window entries + lock-screen glyph).
/// The reverse direction — a lock-screen Like press — toggles the DB natively and notifies the
/// frontend via the `native-preview-like-changed` event.
#[tauri::command]
pub async fn native_preview_set_liked(
    track_id: String,
    liked: bool,
    engine: State<'_, NativePreviewEngine>,
) -> Result<()> {
    engine.set_liked(track_id, liked);
    Ok(())
}

/// Apply the app's repeat mode (`off`/`track`/`release`/`context`) to the native engine: `track`
/// loops the ending item natively (gapless, works while the screen is locked); the lock-screen
/// repeat glyph shows One/All/Off. The reverse direction — the lock-screen repeat button — reports
/// via the `native-preview-repeat-changed` event and the frontend echoes the mode back through here.
#[tauri::command]
pub async fn native_preview_set_repeat_mode(
    mode: String,
    engine: State<'_, NativePreviewEngine>,
) -> Result<()> {
    engine.set_repeat_mode(mode);
    Ok(())
}
