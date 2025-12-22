use tauri::State;

use crate::error::CrateError;
use crate::models::Track;
use crate::services::analysis::AnalysisResult;
use crate::services::AnalysisService;

/// Analyze tracks for BPM and key detection
#[tauri::command]
pub async fn analyze_tracks(
    track_ids: Vec<String>,
    analysis: State<'_, AnalysisService>,
) -> Result<Vec<AnalysisResult>, CrateError> {
    analysis.analyze_tracks(track_ids)
}

/// Get updated tracks after analysis
#[tauri::command]
pub async fn get_analyzed_tracks(
    track_ids: Vec<String>,
    analysis: State<'_, AnalysisService>,
) -> Result<Vec<Track>, CrateError> {
    let mut tracks = Vec::new();
    for id in track_ids {
        let track = analysis.get_updated_track(&id)?;
        tracks.push(track);
    }
    Ok(tracks)
}
