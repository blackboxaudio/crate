use tauri::State;

use crate::error::Result;
use crate::models::{MovePlaylistResult, Playlist, Track};
use crate::services::PlaylistService;

#[tauri::command]
pub async fn get_playlists(playlists: State<'_, PlaylistService>) -> Result<Vec<Playlist>> {
    playlists.get_playlists()
}

#[tauri::command]
pub async fn create_playlist(
    name: String,
    parent_id: Option<String>,
    playlists: State<'_, PlaylistService>,
) -> Result<Playlist> {
    playlists.create_playlist(name, parent_id)
}

#[tauri::command]
pub async fn create_folder(
    name: String,
    parent_id: Option<String>,
    playlists: State<'_, PlaylistService>,
) -> Result<Playlist> {
    playlists.create_folder(name, parent_id)
}

#[tauri::command]
pub async fn rename_playlist(
    id: String,
    name: String,
    playlists: State<'_, PlaylistService>,
) -> Result<Playlist> {
    playlists.rename_playlist(&id, name)
}

#[tauri::command]
pub async fn delete_playlist(id: String, playlists: State<'_, PlaylistService>) -> Result<()> {
    playlists.delete_playlist(&id)
}

#[tauri::command]
pub async fn move_playlist(
    id: String,
    parent_id: Option<String>,
    resolution: Option<String>,
    playlists: State<'_, PlaylistService>,
) -> Result<MovePlaylistResult> {
    playlists.move_playlist(&id, parent_id, resolution.as_deref())
}

#[tauri::command]
pub async fn get_playlist_tracks(
    playlist_id: String,
    playlists: State<'_, PlaylistService>,
) -> Result<Vec<Track>> {
    playlists.get_playlist_tracks(&playlist_id)
}

#[tauri::command]
pub async fn add_to_playlist(
    playlist_id: String,
    track_ids: Vec<String>,
    playlists: State<'_, PlaylistService>,
) -> Result<Playlist> {
    playlists.add_tracks(&playlist_id, track_ids)
}

#[tauri::command]
pub async fn remove_from_playlist(
    playlist_id: String,
    track_ids: Vec<String>,
    playlists: State<'_, PlaylistService>,
) -> Result<Playlist> {
    playlists.remove_tracks(&playlist_id, track_ids)
}

#[tauri::command]
pub async fn reorder_playlist(
    playlist_id: String,
    track_ids: Vec<String>,
    playlists: State<'_, PlaylistService>,
) -> Result<()> {
    playlists.reorder_tracks(&playlist_id, track_ids)
}
