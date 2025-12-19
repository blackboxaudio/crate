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
    let is_dev = cfg!(debug_assertions);

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();

    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        environment: if is_dev {
            "development".to_string()
        } else {
            "production".to_string()
        },
        is_dev,
        data_dir,
    })
}
