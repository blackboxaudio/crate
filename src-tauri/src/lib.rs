mod commands;
mod db;
mod error;
mod models;
mod services;

use std::path::PathBuf;

use db::Database;
use services::{AudioService, LibraryService, PlaylistService, TagService};

fn get_db_path() -> PathBuf {
    let app_data = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.crate.app");
    app_data.join("crate.db")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Initialize database
    let db_path = get_db_path();
    log::info!("Database path: {db_path:?}");

    let db = Database::new(db_path).expect("Failed to initialize database");
    let conn = db.connection();

    // Initialize services
    let library_service = LibraryService::new(conn.clone());
    let tag_service = TagService::new(conn.clone());
    let playlist_service = PlaylistService::new(conn.clone());
    let audio_service = AudioService::new().expect("Failed to initialize audio service");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(library_service)
        .manage(tag_service)
        .manage(playlist_service)
        .manage(audio_service)
        .invoke_handler(tauri::generate_handler![
            // Library commands
            commands::library::import_tracks,
            commands::library::get_tracks,
            commands::library::get_track,
            commands::library::update_track,
            commands::library::delete_tracks,
            commands::library::search_tracks,
            // Playback commands
            commands::playback::play_track,
            commands::playback::pause,
            commands::playback::resume,
            commands::playback::stop,
            commands::playback::seek,
            commands::playback::set_volume,
            commands::playback::get_playback_state,
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
            commands::playlist::get_playlist_tracks,
            commands::playlist::add_to_playlist,
            commands::playlist::remove_from_playlist,
            commands::playlist::reorder_playlist,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
