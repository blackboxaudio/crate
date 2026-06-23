//! iOS-only commands driving the native preview playback engine (#54).
//!
//! The frontend resolves every track's proxy URL up front (via `fetch_preview_stream`) and hands the
//! whole release here; the engine plays through `AVPlayer` and owns the lock screen via
//! `MPRemoteCommandCenter` / `MPNowPlayingInfoCenter`, so prev/next/scrubber keep working while the
//! WebView's JavaScript is suspended on lock. State is pushed back via `native-preview-*` events.

use tauri::State;

use crate::error::Result;
use crate::services::media_controls::{NativePreviewEngine, NativeTrackEntry};

#[tauri::command]
pub async fn native_preview_play(
    tracks: Vec<NativeTrackEntry>,
    start_index: usize,
    engine: State<'_, NativePreviewEngine>,
) -> Result<()> {
    engine.play(tracks, start_index);
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
