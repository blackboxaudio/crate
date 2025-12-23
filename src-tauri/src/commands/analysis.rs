use tauri::{AppHandle, State};

use crate::error::Result;
use crate::models::Track;
use crate::services::AnalysisService;

/// Analyze tracks for BPM and key detection with per-track events
#[tauri::command]
pub async fn analyze_tracks(
    track_ids: Vec<String>,
    analysis: State<'_, AnalysisService>,
    app_handle: AppHandle,
) -> Result<()> {
    analysis.analyze_tracks_async(app_handle, track_ids).await
}

/// Cancel analysis for a specific track
#[tauri::command]
pub async fn cancel_track_analysis(
    track_id: String,
    analysis: State<'_, AnalysisService>,
) -> Result<bool> {
    Ok(analysis.cancel_track_analysis(&track_id))
}

/// Cancel all running analysis operations (legacy)
#[tauri::command]
pub async fn cancel_analysis(analysis: State<'_, AnalysisService>) -> Result<()> {
    analysis.cancel_analysis();
    Ok(())
}

/// Get updated tracks after analysis
#[tauri::command]
pub async fn get_analyzed_tracks(
    track_ids: Vec<String>,
    analysis: State<'_, AnalysisService>,
) -> Result<Vec<Track>> {
    let mut tracks = Vec::new();
    for id in track_ids {
        let track = analysis.get_updated_track(&id)?;
        tracks.push(track);
    }
    Ok(tracks)
}
