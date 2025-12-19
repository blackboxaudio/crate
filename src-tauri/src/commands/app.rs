#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub environment: String,
    pub is_dev: bool,
    pub data_dir: String,
}

#[tauri::command]
pub fn get_app_info() -> AppInfo {
    let is_dev = cfg!(debug_assertions);
    let app_dir_name = if is_dev {
        "com.crate.app.dev"
    } else {
        "com.crate.app"
    };

    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(app_dir_name)
        .to_string_lossy()
        .to_string();

    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        environment: if is_dev {
            "development".to_string()
        } else {
            "production".to_string()
        },
        is_dev,
        data_dir,
    }
}
