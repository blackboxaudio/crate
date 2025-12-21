use tauri::{AppHandle, State};

use crate::error::CrateError;
use crate::models::{DeviceExport, ExportRequest, ExportResult};
use crate::services::ExportService;

/// Export playlists to a USB device
#[tauri::command]
pub async fn export_playlists(
    request: ExportRequest,
    export_service: State<'_, ExportService>,
    app_handle: AppHandle,
) -> Result<ExportResult, CrateError> {
    export_service.export_playlists(&app_handle, request)
}

/// Get all exports for a device
#[tauri::command]
pub async fn get_device_exports(
    device_id: String,
    export_service: State<'_, ExportService>,
) -> Result<Vec<DeviceExport>, CrateError> {
    export_service.get_device_exports(&device_id)
}

/// Cancel the current export operation
#[tauri::command]
pub async fn cancel_export(export_service: State<'_, ExportService>) -> Result<(), CrateError> {
    export_service.cancel_export();
    Ok(())
}

/// Clean up a failed export by removing copied files
#[tauri::command]
pub async fn cleanup_failed_export(
    device_id: String,
    mount_point: String,
    export_service: State<'_, ExportService>,
) -> Result<(), CrateError> {
    export_service.cleanup_failed_export(&device_id, &mount_point)
}
