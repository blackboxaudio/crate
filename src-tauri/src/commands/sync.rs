use tauri::{AppHandle, State};

use crate::error::CrateError;
use crate::services::sync::{DeviceInfo, SyncResult};
use crate::services::SyncService;

/// Sync playlists to a USB device
#[tauri::command]
pub async fn sync_device(
    device_id: String,
    device_name: String,
    mount_point: String,
    playlist_ids: Vec<String>,
    sync_service: State<'_, SyncService>,
    app_handle: AppHandle,
) -> Result<SyncResult, CrateError> {
    sync_service.sync_device(&app_handle, &device_id, &device_name, &mount_point, &playlist_ids)
}

/// Get playlists with pending changes for a device
#[tauri::command]
pub async fn get_pending_sync_playlists(
    device_id: String,
    sync_service: State<'_, SyncService>,
) -> Result<Vec<String>, CrateError> {
    sync_service.get_pending_playlists_for_device(&device_id)
}

/// Check if a device has any pending changes
#[tauri::command]
pub async fn has_pending_sync_changes(
    device_id: String,
    sync_service: State<'_, SyncService>,
) -> Result<bool, CrateError> {
    sync_service.has_pending_changes(&device_id)
}

/// Check if a sync is currently in progress
#[tauri::command]
pub async fn is_syncing(sync_service: State<'_, SyncService>) -> Result<bool, CrateError> {
    Ok(sync_service.is_syncing())
}

/// Cancel the current sync operation
#[tauri::command]
pub async fn cancel_sync(sync_service: State<'_, SyncService>) -> Result<(), CrateError> {
    sync_service.cancel_sync();
    Ok(())
}

/// Get playlists containing a specific track
#[tauri::command]
pub async fn get_playlists_containing_track(
    track_id: String,
    sync_service: State<'_, SyncService>,
) -> Result<Vec<String>, CrateError> {
    sync_service.get_playlists_containing_track(&track_id)
}

/// Get playlists containing any of the specified tracks
#[tauri::command]
pub async fn get_playlists_containing_tracks(
    track_ids: Vec<String>,
    sync_service: State<'_, SyncService>,
) -> Result<Vec<String>, CrateError> {
    sync_service.get_playlists_containing_tracks(&track_ids)
}

/// Get devices that have a specific playlist exported with sync enabled
#[tauri::command]
pub async fn get_devices_for_playlist(
    playlist_id: String,
    sync_service: State<'_, SyncService>,
) -> Result<Vec<DeviceInfo>, CrateError> {
    sync_service.get_devices_for_playlist(&playlist_id)
}

/// Get devices that have any of the specified playlists exported with sync enabled
#[tauri::command]
pub async fn get_devices_for_playlists(
    playlist_ids: Vec<String>,
    sync_service: State<'_, SyncService>,
) -> Result<Vec<DeviceInfo>, CrateError> {
    sync_service.get_devices_for_playlists(&playlist_ids)
}
