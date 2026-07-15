// Dual-mode (foreground/headless) access to the Android JVM + Context for JNI bridges.
#[cfg(target_os = "android")]
mod android_context;
mod commands;
mod db;
mod error;
// The native application menu is desktop-only.
#[cfg(feature = "desktop")]
mod menu;
mod models;
mod proxy;
mod services;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use db::Database;

/// Port of the localhost stream proxy HTTP server. Managed as Tauri state so that
/// `fetch_preview_stream` can embed it in the URL it returns to the frontend.
pub(crate) struct ProxyServerPort(pub u16);

/// Global throttle for BACKGROUND stream-URL resolution (playback-queue look-ahead,
/// offline pre-caching). Foreground `fetch_preview_stream` calls never take a permit,
/// so a tap-to-play can't queue behind opportunistic work.
pub(crate) struct StreamFetchPermits(pub Arc<tokio::sync::Semaphore>);

/// Flag to signal cancellation of a running bulk import operation.
pub(crate) struct BulkImportCancelFlag(pub Arc<std::sync::atomic::AtomicBool>);

/// Flag to signal cancellation of a running page scan operation.
pub(crate) struct ScanPageCancelFlag(pub Arc<std::sync::atomic::AtomicBool>);

/// Set of release IDs that should be skipped by background enrichment.
/// Populated when the user cancels enrichment for individual releases.
pub(crate) struct EnrichmentSkipIds(pub Arc<tokio::sync::Mutex<HashSet<String>>>);

impl EnrichmentSkipIds {
    pub fn new() -> Self {
        Self(Arc::new(tokio::sync::Mutex::new(HashSet::new())))
    }
}

/// Session cache of artist/label page avatars (og:image), keyed by page URL, so the follow
/// popover can show a profile picture without re-scraping on every open. Local + ephemeral.
pub(crate) struct AvatarCache(pub Arc<tokio::sync::Mutex<HashMap<String, Option<String>>>>);

impl AvatarCache {
    pub fn new() -> Self {
        Self(Arc::new(tokio::sync::Mutex::new(HashMap::new())))
    }
}

/// Cache for pre-fetched release metadata populated during background enrichment after a page scan.
/// Keyed by release URL. Entries are consumed (removed) by `bulk_create_discovery_releases`.
pub(crate) struct ScanEnrichmentCache(
    pub Arc<tokio::sync::Mutex<HashMap<String, services::discovery::metadata::FetchedMetadata>>>,
);

impl ScanEnrichmentCache {
    pub fn new() -> Self {
        Self(Arc::new(tokio::sync::Mutex::new(HashMap::new())))
    }
}

impl StreamFetchPermits {
    /// Two concurrent background extractions: enough to keep the queue window warm
    /// without hammering the source platforms or the DB while the user is playing.
    pub fn new() -> Self {
        Self(Arc::new(tokio::sync::Semaphore::new(2)))
    }
}

use services::{
    discovery::n_transform::NsigSolverState, BackupService, DiscoveryService, FollowService,
    PlaylistService, SettingsService, TagService,
};
// Desktop-only services and their backing crates are excluded from the mobile build.
#[cfg(feature = "desktop")]
use services::{
    export::CheckpointService, AnalysisService, AudioService, DeviceService, DiagnosticsService,
    ExportService, LibraryService, SyncService,
};
// Cross-platform media session (Now Playing / lock-screen controls): souvlaki on desktop,
// AVAudioSession + MediaPlayer on iOS (#79), no-op elsewhere.
use services::MediaControlsService;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Install a panic hook that writes to a crash log file. On Windows, release builds
    // use `windows_subsystem = "windows"` which hides all console output, so without
    // this hook panics during startup are completely invisible to the user.
    let crash_log_path = std::env::temp_dir().join("crate-crash.log");
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = format!(
            "[{}] PANIC: {}\nLocation: {:?}\n\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            info,
            info.location(),
        );
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(std::env::temp_dir().join("crate-crash.log"))
            .and_then(|mut f| std::io::Write::write_all(&mut f, message.as_bytes()));
        // Also route the panic through `log`: on mobile that's the only channel that reaches the
        // device console (the default hook prints to stderr, which iOS/Android discard).
        log::error!("PANIC: {info}");
        default_hook(info);
    }));

    // Route `log` output where each platform can actually see it. A normally-launched mobile app's
    // stderr is discarded by the OS, so env_logger would be invisible on-device: iOS logs via
    // os_log (Console.app, subsystem `com.bbx-audio.crate`), Android via logcat (tag `crate`).
    // Desktop keeps env_logger on stderr, where `RUST_LOG` still applies.
    #[cfg(target_os = "ios")]
    oslog::OsLogger::new("com.bbx-audio.crate")
        .level_filter(log::LevelFilter::Info)
        .init()
        .ok();
    #[cfg(target_os = "android")]
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("crate"),
    );
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("Crash log path: {crash_log_path:?}");

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init());

    // Desktop-only plugins: the updater (mobile updates via the App Store / TestFlight),
    // the process plugin, and window-state (there are no OS windows to persist on mobile).
    #[cfg(feature = "desktop")]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::default().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_window_state::Builder::default().build());

    // Mobile-only plugins: native web-auth (iOS ASWebAuthenticationSession / Android Custom Tabs)
    // backs the mobile OAuth sign-in flow — there is no loopback browser flow on mobile.
    #[cfg(feature = "mobile")]
    let builder = builder
        .plugin(tauri_plugin_web_auth::init())
        .plugin(tauri_plugin_haptics::init());

    // If the OS kills the WKWebView content process (e.g. iOS jetsam under memory pressure: the
    // screen goes white while native audio keeps playing), reload the webview so the UI recovers
    // instead of staying blank until a manual relaunch. iOS/macOS only — the hook is unsupported
    // on other platforms.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    let builder = builder.on_web_content_process_terminate(|webview| {
        log::warn!("Web content process terminated — reloading webview");
        if let Err(e) = webview.reload() {
            log::error!("Failed to reload webview after content process termination: {e}");
        }
    });

    let builder = builder
        .invoke_handler(tauri::generate_handler![
            // App commands
            commands::app::get_app_info,
            commands::app::open_dev_tools,
            commands::app::close_dev_tools,
            #[cfg(feature = "desktop")]
            commands::app::rebuild_menu,
            #[cfg(feature = "desktop")]
            commands::app::set_menu_item_enabled,
            #[cfg(feature = "desktop")]
            commands::app::set_dialog_conflicting_items_enabled,
            #[cfg(feature = "desktop")]
            commands::app::set_onboarding_items_enabled,
            // Library commands (desktop-only)
            #[cfg(feature = "desktop")]
            commands::library::import_tracks,
            #[cfg(feature = "desktop")]
            commands::library::get_tracks,
            #[cfg(feature = "desktop")]
            commands::library::get_track,
            #[cfg(feature = "desktop")]
            commands::library::update_track,
            #[cfg(feature = "desktop")]
            commands::library::delete_tracks,
            #[cfg(feature = "desktop")]
            commands::library::search_tracks,
            #[cfg(feature = "desktop")]
            commands::library::rescan_artwork,
            #[cfg(feature = "desktop")]
            commands::library::rescan_track_artwork,
            #[cfg(feature = "desktop")]
            commands::library::check_file_exists,
            #[cfg(feature = "desktop")]
            commands::library::validate_replacement_file,
            #[cfg(feature = "desktop")]
            commands::library::relocate_track,
            #[cfg(feature = "desktop")]
            commands::library::set_track_colors,
            #[cfg(feature = "desktop")]
            commands::library::update_tracks,
            #[cfg(feature = "desktop")]
            commands::library::set_track_artwork,
            #[cfg(feature = "desktop")]
            commands::library::delete_track_artwork,
            #[cfg(feature = "desktop")]
            commands::library::reextract_track_artwork,
            #[cfg(feature = "desktop")]
            commands::library::compare_track_artworks,
            #[cfg(feature = "desktop")]
            commands::library::import_tracks_with_duplicates,
            #[cfg(feature = "desktop")]
            commands::library::resolve_duplicate,
            // Playback commands (desktop-only)
            #[cfg(feature = "desktop")]
            commands::playback::play_track,
            #[cfg(feature = "desktop")]
            commands::playback::pause,
            #[cfg(feature = "desktop")]
            commands::playback::resume,
            #[cfg(feature = "desktop")]
            commands::playback::stop,
            #[cfg(feature = "desktop")]
            commands::playback::seek,
            #[cfg(feature = "desktop")]
            commands::playback::set_volume,
            #[cfg(feature = "desktop")]
            commands::playback::set_speed,
            #[cfg(feature = "desktop")]
            commands::playback::get_playback_state,
            #[cfg(feature = "desktop")]
            commands::playback::get_audio_devices,
            #[cfg(feature = "desktop")]
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
            commands::playlist::reorder_playlist_releases,
            commands::playlist::get_playlist_cover_art,
            commands::playlist::create_smart_playlist,
            commands::playlist::update_smart_rules,
            commands::playlist::get_smart_playlist_tracks,
            commands::playlist::get_smart_playlist_releases,
            commands::playlist::preview_smart_rules_count,
            // Settings commands
            commands::settings::get_settings,
            commands::settings::set_setting,
            // Device commands (desktop-only)
            #[cfg(feature = "desktop")]
            commands::device::get_devices,
            #[cfg(feature = "desktop")]
            commands::device::eject_device,
            #[cfg(feature = "desktop")]
            commands::device::reformat_device,
            // Export commands (desktop-only)
            #[cfg(feature = "desktop")]
            commands::export::export_playlists,
            #[cfg(feature = "desktop")]
            commands::export::get_device_exports,
            #[cfg(feature = "desktop")]
            commands::export::cancel_export,
            #[cfg(feature = "desktop")]
            commands::export::cleanup_failed_export,
            #[cfg(feature = "desktop")]
            commands::export::get_pending_checkpoint,
            #[cfg(feature = "desktop")]
            commands::export::delete_checkpoint,
            #[cfg(feature = "desktop")]
            commands::export::resume_export,
            // Sync commands (desktop-only)
            #[cfg(feature = "desktop")]
            commands::sync::sync_device,
            #[cfg(feature = "desktop")]
            commands::sync::get_pending_sync_playlists,
            #[cfg(feature = "desktop")]
            commands::sync::has_pending_sync_changes,
            #[cfg(feature = "desktop")]
            commands::sync::is_syncing,
            #[cfg(feature = "desktop")]
            commands::sync::cancel_sync,
            #[cfg(feature = "desktop")]
            commands::sync::get_playlists_containing_track,
            #[cfg(feature = "desktop")]
            commands::sync::get_playlists_containing_tracks,
            #[cfg(feature = "desktop")]
            commands::sync::get_devices_for_playlist,
            #[cfg(feature = "desktop")]
            commands::sync::get_devices_for_playlists,
            // Diagnostics commands (desktop-only)
            #[cfg(feature = "desktop")]
            commands::diagnostics::get_diagnostic_entries,
            #[cfg(feature = "desktop")]
            commands::diagnostics::get_system_info,
            #[cfg(feature = "desktop")]
            commands::diagnostics::get_diagnostics_report,
            #[cfg(feature = "desktop")]
            commands::diagnostics::clear_diagnostic_entries,
            #[cfg(feature = "desktop")]
            commands::diagnostics::log_error,
            // Analysis commands (desktop-only)
            #[cfg(feature = "desktop")]
            commands::analysis::analyze_tracks,
            #[cfg(feature = "desktop")]
            commands::analysis::cancel_track_analysis,
            #[cfg(feature = "desktop")]
            commands::analysis::cancel_analysis,
            #[cfg(feature = "desktop")]
            commands::analysis::get_analyzed_tracks,
            // Discovery commands
            commands::discovery::toggle_discovery_track_liked,
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
            #[cfg(feature = "desktop")]
            commands::discovery::purchase_discovery_release,
            commands::discovery::fetch_preview_stream,
            commands::discovery::invalidate_preview_stream_cache,
            commands::discovery::purge_release_audio_cache,
            commands::discovery::precache_preview_stream,
            commands::discovery::get_release_cache_state,
            commands::discovery::get_cached_release_states,
            commands::discovery::get_discovery_audio_cache_size,
            commands::discovery::clear_discovery_audio_cache,
            commands::discovery::cache_release_artwork,
            commands::discovery::get_discovery_artwork_cache_size,
            commands::discovery::clear_discovery_artwork_cache,
            commands::discovery::nsig_solve_callback,
            commands::discovery::set_discovery_release_artwork,
            commands::discovery::delete_discovery_release_artwork,
            commands::discovery::scan_discovery_page,
            commands::discovery::bulk_create_discovery_releases,
            commands::discovery::cancel_bulk_import,
            commands::discovery::cancel_scan_page,
            commands::discovery::skip_enrichment,
            commands::discovery::fetch_source_avatar,
            // Follow commands
            commands::follow::follow_source,
            commands::follow::follow_from_entity,
            commands::follow::unfollow_source,
            commands::follow::relink_followed_source,
            commands::follow::set_follow_enabled,
            commands::follow::set_follow_type,
            commands::follow::get_followed_sources,
            commands::follow::check_followed_source,
            commands::follow::check_all_followed_sources,
            commands::follow::set_release_new_flag,
            // Backup commands
            commands::backup::get_backup_info,
            commands::backup::create_backup,
            commands::backup::restore_from_backup,
            // Share-intent intake (#62): Android drains queued shared URLs; empty elsewhere.
            commands::share::take_shared_texts,
            commands::share::share_url,
            // Media controls commands (cross-platform; no-op backend where unsupported)
            commands::media_controls::update_now_playing,
            commands::media_controls::update_playback_state,
            commands::media_controls::clear_now_playing,
            // Native iOS preview playback engine (#54): AVPlayer + lock-screen transport.
            #[cfg(target_os = "ios")]
            commands::native_preview::native_preview_play,
            #[cfg(target_os = "ios")]
            commands::native_preview::native_preview_set_upcoming,
            #[cfg(target_os = "ios")]
            commands::native_preview::native_preview_pause,
            #[cfg(target_os = "ios")]
            commands::native_preview::native_preview_resume,
            #[cfg(target_os = "ios")]
            commands::native_preview::native_preview_seek,
            #[cfg(target_os = "ios")]
            commands::native_preview::native_preview_next,
            #[cfg(target_os = "ios")]
            commands::native_preview::native_preview_previous,
            #[cfg(target_os = "ios")]
            commands::native_preview::native_preview_stop,
            #[cfg(target_os = "ios")]
            commands::native_preview::native_preview_set_volume,
            #[cfg(target_os = "ios")]
            commands::native_preview::native_preview_set_rate,
            #[cfg(target_os = "ios")]
            commands::native_preview::native_preview_set_liked,
            // Cloud sync commands
            #[cfg(feature = "desktop")]
            commands::cloud_sync::sign_in,
            #[cfg(feature = "mobile")]
            commands::cloud_sync::begin_sign_in,
            #[cfg(feature = "mobile")]
            commands::cloud_sync::complete_sign_in,
            commands::cloud_sync::sign_out,
            commands::cloud_sync::get_sync_status,
            commands::cloud_sync::get_sync_diagnostics,
            commands::cloud_sync::sync_now,
            commands::cloud_sync::pull_now,
            commands::cloud_sync::sync_foreground,
            #[cfg(feature = "mobile")]
            commands::cloud_sync::schedule_background_sync,
            #[cfg(feature = "mobile")]
            commands::cloud_sync::cancel_background_sync,
            commands::cloud_sync::get_recent_overrides,
            commands::cloud_sync::list_devices,
            commands::cloud_sync::rename_device,
            commands::cloud_sync::revoke_device,
            commands::cloud_sync::delete_cloud_vault,
            commands::cloud_sync::delete_account,
            commands::cloud_sync::list_library_roots,
            commands::cloud_sync::create_library_root,
            commands::cloud_sync::rename_library_root,
            commands::cloud_sync::remove_library_root,
            commands::cloud_sync::set_library_root_mapping,
            commands::cloud_sync::suggest_library_roots,
            #[cfg(feature = "desktop")]
            commands::cloud_sync::locate_track,
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

            let db = Database::new(db_path).map_err(|e| {
                log::error!("Database initialization failed: {e}");
                e
            })?;
            let conn = db.connection();

            // Initialize services. Desktop-only services (file import/analysis, audio
            // playback, USB export/sync, device detection, diagnostics) are gated out of
            // the mobile build along with their commands and backing crates.
            #[cfg(feature = "desktop")]
            let library_service = LibraryService::new(conn.clone(), app_data_dir.clone());
            let tag_service = TagService::new(conn.clone());
            let playlist_service = PlaylistService::new(conn.clone());
            let settings_service = SettingsService::new(conn.clone());
            #[cfg(feature = "desktop")]
            let export_service = Arc::new(ExportService::new(conn.clone()));
            #[cfg(feature = "desktop")]
            let checkpoint_service = Arc::new(CheckpointService::new(conn.clone()));
            #[cfg(feature = "desktop")]
            let sync_service = SyncService::new(conn.clone(), export_service.clone());
            #[cfg(feature = "desktop")]
            let audio_service = AudioService::new()
                .map_err(|e| format!("Failed to initialize audio service: {e}"))?;
            #[cfg(feature = "desktop")]
            let device_service = DeviceService::new();
            #[cfg(feature = "desktop")]
            let diagnostics_service = DiagnosticsService::new(app_data_dir.clone());
            #[cfg(feature = "desktop")]
            let analysis_service = AnalysisService::new(conn.clone());
            let backup_service = BackupService::new(conn.clone());
            let discovery_service = DiscoveryService::with_db(db.handle(), app_data_dir.clone());
            let follow_service = FollowService::new(conn.clone(), app_data_dir.clone());

            // Load saved audio device setting (desktop-only: no rodio playback on mobile)
            #[cfg(feature = "desktop")]
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

            #[cfg(feature = "desktop")]
            app.manage(library_service);
            app.manage(tag_service);
            app.manage(playlist_service);
            app.manage(settings_service);
            #[cfg(feature = "desktop")]
            app.manage(export_service);
            #[cfg(feature = "desktop")]
            app.manage(checkpoint_service);
            #[cfg(feature = "desktop")]
            app.manage(sync_service);
            #[cfg(feature = "desktop")]
            app.manage(audio_service);
            #[cfg(feature = "desktop")]
            app.manage(device_service);
            #[cfg(feature = "desktop")]
            app.manage(diagnostics_service);
            #[cfg(feature = "desktop")]
            app.manage(analysis_service);
            app.manage(discovery_service);
            app.manage(follow_service);
            // Background watch loop: poll followed sources on the configured cadence.
            // No-ops (and makes no network requests) when nothing is followed.
            crate::services::follow::watch::start_watching(
                app.handle().clone(),
                conn.clone(),
                app_data_dir.clone(),
            );
            app.manage(NsigSolverState::new());
            app.manage(StreamFetchPermits::new());
            app.manage(BulkImportCancelFlag(Arc::new(
                std::sync::atomic::AtomicBool::new(false),
            )));
            app.manage(ScanPageCancelFlag(Arc::new(
                std::sync::atomic::AtomicBool::new(false),
            )));
            app.manage(ScanEnrichmentCache::new());
            app.manage(EnrichmentSkipIds::new());
            app.manage(AvatarCache::new());

            // Cloud sync: build the Firebase backend if a config file is present
            // (degrades gracefully to "unavailable" when it isn't), manage the runtime
            // state, and spawn the session-restore + pull/push/GC sync task.
            {
                use crate::services::cloud_sync::{
                    auth, backend, config, hlc, runtime::CloudSyncState,
                };

                let cloud_config =
                    config::load_cloud_config(app.path().app_config_dir().ok().as_deref())
                        .unwrap_or(None);

                let cloud_backend = cloud_config.as_ref().and_then(|cfg| {
                    match backend::build_default_backend(cfg) {
                        Ok(b) => Some(b),
                        Err(e) => {
                            log::warn!("cloud_sync: backend init failed: {e}");
                            None
                        }
                    }
                });

                let device_id = {
                    let guard = conn.lock().expect("db mutex poisoned");
                    hlc::load_node_id(&guard)
                        .map(|n| format!("{n:08x}"))
                        .unwrap_or_else(|_| "00000000".to_string())
                };
                let device_name = {
                    let guard = conn.lock().expect("db mutex poisoned");
                    auth::read_state(&guard, "device_name")
                        .ok()
                        .flatten()
                        .filter(|s| !s.is_empty())
                }
                .unwrap_or_else(|| {
                    // `sysinfo` is desktop-only; mobile falls back to a generic name until
                    // the device is renamed.
                    #[cfg(feature = "desktop")]
                    {
                        sysinfo::System::host_name().unwrap_or_else(|| "Crate device".to_string())
                    }
                    #[cfg(not(feature = "desktop"))]
                    {
                        "Crate device".to_string()
                    }
                });
                let app_version = app.package_info().version.to_string();

                let cloud_state = Arc::new(CloudSyncState::new(
                    cloud_backend,
                    cloud_config,
                    conn.clone(),
                    device_id,
                    device_name,
                    app_version,
                    app.handle().clone(),
                    &app_data_dir,
                ));
                app.manage(cloud_state.clone());

                // iOS: register the BGTaskScheduler launch handler now (must happen before
                // `didFinishLaunchingWithOptions` returns — `.setup()` runs within it). The
                // handler resolves the managed `CloudSyncState` above when the OS later fires an
                // opportunistic background refresh. The frontend arms the actual schedule after
                // sign-in via `schedule_background_sync`.
                #[cfg(target_os = "ios")]
                services::cloud_sync::background::ios::register(app.handle().clone());

                // Restore any persisted session and run a one-shot startup GC sweep on
                // both platforms. Desktop then enters a serialized poll loop: each tick
                // pulls other devices' changes (~10s) and pushes ours once the dirty queue
                // goes quiescent (~15s after the last mutation). Mobile deliberately has NO
                // such loop — an always-on 5s tick would drain the battery and iOS/Android
                // freeze the process when backgrounded anyway — so it syncs on launch and on
                // every foreground via the `sync_foreground` command instead. "Sync now"
                // pushes immediately via the command on both.
                tauri::async_runtime::spawn(async move {
                    cloud_state.restore_session().await;
                    if !cloud_state.is_available() {
                        return;
                    }
                    // Best-effort GC sweep once per session (signed-in only).
                    if cloud_state.is_signed_in().await {
                        if let Err(e) = cloud_state.run_gc_sweep().await {
                            log::warn!("cloud_sync: gc sweep failed: {e}");
                        }
                    }
                    #[cfg(feature = "desktop")]
                    {
                        let quiescent = std::time::Duration::from_secs(15);
                        let mut tick: u64 = 0;
                        loop {
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                            tick += 1;
                            if !cloud_state.is_signed_in().await {
                                continue;
                            }
                            // Pull every other tick (~10s) to halve manifest reads; the
                            // etag gate keeps an unchanged poll cheap.
                            if tick.is_multiple_of(2) {
                                if let Err(e) = cloud_state.run_pull().await {
                                    // A transient connectivity failure is expected while
                                    // offline (already surfaced as the `Offline` phase) —
                                    // don't spam warnings every poll.
                                    if e.is_transient() {
                                        log::debug!("cloud_sync: pull offline: {e}");
                                    } else {
                                        log::warn!("cloud_sync: pull failed: {e}");
                                    }
                                }
                            }
                            match cloud_state.dirty_quiescent(quiescent) {
                                Ok(true) => {
                                    if let Err(e) = cloud_state.run_push().await {
                                        if e.is_transient() {
                                            log::debug!("cloud_sync: push offline: {e}");
                                        } else {
                                            log::warn!("cloud_sync: debounced push failed: {e}");
                                        }
                                    }
                                }
                                Ok(false) => {}
                                Err(e) => log::warn!("cloud_sync: dirty check failed: {e}"),
                            }
                        }
                    }
                });
            }

            // Start device monitoring (desktop-only: USB device attach/detach)
            #[cfg(feature = "desktop")]
            {
                let device_service = app.state::<DeviceService>();
                device_service.start_monitoring(app.handle().clone());
            }

            // Build and set the application menu (desktop-only: no native menu on mobile)
            #[cfg(feature = "desktop")]
            {
                let menu = menu::build_menu(app.handle())?;
                app.set_menu(menu)?;
                menu::setup_menu_handlers(app.handle());

                // Manage fullscreen label translations for dynamic menu text toggling
                app.manage(std::sync::Mutex::new(menu::FullscreenLabels::default()));
            }

            // Initialize the cross-platform media session (Now Playing / lock-screen controls).
            // The backend is platform-selected: souvlaki on desktop, AVAudioSession + MediaPlayer on
            // iOS, no-op elsewhere. On iOS, constructing it activates the AVAudioSession `.playback`
            // category (#79) so audio can continue while the app is backgrounded.
            {
                let media_controls_service = MediaControlsService::new(app.handle());
                app.manage(media_controls_service);
            }

            // iOS native preview playback engine (#54): drives AVPlayer + the lock-screen transport
            // (prev/next/scrubber that keep working while the WebView's JS is suspended on lock).
            #[cfg(target_os = "ios")]
            app.manage(services::media_controls::NativePreviewEngine::new(
                app.handle().clone(),
            ));

            // Sweep orphaned partial downloads (`*.part`) from a previous crash/kill. Runs once
            // HERE, before the proxy below can start new downloads, so it can never touch a live
            // `.part` — do NOT move this into DiscoveryService construction: background tasks
            // build sibling service instances mid-session, which would race in-flight downloads.
            let streams_dir = app_data_dir.join("discovery").join("streams");
            if let Ok(entries) = std::fs::read_dir(&streams_dir) {
                for entry in entries.flatten() {
                    if entry.path().extension().and_then(|e| e.to_str()) == Some("part") {
                        if let Err(e) = std::fs::remove_file(entry.path()) {
                            log::warn!(
                                "Failed to remove orphaned .part {}: {e}",
                                entry.path().display()
                            );
                        } else {
                            log::info!("Removed orphaned partial download {}", entry.path().display());
                        }
                    }
                }
            }

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
                if let Err(e) = std_listener.set_nonblocking(true) {
                    log::error!("Failed to set proxy listener non-blocking: {e}");
                    return;
                }
                let listener = match tokio::net::TcpListener::from_std(std_listener) {
                    Ok(l) => l,
                    Err(e) => {
                        log::error!("Failed to convert proxy listener to tokio: {e}");
                        return;
                    }
                };

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
        });

    // Fullscreen menu-text tracking via window resize is desktop-only (no native menu on
    // mobile, and no `WindowEvent::Resized` to react to).
    #[cfg(feature = "desktop")]
    let builder = builder.on_window_event(|window, event| {
        // Track fullscreen state changes and update the menu text accordingly.
        // There is no dedicated fullscreen event, so we check on every resize.
        if let tauri::WindowEvent::Resized(_) = event {
            use std::sync::atomic::{AtomicBool, Ordering};
            static WAS_FULLSCREEN: AtomicBool = AtomicBool::new(false);

            let is_fullscreen = window.is_fullscreen().unwrap_or(false);
            let was_fullscreen = WAS_FULLSCREEN.swap(is_fullscreen, Ordering::Relaxed);
            if is_fullscreen != was_fullscreen {
                menu::update_fullscreen_menu_text(window.app_handle(), is_fullscreen);
            }
        }
    });

    let app = builder
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| {
            log::error!("Fatal: failed to run Tauri application: {e}");
            std::process::exit(1);
        });
    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            // WAL: fold the -wal file back into crate.db on clean shutdown so the main
            // file stays self-contained for users who copy it manually.
            if let Some(discovery) = app_handle.try_state::<DiscoveryService>() {
                if let Ok(conn) = discovery.connection().lock() {
                    if let Err(e) = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);") {
                        log::warn!("shutdown wal_checkpoint failed: {e}");
                    }
                }
            }
        }
    });
}
