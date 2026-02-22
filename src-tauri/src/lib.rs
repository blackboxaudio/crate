mod commands;
mod db;
mod error;
mod menu;
mod models;
mod proxy;
mod services;

use std::collections::HashSet;
use std::sync::Arc;

use db::Database;

/// Port of the localhost stream proxy HTTP server. Managed as Tauri state so that
/// `fetch_preview_stream` can embed it in the URL it returns to the frontend.
pub(crate) struct ProxyServerPort(pub u16);

/// Tracks in-flight prefetch tasks by release ID to prevent duplicate spawns.
pub(crate) struct PrefetchTracker(pub Arc<tokio::sync::Mutex<HashSet<String>>>);

impl PrefetchTracker {
    pub fn new() -> Self {
        Self(Arc::new(tokio::sync::Mutex::new(HashSet::new())))
    }
}

use services::{
    discovery::n_transform::NsigSolverState, export::CheckpointService, AnalysisService,
    AudioService, BackupService, DeviceService, DiagnosticsService, DiscoveryService,
    ExportService, LibraryService, MediaControlsService, PlaylistService, SettingsService,
    SyncService, TagService,
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
        .plugin(tauri_plugin_updater::Builder::default().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            // App commands
            commands::app::get_app_info,
            commands::app::open_dev_tools,
            commands::app::close_dev_tools,
            commands::app::rebuild_menu,
            commands::app::set_menu_item_enabled,
            commands::app::set_dialog_conflicting_items_enabled,
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
            commands::playlist::create_smart_playlist,
            commands::playlist::update_smart_rules,
            commands::playlist::get_smart_playlist_tracks,
            commands::playlist::get_smart_playlist_releases,
            commands::playlist::preview_smart_rules_count,
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
            commands::discovery::nsig_solve_callback,
            commands::discovery::set_discovery_release_artwork,
            commands::discovery::delete_discovery_release_artwork,
            // Backup commands
            commands::backup::create_backup,
            commands::backup::restore_from_backup,
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
                .map_err(|e| format!("Failed to get app data directory: {e}"))?;

            // Ensure directory exists
            std::fs::create_dir_all(&app_data_dir)?;

            let db_path = app_data_dir.join("crate.db");
            log::info!("Database path: {db_path:?}");

            let db = Database::new(db_path)?;
            let conn = db.connection();

            // Initialize services
            let library_service = LibraryService::new(conn.clone(), app_data_dir.clone());
            let tag_service = TagService::new(conn.clone());
            let playlist_service = PlaylistService::new(conn.clone());
            let settings_service = SettingsService::new(conn.clone());
            let export_service = Arc::new(ExportService::new(conn.clone()));
            let checkpoint_service = Arc::new(CheckpointService::new(conn.clone()));
            let sync_service = SyncService::new(conn.clone(), export_service.clone());
            let audio_service = AudioService::new()
                .map_err(|e| format!("Failed to initialize audio service: {e}"))?;
            let device_service = DeviceService::new();
            let diagnostics_service = DiagnosticsService::new(app_data_dir.clone());
            let analysis_service = AnalysisService::new(conn.clone());
            let backup_service = BackupService::new(conn.clone());
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
            app.manage(backup_service);

            // Spawn auto-backup check (runs in background, does not block startup)
            {
                let conn = conn.clone();
                let app_handle = app.handle().clone();
                let app_version = app.package_info().version.to_string();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = crate::services::backup::run_auto_backup_if_due(
                        conn,
                        app_handle,
                        app_version,
                    )
                    .await
                    {
                        log::warn!("Auto-backup failed: {e}");
                    }
                });
            }

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
            app.manage(NsigSolverState::new());
            app.manage(PrefetchTracker::new());

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

            // Bind a stream proxy HTTP server on a random OS-assigned port. Real HTTP is required
            // for WKWebView's AVFoundation media layer to correctly handle Range requests during
            // seeking; WKWebView's custom URI scheme handler (WKURLSchemeHandler) does not reliably
            // support the multi-request, cancellation-heavy lifecycle that AVFoundation uses.
            let std_listener = std::net::TcpListener::bind("127.0.0.1:0")
                .map_err(|e| format!("Failed to bind stream proxy: {e}"))?;
            let proxy_port = std_listener
                .local_addr()
                .map_err(|e| format!("Failed to get proxy address: {e}"))?
                .port();
            log::info!("Stream proxy HTTP server bound to 127.0.0.1:{proxy_port}");
            app.manage(ProxyServerPort(proxy_port));

            // Force HTTP/1.1 with no idle connection pooling. HTTP/2 multiplexes all requests
            // over one TCP connection; when AVFoundation drops a body mid-stream (seek), hyper
            // sends RST_STREAM and transitions the connection back to idle — but if the next
            // Range request arrives before that cleanup completes, hyper tries to reuse the
            // half-torn-down connection and fails with "error sending request". HTTP/1.1 +
            // pool_max_idle_per_host(0) guarantees a fresh TCP+TLS connection per request,
            // which is safe because each 1 MB chunk holds ~60 s of audio.
            let proxy_client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .pool_max_idle_per_host(0)
                .http1_only()
                .build()
                .map_err(|e| format!("Failed to build proxy client: {e}"))?;

            let proxy_state = proxy::ProxyServerState::new(app.handle().clone(), proxy_client);

            tauri::async_runtime::spawn(async move {
                std_listener
                    .set_nonblocking(true)
                    .expect("Failed to set proxy listener non-blocking");
                let listener = tokio::net::TcpListener::from_std(std_listener)
                    .expect("Failed to convert proxy listener to tokio");

                let router = axum::Router::new()
                    .route(
                        "/:release_id/:track_position",
                        axum::routing::get(proxy::proxy_http_handler)
                            .options(proxy::proxy_cors_preflight_handler),
                    )
                    .with_state(proxy_state);

                if let Err(e) = axum::serve(listener, router).await {
                    log::error!("Stream proxy HTTP server error: {e}");
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
