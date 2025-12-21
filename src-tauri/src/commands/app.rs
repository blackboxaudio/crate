use crate::menu::{rebuild_menu_with_translations, MenuTranslations};
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
pub fn rebuild_menu(app: tauri::AppHandle, translations: MenuTranslations) -> Result<(), String> {
    let menu = rebuild_menu_with_translations(&app, &translations).map_err(|e| e.to_string())?;
    app.set_menu(menu).map_err(|e| e.to_string())?;
    Ok(())
}
