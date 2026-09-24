use tauri::State;

use crate::error::Result;
use crate::models::AppSettings;
#[cfg(feature = "desktop")]
use crate::services::ui_zoom;
use crate::services::SettingsService;

#[tauri::command]
pub async fn get_settings(settings: State<'_, SettingsService>) -> Result<AppSettings> {
    settings.get_settings()
}

#[tauri::command]
pub async fn set_setting(
    key: String,
    value: String,
    settings: State<'_, SettingsService>,
) -> Result<()> {
    settings.set_setting(&key, &value)
}

/// Set the webview page zoom to a ladder level (snapped) and persist it. Reset = 1.0.
#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn set_ui_zoom(app: tauri::AppHandle, level: f64) -> Result<f64> {
    ui_zoom::apply(&app, level)
}

/// Step the webview page zoom up (+1) or down (-1) the ladder from the persisted level.
#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn step_ui_zoom(app: tauri::AppHandle, delta: i32) -> Result<f64> {
    ui_zoom::step_and_apply(&app, delta)
}
