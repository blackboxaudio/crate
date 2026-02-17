use crate::menu::{self, update_menu_translations, MenuTranslations};
use tauri::Manager;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub environment: String,
    pub is_dev: bool,
    pub data_dir: String,
}

#[tauri::command]
pub fn get_app_info(app: tauri::AppHandle) -> Result<AppInfo, String> {
    let environment = option_env!("CRATE_ENV")
        .unwrap_or("development")
        .to_string();
    let is_dev = environment == "development";

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();

    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        environment,
        is_dev,
        data_dir,
    })
}

#[tauri::command]
pub fn open_dev_tools(app: tauri::AppHandle) {
    #[cfg(feature = "devtools")]
    if let Some(window) = app.get_webview_window("main") {
        window.open_devtools();
    }

    #[cfg(not(feature = "devtools"))]
    {
        log::warn!("DevTools requested but not available in this build");
        let _ = app;
    }
}

#[tauri::command]
pub fn close_dev_tools(app: tauri::AppHandle) {
    #[cfg(feature = "devtools")]
    if let Some(window) = app.get_webview_window("main") {
        window.close_devtools();
    }

    #[cfg(not(feature = "devtools"))]
    {
        log::warn!("DevTools requested but not available in this build");
        let _ = app;
    }
}

#[tauri::command]
pub fn set_menu_item_enabled(app: tauri::AppHandle, id: String, enabled: bool) -> Result<(), String> {
    menu::set_menu_item_enabled(&app, &id, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rebuild_menu(app: tauri::AppHandle, translations: MenuTranslations) -> Result<(), String> {
    // Use in-place text updates instead of rebuilding the entire menu
    // This works better on macOS where set_menu() may not visually refresh in production builds
    update_menu_translations(&app, &translations).map_err(|e| e.to_string())
}
