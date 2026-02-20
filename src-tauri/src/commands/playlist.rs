use tauri::State;

use crate::error::Result;
use crate::models::{DiscoveryRelease, MovePlaylistResult, Playlist, Track};
use crate::services::{DiscoveryService, LibraryService, PlaylistService};

#[tauri::command]
pub async fn get_playlists(
    context: String,
    playlists: State<'_, PlaylistService>,
) -> Result<Vec<Playlist>> {
    playlists.get_playlists(&context)
}

#[tauri::command]
pub async fn create_playlist(
    name: String,
    parent_id: Option<String>,
    context: String,
    playlists: State<'_, PlaylistService>,
) -> Result<Playlist> {
    playlists.create_playlist(name, parent_id, context)
}

#[tauri::command]
pub async fn create_folder(
    name: String,
    parent_id: Option<String>,
    context: String,
    playlists: State<'_, PlaylistService>,
) -> Result<Playlist> {
    playlists.create_folder(name, parent_id, context)
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
pub async fn delete_playlist(
    id: String,
    delete_tracks_from_collection: bool,
    playlists: State<'_, PlaylistService>,
    library: State<'_, LibraryService>,
    discovery: State<'_, DiscoveryService>,
) -> Result<()> {
    if delete_tracks_from_collection {
        // Collect IDs before deletion (CASCADE removes junction table entries)
        let (track_ids, release_ids) = playlists.collect_associated_item_ids(&id)?;
        playlists.delete_playlist(&id)?;
        if !track_ids.is_empty() {
            library.delete_tracks(track_ids)?;
        }
        if !release_ids.is_empty() {
            discovery.delete_releases(release_ids)?;
        }
        Ok(())
    } else {
        playlists.delete_playlist(&id)
    }
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

#[tauri::command]
pub async fn add_releases_to_playlist(
    playlist_id: String,
    release_ids: Vec<String>,
    playlists: State<'_, PlaylistService>,
) -> Result<Playlist> {
    playlists.add_releases(&playlist_id, release_ids)
}

#[tauri::command]
pub async fn remove_releases_from_playlist(
    playlist_id: String,
    release_ids: Vec<String>,
    playlists: State<'_, PlaylistService>,
) -> Result<Playlist> {
    playlists.remove_releases(&playlist_id, release_ids)
}

#[tauri::command]
pub async fn get_playlist_releases(
    playlist_id: String,
    playlists: State<'_, PlaylistService>,
) -> Result<Vec<DiscoveryRelease>> {
    playlists.get_playlist_releases(&playlist_id)
}
