use tauri::{AppHandle, State};

use crate::error::Result;
use crate::services::BackupService;

#[tauri::command]
pub async fn create_backup(
    path: String,
    backup_service: State<'_, BackupService>,
    app_handle: AppHandle,
) -> Result<()> {
    let conn = backup_service.connection();
    let app_version = app_handle.package_info().version.to_string();
    crate::services::backup::create_backup(path, conn, app_handle, app_version).await
}

#[tauri::command]
pub async fn restore_from_backup(
    path: String,
    backup_service: State<'_, BackupService>,
    app_handle: AppHandle,
) -> Result<()> {
    let conn = backup_service.connection();
    crate::services::backup::restore_from_backup(path, conn, app_handle).await
}
