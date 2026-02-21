use tauri::State;

use crate::error::Result;
use crate::models::{DiagnosticEntry, DiagnosticLevel, DiagnosticsReport, SystemInfo};
use crate::services::DiagnosticsService;

#[tauri::command]
pub async fn get_diagnostic_entries(
    diagnostics_service: State<'_, DiagnosticsService>,
) -> Result<Vec<DiagnosticEntry>> {
    Ok(diagnostics_service.get_entries())
}

#[tauri::command]
pub async fn get_system_info(
    diagnostics_service: State<'_, DiagnosticsService>,
) -> Result<SystemInfo> {
    Ok(diagnostics_service.get_system_info())
}

#[tauri::command]
pub async fn get_diagnostics_report(
    app: tauri::AppHandle,
    diagnostics_service: State<'_, DiagnosticsService>,
) -> Result<DiagnosticsReport> {
    let version = app.package_info().version.to_string();
    Ok(diagnostics_service.generate_report(version))
}

#[tauri::command]
pub async fn clear_diagnostic_entries(
    diagnostics_service: State<'_, DiagnosticsService>,
) -> Result<()> {
    diagnostics_service.clear_entries();
    Ok(())
}

#[tauri::command]
pub async fn log_error(
    category: String,
    message: String,
    details: Option<String>,
    diagnostics_service: State<'_, DiagnosticsService>,
) -> Result<()> {
    diagnostics_service.log(
        DiagnosticLevel::Error,
        &category,
        &message,
        details.as_deref(),
    );
    Ok(())
}
