mod commands;
mod db;
mod error;
mod menu;
mod models;
mod services;

use std::sync::Arc;

use db::Database;
use error::CrateError;
use services::{
    export::CheckpointService, AnalysisService, AudioService, DeviceService, DiagnosticsService,
    DiscoveryService, ExportService, LibraryService, MediaControlsService, PlaylistService,
    SettingsService, SyncService, TagService,
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
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .register_asynchronous_uri_scheme_protocol("crate-stream", |ctx, request, responder| {
            let app_handle = ctx.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                let response = handle_crate_stream_proxy(&app_handle, &request).await;
                responder.respond(response);
            });
        })
        .invoke_handler(tauri::generate_handler![
            // App commands
            commands::app::get_app_info,
            commands::app::open_dev_tools,
            commands::app::close_dev_tools,
            commands::app::rebuild_menu,
            commands::app::set_menu_item_enabled,
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
            commands::library::compare_track_artworks,
            commands::library::import_tracks_with_duplicates,
            commands::library::resolve_duplicate,
            // Playback commands
            commands::playback::play_track,
            commands::playback::pause,
            commands::playback::resume,
            commands::playback::stop,
            commands::playback::seek,
            commands::playback::set_volume,
            commands::playback::set_speed,
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
            commands::tag::move_tag,
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
            commands::playlist::add_releases_to_playlist,
            commands::playlist::remove_releases_from_playlist,
            commands::playlist::get_playlist_releases,
            // Settings commands
            commands::settings::get_settings,
            commands::settings::set_setting,
            // Device commands
            commands::device::get_devices,
            commands::device::eject_device,
            commands::device::reformat_device,
            // Export commands
            commands::export::export_playlists,
            commands::export::get_device_exports,
            commands::export::cancel_export,
            commands::export::cleanup_failed_export,
            commands::export::get_pending_checkpoint,
            commands::export::delete_checkpoint,
            commands::export::resume_export,
            // Sync commands
            commands::sync::sync_device,
            commands::sync::get_pending_sync_playlists,
            commands::sync::has_pending_sync_changes,
            commands::sync::is_syncing,
            commands::sync::cancel_sync,
            commands::sync::get_playlists_containing_track,
            commands::sync::get_playlists_containing_tracks,
            commands::sync::get_devices_for_playlist,
            commands::sync::get_devices_for_playlists,
            // Diagnostics commands
            commands::diagnostics::get_diagnostic_entries,
            commands::diagnostics::get_system_info,
            commands::diagnostics::get_diagnostics_report,
            commands::diagnostics::clear_diagnostic_entries,
            commands::diagnostics::log_error,
            // Analysis commands
            commands::analysis::analyze_tracks,
            commands::analysis::cancel_track_analysis,
            commands::analysis::cancel_analysis,
            commands::analysis::get_analyzed_tracks,
            // Discovery commands
            commands::discovery::create_discovery_release,
            commands::discovery::get_discovery_release,
            commands::discovery::get_discovery_releases,
            commands::discovery::update_discovery_release,
            commands::discovery::delete_discovery_release,
            commands::discovery::delete_discovery_releases,
            commands::discovery::assign_discovery_tags,
            commands::discovery::remove_discovery_tags,
            commands::discovery::check_discovery_matches,
            commands::discovery::add_tracks_to_discovery_release,
            commands::discovery::merge_discovery_releases,
            commands::discovery::fetch_release_metadata,
            commands::discovery::refresh_release_metadata,
            commands::discovery::purchase_discovery_release,
            commands::discovery::fetch_preview_stream,
            commands::discovery::invalidate_preview_stream_cache,
            commands::discovery::set_discovery_release_artwork,
            commands::discovery::delete_discovery_release_artwork,
            // Media controls commands
            commands::media_controls::update_now_playing,
            commands::media_controls::update_playback_state,
            commands::media_controls::clear_now_playing,
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
            let export_service = Arc::new(ExportService::new(conn.clone()));
            let checkpoint_service = Arc::new(CheckpointService::new(conn.clone()));
            let sync_service = SyncService::new(conn.clone(), export_service.clone());
            let audio_service = AudioService::new().expect("Failed to initialize audio service");
            let device_service = DeviceService::new();
            let diagnostics_service = DiagnosticsService::new(app_data_dir.clone());
            let analysis_service = AnalysisService::new(conn.clone());
            let discovery_service = DiscoveryService::new(conn.clone(), app_data_dir.clone());

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
            app.manage(export_service);
            app.manage(checkpoint_service);
            app.manage(sync_service);
            app.manage(audio_service);
            app.manage(device_service);
            app.manage(diagnostics_service);
            app.manage(analysis_service);
            app.manage(discovery_service);

            // Start device monitoring
            let device_service = app.state::<DeviceService>();
            device_service.start_monitoring(app.handle().clone());

            // Build and set the application menu
            let menu = menu::build_menu(app.handle())?;
            app.set_menu(menu)?;
            menu::setup_menu_handlers(app.handle());

            // Initialize media controls (Now Playing / media key integration)
            let media_controls_service = MediaControlsService::new(app.handle());
            app.manage(media_controls_service);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Proxy handler for `crate-stream://` custom protocol.
///
/// YouTube stream URLs signed for non-browser clients (IOS, TVHTML5) contain a client
/// identifier that YouTube's CDN validates against the requesting user-agent. The HTML5
/// Audio element sends a browser UA, causing a 403. This handler fetches the stream
/// server-side with the correct UA and forwards the bytes to the WebView.
async fn handle_crate_stream_proxy(
    app_handle: &tauri::AppHandle,
    request: &tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    match handle_crate_stream_proxy_inner(app_handle, request).await {
        Ok(response) => response,
        Err(e) => {
            log::error!("crate-stream proxy error: {e}");
            tauri::http::Response::builder()
                .status(502)
                .header("Content-Type", "text/plain")
                .body(format!("Stream proxy error: {e}").into_bytes())
                .unwrap()
        }
    }
}

async fn handle_crate_stream_proxy_inner(
    app_handle: &tauri::AppHandle,
    request: &tauri::http::Request<Vec<u8>>,
) -> error::Result<tauri::http::Response<Vec<u8>>> {
    // Parse path: /{release_id}/{track_position}
    let path = request.uri().path();
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if segments.len() != 2 {
        return Ok(tauri::http::Response::builder()
            .status(400)
            .header("Content-Type", "text/plain")
            .body(b"Expected path: /{release_id}/{track_position}".to_vec())
            .unwrap());
    }

    let release_id = segments[0];
    let track_position: i32 = segments[1].parse().map_err(|_| {
        CrateError::Discovery("Invalid track position in proxy URL".into())
    })?;

    // Look up cached stream (must exist — the command layer only returns crate-stream:// URLs
    // when it has just cached the stream with a proxy_ua)
    let discovery = app_handle.state::<DiscoveryService>();
    let cached = discovery
        .get_cached_stream(release_id, track_position)?
        .ok_or_else(|| CrateError::Discovery("No cached stream for proxy request".into()))?;

    let proxy_ua = cached.proxy_ua.ok_or_else(|| {
        CrateError::Discovery("Cached stream has no proxy_ua — should use direct URL".into())
    })?;

    // Fetch the YouTube stream with the correct user-agent
    let client = reqwest::Client::builder()
        .user_agent(&proxy_ua)
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| CrateError::Discovery(format!("Failed to build proxy client: {e}")))?;

    let mut yt_request = client.get(&cached.stream_url);

    // Forward Range header to support seeking in the Audio element
    if let Some(range) = request.headers().get("Range") {
        yt_request = yt_request.header("Range", range.to_str().unwrap_or(""));
    }

    let yt_response = yt_request
        .send()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to fetch YouTube stream: {e}")))?;

    if !yt_response.status().is_success() && yt_response.status().as_u16() != 206 {
        return Ok(tauri::http::Response::builder()
            .status(502)
            .header("Content-Type", "text/plain")
            .body(format!("YouTube CDN returned {}", yt_response.status()).into_bytes())
            .unwrap());
    }

    let status = yt_response.status().as_u16();
    let content_type = yt_response
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("audio/mp4")
        .to_string();
    let content_range = yt_response
        .headers()
        .get("Content-Range")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let bytes = yt_response
        .bytes()
        .await
        .map_err(|e| CrateError::Discovery(format!("Failed to read YouTube stream bytes: {e}")))?;

    let mut builder = tauri::http::Response::builder()
        .status(status)
        .header("Content-Type", content_type)
        .header("Access-Control-Allow-Origin", "*")
        .header("Content-Length", bytes.len().to_string());

    if let Some(cr) = content_range {
        builder = builder.header("Content-Range", cr);
    }

    Ok(builder.body(bytes.to_vec()).unwrap())
}
