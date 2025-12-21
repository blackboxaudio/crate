mod commands;
mod db;
mod error;
mod menu;
mod models;
mod services;

use db::Database;
use services::{
    AudioService, DeviceService, DiagnosticsService, LibraryService, PlaylistService,
    SettingsService, TagService,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            // App commands
            commands::app::get_app_info,
            commands::app::open_dev_tools,
            commands::app::close_dev_tools,
            // Library commands
            commands::library::import_tracks,
            commands::library::get_tracks,
            commands::library::get_track,
            commands::library::update_track,
            commands::library::delete_tracks,
            commands::library::search_tracks,
            commands::library::rescan_artwork,
            commands::library::rescan_track_artwork,
            commands::library::check_file_exists,
            commands::library::validate_replacement_file,
            commands::library::relocate_track,
            commands::library::set_track_colors,
            commands::library::update_tracks,
            commands::library::set_track_artwork,
            commands::library::delete_track_artwork,
            commands::library::reextract_track_artwork,
            commands::library::import_tracks_with_duplicates,
            commands::library::resolve_duplicate,
            // Playback commands
            commands::playback::play_track,
            commands::playback::pause,
            commands::playback::resume,
            commands::playback::stop,
            commands::playback::seek,
            commands::playback::set_volume,
            commands::playback::get_playback_state,
            commands::playback::get_audio_devices,
            commands::playback::set_audio_device,
            // Tag commands
            commands::tag::get_tag_categories,
            commands::tag::create_tag_category,
            commands::tag::update_tag_category,
            commands::tag::delete_tag_category,
            commands::tag::create_tag,
            commands::tag::update_tag,
            commands::tag::delete_tag,
            commands::tag::assign_tags,
            commands::tag::remove_tags,
            // Playlist commands
            commands::playlist::get_playlists,
            commands::playlist::create_playlist,
            commands::playlist::create_folder,
            commands::playlist::rename_playlist,
            commands::playlist::delete_playlist,
            commands::playlist::move_playlist,
            commands::playlist::get_playlist_tracks,
            commands::playlist::add_to_playlist,
            commands::playlist::remove_from_playlist,
            commands::playlist::reorder_playlist,
            // Settings commands
            commands::settings::get_settings,
            commands::settings::set_setting,
            // Device commands
            commands::device::get_devices,
            commands::device::eject_device,
            // Diagnostics commands
            commands::diagnostics::get_diagnostic_entries,
            commands::diagnostics::get_system_info,
            commands::diagnostics::get_diagnostics_report,
            commands::diagnostics::clear_diagnostic_entries,
            commands::diagnostics::log_error,
        ])
        .setup(|app| {
            // Get Tauri's app data directory
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            // Ensure directory exists
            std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");

            let db_path = app_data_dir.join("crate.db");
            log::info!("Database path: {db_path:?}");

            let db = Database::new(db_path).expect("Failed to initialize database");
            let conn = db.connection();

            // Initialize services
            let library_service = LibraryService::new(conn.clone(), app_data_dir.clone());
            let tag_service = TagService::new(conn.clone());
            let playlist_service = PlaylistService::new(conn.clone());
            let settings_service = SettingsService::new(conn.clone());
            let audio_service = AudioService::new().expect("Failed to initialize audio service");
            let device_service = DeviceService::new();
            let diagnostics_service = DiagnosticsService::new(app_data_dir.clone());

            // Load saved audio device setting
            if let Ok(settings) = settings_service.get_settings() {
                if let Some(device_name) = settings.audio_device {
                    if !device_name.is_empty() {
                        let _ = audio_service.set_device(Some(device_name));
                    }
                }
            }

            // Register services with Tauri
            app.manage(library_service);
            app.manage(tag_service);
            app.manage(playlist_service);
            app.manage(settings_service);
            app.manage(audio_service);
            app.manage(device_service);
            app.manage(diagnostics_service);

            // Start device monitoring
            let device_service = app.state::<DeviceService>();
            device_service.start_monitoring(app.handle().clone());

            // Build and set the application menu
            let menu = menu::build_menu(app.handle())?;
            app.set_menu(menu)?;
            menu::setup_menu_handlers(app.handle());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
