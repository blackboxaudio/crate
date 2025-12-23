use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::error::Result;
use crate::models::export::ExportCheckpoint;
use crate::models::{DeviceExport, ExportRequest, ExportResult};
use crate::services::export::CheckpointService;
use crate::services::ExportService;

/// Export playlists to a USB device
#[tauri::command]
pub async fn export_playlists(
    request: ExportRequest,
    export_service: State<'_, Arc<ExportService>>,
    app_handle: AppHandle,
) -> Result<ExportResult> {
    export_service.export_playlists(&app_handle, request)
}

/// Get all exports for a device
#[tauri::command]
pub async fn get_device_exports(
    device_id: String,
    export_service: State<'_, Arc<ExportService>>,
) -> Result<Vec<DeviceExport>> {
    export_service.get_device_exports(&device_id)
}

/// Cancel the current export operation
#[tauri::command]
pub async fn cancel_export(export_service: State<'_, Arc<ExportService>>) -> Result<()> {
    export_service.cancel_export();
    Ok(())
}

/// Clean up a failed export by removing copied files
#[tauri::command]
pub async fn cleanup_failed_export(
    device_id: String,
    mount_point: String,
    export_service: State<'_, Arc<ExportService>>,
) -> Result<()> {
    export_service.cleanup_failed_export(&device_id, &mount_point)
}

/// Get a pending checkpoint for a device
#[tauri::command]
pub async fn get_pending_checkpoint(
    device_id: String,
    checkpoint_service: State<'_, Arc<CheckpointService>>,
) -> Result<Option<ExportCheckpoint>> {
    checkpoint_service.get_pending_checkpoint(&device_id)
}

/// Delete a checkpoint
#[tauri::command]
pub async fn delete_checkpoint(
    checkpoint_id: String,
    checkpoint_service: State<'_, Arc<CheckpointService>>,
) -> Result<()> {
    checkpoint_service.delete_checkpoint(&checkpoint_id)
}

/// Resume a previously interrupted export
#[tauri::command]
pub async fn resume_export(
    device_id: String,
    mount_point: String,
    export_service: State<'_, Arc<ExportService>>,
    checkpoint_service: State<'_, Arc<CheckpointService>>,
    app_handle: AppHandle,
) -> Result<ExportResult> {
    // Get the pending checkpoint
    let checkpoint = checkpoint_service
        .get_pending_checkpoint(&device_id)?
        .ok_or_else(|| crate::error::CrateError::Export("No pending checkpoint found".to_string()))?;

    // Create a request from the checkpoint
    let request = ExportRequest {
        device_id: checkpoint.device_id.clone(),
        mount_point,
        device_name: checkpoint.device_name.clone(),
        playlist_ids: checkpoint.playlist_ids.clone(),
        enable_sync: true,
        use_device_library_plus: false,
    };

    // Resume the export - the export service will detect the checkpoint
    // and skip already-completed tracks
    let result = export_service.export_playlists(&app_handle, request)?;

    // If successful, delete the checkpoint
    if result.success {
        checkpoint_service.complete_checkpoint(&checkpoint.id)?;
    }

    Ok(result)
}
