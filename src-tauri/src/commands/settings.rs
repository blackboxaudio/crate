use tauri::State;

use crate::error::CrateError;
use crate::models::AppSettings;
use crate::services::SettingsService;

#[tauri::command]
pub async fn get_settings(
    settings: State<'_, SettingsService>,
) -> Result<AppSettings, CrateError> {
    settings.get_settings()
}

#[tauri::command]
pub async fn set_setting(
    key: String,
    value: String,
    settings: State<'_, SettingsService>,
) -> Result<(), CrateError> {
    settings.set_setting(&key, &value)
}
