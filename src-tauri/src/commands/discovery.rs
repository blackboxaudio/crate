use std::path::PathBuf;
use std::sync::atomic::Ordering;

use tauri::{Emitter, State};

use crate::error::{CrateError, Result};
use crate::models::{
    BulkImportProgress, BulkImportResult, DiscoveryFilter, DiscoveryRelease,
    DiscoveryReleaseCreate, DiscoveryReleaseUpdate, DiscoveryTrackCreate,
    ImportResultWithDuplicates, ScannedPage,
};
use crate::services::discovery::metadata::{self, FetchedMetadata};
use crate::services::discovery::n_transform::{self, NsigSolverState};
use crate::services::discovery::streams::{self, StreamInfo};
use crate::services::discovery::CachedStream;
use crate::services::{DiscoveryService, LibraryService, TagService};
use crate::{BulkImportCancelFlag, PrefetchTracker, ProxyServerPort, ScanPageCancelFlag};

/// Spawn background stream prefetch for a release. Shared by single-create and bulk-create.
async fn spawn_stream_prefetch(
    release: &DiscoveryRelease,
    discovery: &DiscoveryService,
    tracker: &PrefetchTracker,
    app: &tauri::AppHandle,
) {
    if release.tracks.is_empty() {
        return;
    }

    // Streamable sources: bandcamp, soundcloud, youtube
    if matches!(
        release.source_type.as_str(),
        "bandcamp" | "soundcloud" | "youtube"
    ) {
        let mut inflight = tracker.0.lock().await;
        if !inflight.contains(&release.id) {
            inflight.insert(release.id.clone());
            let conn = discovery.connection();
            let app_data_dir = discovery.app_data_dir();
            let release_id = release.id.clone();
            let release_url = release.url.clone();
            let source_type = release.source_type.clone();
            let tracker_ref = tracker.0.clone();
            let app_handle = app.clone();
            tokio::spawn(async move {
                let svc = DiscoveryService::new(conn, app_data_dir.clone());
                if let Err(e) = prefetch_streams(
                    &svc,
                    &release_id,
                    &release_url,
                    &source_type,
                    &app_handle,
                    &app_data_dir,
                )
                .await
                {
                    log::warn!("Background stream prefetch failed for {release_id}: {e}");
                }
                tracker_ref.lock().await.remove(&release_id);
            });
        }
    }

    // Discogs: per-track prefetch via YouTube video IDs
    if release.source_type == "discogs" {
        let mut inflight = tracker.0.lock().await;
        if !inflight.contains(&release.id) {
            inflight.insert(release.id.clone());
            let conn = discovery.connection();
            let app_data_dir = discovery.app_data_dir();
            let release_id = release.id.clone();
            let tracker_ref = tracker.0.clone();
            let app_handle = app.clone();
            tokio::spawn(async move {
                let svc = DiscoveryService::new(conn, app_data_dir.clone());
                if let Err(e) =
                    prefetch_discogs_streams(&svc, &release_id, &app_handle, &app_data_dir).await
                {
                    log::warn!("Background Discogs stream prefetch failed for {release_id}: {e}");
                }
                tracker_ref.lock().await.remove(&release_id);
            });
        }
    }
}

#[tauri::command]
pub async fn create_discovery_release(
    create: DiscoveryReleaseCreate,
    app: tauri::AppHandle,
    discovery: State<'_, DiscoveryService>,
    tracker: State<'_, PrefetchTracker>,
) -> Result<DiscoveryRelease> {
    let release = discovery.create_release(create)?;
    spawn_stream_prefetch(&release, &discovery, &tracker, &app).await;
    Ok(release)
}

/// Prefetch and cache stream URLs for a release.
async fn prefetch_streams(
    discovery: &DiscoveryService,
    release_id: &str,
    url: &str,
    source_type: &str,
    app_handle: &tauri::AppHandle,
    app_data_dir: &std::path::Path,
) -> Result<()> {
    let mut stream_infos = match source_type {
        "bandcamp" => streams::extract_bandcamp_streams(url).await?,
        "soundcloud" => {
            let cached_cid = discovery.get_cached_sc_client_id()?;
            let (infos, new_cid) = streams::extract_soundcloud_streams(url, cached_cid).await?;
            discovery.cache_sc_client_id(&new_cid)?;
            infos
        }
        "youtube" => streams::extract_youtube_streams(url).await?,
        _ => return Ok(()),
    };
    transform_youtube_n_params(&mut stream_infos, app_handle, app_data_dir).await;
    discovery.cache_streams(release_id, &stream_infos)?;
    log::info!(
        "Prefetched {} stream URLs for release {release_id}",
        stream_infos.len()
    );
    Ok(())
}

/// Incrementally prefetch and cache stream URLs for a Discogs release.
///
/// Each track is fetched and cached individually so that tracks become playable
/// as soon as their stream is resolved — without waiting for all tracks to complete.
/// Tracks already cached (e.g. fetched on-demand by a user click) are skipped.
async fn prefetch_discogs_streams(
    discovery: &DiscoveryService,
    release_id: &str,
    app_handle: &tauri::AppHandle,
    app_data_dir: &std::path::Path,
) -> Result<()> {
    let tracks = discovery.get_all_video_ids_for_release(release_id)?;
    for (idx, (position, video_id)) in tracks.iter().enumerate() {
        // Skip tracks already cached (e.g. fetched on-demand by a prior user click)
        if discovery
            .get_cached_stream(release_id, *position)?
            .is_some()
        {
            continue;
        }
        // Randomized delay between requests to avoid YouTube rate limiting / bot detection
        if idx > 0 {
            tokio::time::sleep(metadata::jittered_delay(1500)).await;
        }
        match streams::extract_single_youtube_stream(video_id, *position).await {
            Ok(mut stream) => {
                transform_youtube_n_params(
                    std::slice::from_mut(&mut stream),
                    app_handle,
                    app_data_dir,
                )
                .await;
                if let Err(e) = discovery.cache_streams(release_id, &[stream]) {
                    log::warn!(
                        "Failed to cache Discogs stream for position {position} on {release_id}: {e}"
                    );
                } else {
                    log::info!("Prefetched Discogs stream for position {position} on {release_id}");
                }
            }
            Err(e) => log::warn!(
                "Failed to prefetch Discogs stream for position {position} on {release_id}: {e}"
            ),
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn fetch_preview_stream(
    release_id: String,
    track_position: i32,
    app: tauri::AppHandle,
    discovery: State<'_, DiscoveryService>,
    proxy_port: State<'_, ProxyServerPort>,
    tracker: State<'_, PrefetchTracker>,
) -> Result<String> {
    let port = proxy_port.0;
    let app_data_dir = discovery.app_data_dir();

    // Check if audio bytes are already cached on disk — skip stream URL resolution entirely
    if discovery
        .get_cached_audio_meta(&release_id, track_position)
        .unwrap_or(None)
        .is_some()
    {
        return Ok(format!(
            "http://127.0.0.1:{port}/{release_id}/{track_position}"
        ));
    }

    // Check stream URL cache
    if let Some(cached) = discovery.get_cached_stream(&release_id, track_position)? {
        return Ok(resolve_stream_url(
            &cached,
            &release_id,
            track_position,
            port,
        ));
    }

    // Get release to determine source type and URL
    let release = discovery.get_release(&release_id)?;

    // YouTube fast path: use stored video_id for single-track fetch (~500ms vs ~8s)
    if release.source_type == "youtube" {
        if let Some(video_id) = discovery.get_video_id_for_track(&release_id, track_position)? {
            let mut stream =
                streams::extract_single_youtube_stream(&video_id, track_position).await?;
            transform_youtube_n_params(std::slice::from_mut(&mut stream), &app, &app_data_dir)
                .await;
            discovery.cache_streams(&release_id, std::slice::from_ref(&stream))?;

            let cached = CachedStream {
                stream_url: stream.stream_url.clone(),
                proxy_ua: stream.proxy_ua.clone(),
            };
            let result = resolve_stream_url(&cached, &release_id, track_position, port);

            // Background re-prefetch all tracks (skip if already in-flight)
            let mut inflight = tracker.0.lock().await;
            if !inflight.contains(&release_id) {
                inflight.insert(release_id.clone());
                let conn = discovery.connection();
                let app_data_dir = app_data_dir.clone();
                let url = release.url.clone();
                let rid = release_id.clone();
                let tracker_ref = tracker.0.clone();
                let app_handle = app.clone();
                tokio::spawn(async move {
                    let svc = DiscoveryService::new(conn, app_data_dir.clone());
                    if let Err(e) =
                        prefetch_streams(&svc, &rid, &url, "youtube", &app_handle, &app_data_dir)
                            .await
                    {
                        log::warn!("Background YouTube re-prefetch failed for {rid}: {e}");
                    }
                    tracker_ref.lock().await.remove(&rid);
                });
            }

            return Ok(result);
        }
        // Fall through to full extraction for pre-migration releases without video_id
    }

    // Discogs: stream via stored YouTube video_id
    if release.source_type == "discogs" {
        return match discovery.get_video_id_for_track(&release_id, track_position)? {
            Some(video_id) => {
                let mut stream =
                    streams::extract_single_youtube_stream(&video_id, track_position).await?;
                transform_youtube_n_params(std::slice::from_mut(&mut stream), &app, &app_data_dir)
                    .await;
                discovery.cache_streams(&release_id, std::slice::from_ref(&stream))?;
                let cached = CachedStream {
                    stream_url: stream.stream_url.clone(),
                    proxy_ua: stream.proxy_ua.clone(),
                };
                Ok(resolve_stream_url(
                    &cached,
                    &release_id,
                    track_position,
                    port,
                ))
            }
            None => Err(CrateError::Discovery(
                "No YouTube video available for this Discogs track".into(),
            )),
        };
    }

    let mut stream_infos = match release.source_type.as_str() {
        "bandcamp" => streams::extract_bandcamp_streams(&release.url).await?,
        "soundcloud" => {
            let cached_cid = discovery.get_cached_sc_client_id()?;
            let (infos, new_cid) =
                streams::extract_soundcloud_streams(&release.url, cached_cid).await?;
            discovery.cache_sc_client_id(&new_cid)?;
            infos
        }
        "youtube" => streams::extract_youtube_streams(&release.url).await?,
        other => {
            return Err(CrateError::Discovery(format!(
                "Preview not supported for source type: {other}"
            )));
        }
    };

    transform_youtube_n_params(&mut stream_infos, &app, &app_data_dir).await;

    // Cache all extracted streams
    discovery.cache_streams(&release_id, &stream_infos)?;

    // Return the requested track's URL (direct or proxied)
    let stream = stream_infos
        .iter()
        .find(|s| s.track_position == track_position)
        .ok_or_else(|| {
            CrateError::Discovery(format!(
                "No stream found for track position {track_position}"
            ))
        })?;

    let cached = CachedStream {
        stream_url: stream.stream_url.clone(),
        proxy_ua: stream.proxy_ua.clone(),
    };
    Ok(resolve_stream_url(
        &cached,
        &release_id,
        track_position,
        port,
    ))
}

/// Always route through the localhost proxy for unified disk caching.
/// The proxy uses `proxy_ua` from the stream cache when set (YouTube/Discogs),
/// or a default user-agent when not (Bandcamp/SoundCloud).
fn resolve_stream_url(
    _cached: &CachedStream,
    release_id: &str,
    track_position: i32,
    proxy_port: u16,
) -> String {
    format!("http://127.0.0.1:{proxy_port}/{release_id}/{track_position}")
}

#[tauri::command]
pub async fn get_discovery_audio_cache_size(discovery: State<'_, DiscoveryService>) -> Result<i64> {
    discovery.get_audio_cache_total_size()
}

#[tauri::command]
pub async fn clear_discovery_audio_cache(discovery: State<'_, DiscoveryService>) -> Result<()> {
    discovery.clear_audio_cache()
}

#[tauri::command]
pub async fn invalidate_preview_stream_cache(
    release_id: String,
    discovery: State<'_, DiscoveryService>,
) -> Result<()> {
    discovery.invalidate_stream_cache(&release_id)
}

#[tauri::command]
pub async fn get_discovery_release(
    id: String,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    discovery.get_release(&id)
}

#[tauri::command]
pub async fn get_discovery_releases(
    filter: Option<DiscoveryFilter>,
    discovery: State<'_, DiscoveryService>,
) -> Result<Vec<DiscoveryRelease>> {
    discovery.get_releases(filter)
}

#[tauri::command]
pub async fn update_discovery_release(
    id: String,
    update: DiscoveryReleaseUpdate,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    discovery.update_release(&id, update)
}

#[tauri::command]
pub async fn delete_discovery_release(
    id: String,
    discovery: State<'_, DiscoveryService>,
) -> Result<()> {
    discovery.delete_release(&id)
}

#[tauri::command]
pub async fn delete_discovery_releases(
    ids: Vec<String>,
    discovery: State<'_, DiscoveryService>,
) -> Result<()> {
    discovery.delete_releases(ids)
}

#[tauri::command]
pub async fn assign_discovery_tags(
    release_ids: Vec<String>,
    tag_ids: Vec<String>,
    discovery: State<'_, DiscoveryService>,
) -> Result<()> {
    discovery.assign_tags(release_ids, tag_ids)
}

#[tauri::command]
pub async fn remove_discovery_tags(
    release_ids: Vec<String>,
    tag_ids: Vec<String>,
    discovery: State<'_, DiscoveryService>,
) -> Result<()> {
    discovery.remove_tags(release_ids, tag_ids)
}

#[tauri::command]
pub async fn check_discovery_matches(
    artist: Option<String>,
    title: Option<String>,
    parent_url: Option<String>,
    discovery: State<'_, DiscoveryService>,
) -> Result<Vec<DiscoveryRelease>> {
    discovery.find_matching_releases(artist.as_deref(), title.as_deref(), parent_url.as_deref())
}

#[tauri::command]
pub async fn add_tracks_to_discovery_release(
    release_id: String,
    tracks: Vec<DiscoveryTrackCreate>,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    discovery.add_tracks_to_release(&release_id, tracks)
}

#[tauri::command]
pub async fn merge_discovery_releases(
    target_id: String,
    source_ids: Vec<String>,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    discovery.merge_releases(&target_id, source_ids)
}

#[tauri::command]
pub async fn fetch_release_metadata(url: String) -> Result<FetchedMetadata> {
    metadata::fetch_metadata(&url).await
}

#[tauri::command]
pub async fn refresh_release_metadata(
    id: String,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    let release = discovery.get_release(&id)?;
    let fetched = metadata::fetch_metadata(&release.url).await?;

    // Backfill any missing track durations and video_ids from fetched data
    discovery.update_track_durations(&id, &fetched.tracks)?;
    discovery.update_track_video_ids(&id, &fetched.tracks)?;

    let mut update = DiscoveryReleaseUpdate::default();
    if let Some(artist) = fetched.artist {
        update.artist = Some(artist);
    }
    if let Some(title) = fetched.title {
        update.title = Some(title);
    }
    if let Some(label) = fetched.label {
        update.label = Some(label);
    }
    if let Some(release_date) = fetched.release_date {
        update.release_date = Some(release_date);
    }
    if let Some(artwork_url) = fetched.artwork_url {
        update.artwork_url = Some(artwork_url);
    }

    discovery.update_release(&id, update)
}

#[tauri::command]
pub async fn set_discovery_release_artwork(
    release_id: String,
    file_path: String,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    discovery.set_release_artwork(&release_id, &PathBuf::from(file_path))
}

#[tauri::command]
pub async fn delete_discovery_release_artwork(
    release_id: String,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    discovery.delete_release_artwork(&release_id)
}

#[tauri::command]
pub async fn purchase_discovery_release(
    release_id: String,
    file_paths: Vec<String>,
    transfer_tags: bool,
    remove_after_import: bool,
    discovery: State<'_, DiscoveryService>,
    library: State<'_, LibraryService>,
    tag: State<'_, TagService>,
) -> Result<ImportResultWithDuplicates> {
    let release = discovery.get_release(&release_id)?;

    let pathbufs: Vec<PathBuf> = file_paths.into_iter().map(PathBuf::from).collect();
    let result = library.import_tracks_with_duplicate_detection(pathbufs)?;

    if transfer_tags && !release.tags.is_empty() && !result.tracks.is_empty() {
        let track_ids: Vec<String> = result.tracks.iter().map(|t| t.id.clone()).collect();
        let tag_ids: Vec<String> = release.tags.iter().map(|t| t.id.clone()).collect();
        tag.assign_tags(track_ids, tag_ids)?;
    }

    if remove_after_import {
        discovery.delete_release(&release_id)?;
    }

    Ok(result)
}

/// Callback command invoked from the WebView after the EJS solver completes.
/// Receives the solver result (or error) and unblocks the waiting Rust future.
#[tauri::command]
pub async fn nsig_solve_callback(
    request_id: String,
    result: Option<String>,
    error: Option<String>,
    state: State<'_, NsigSolverState>,
) -> Result<()> {
    let response = error
        .map(|e| format!(r#"{{"type":"error","error":"{e}"}}"#))
        .unwrap_or_else(|| result.unwrap_or_default());
    if let Some(tx) = state.pending.lock().await.remove(&request_id) {
        let _ = tx.send(response);
    }
    Ok(())
}

#[tauri::command]
pub async fn scan_discovery_page(
    url: String,
    discovery: State<'_, DiscoveryService>,
    cancel_flag: State<'_, ScanPageCancelFlag>,
) -> Result<ScannedPage> {
    cancel_flag.0.store(false, Ordering::SeqCst);
    let existing_urls = discovery.get_all_release_urls()?;
    metadata::scan_page(&url, &existing_urls, &cancel_flag.0).await
}

#[tauri::command]
pub async fn bulk_create_discovery_releases(
    urls: Vec<String>,
    page_label: Option<String>,
    page_artist: Option<String>,
    app: tauri::AppHandle,
    discovery: State<'_, DiscoveryService>,
    tracker: State<'_, PrefetchTracker>,
    cancel_flag: State<'_, BulkImportCancelFlag>,
) -> Result<BulkImportResult> {
    // Reset cancel flag
    cancel_flag.0.store(false, Ordering::SeqCst);

    let total = urls.len();
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut failed_urls = Vec::new();

    for (i, url) in urls.iter().enumerate() {
        // Check for cancellation
        if cancel_flag.0.load(Ordering::SeqCst) {
            log::info!("Bulk import cancelled at {}/{total}", i + 1);
            break;
        }

        // Throttle between requests
        if i > 0 {
            let delay = if url.to_lowercase().contains("discogs.com") {
                metadata::jittered_delay(2500)
            } else {
                metadata::jittered_delay(500)
            };
            tokio::time::sleep(delay).await;
        }

        // Fetch full metadata for this URL
        let fetched = match metadata::fetch_metadata(url).await {
            Ok(data) => data,
            Err(e) => {
                log::warn!("Bulk import: failed to fetch metadata for {url}: {e}");
                failed += 1;
                failed_urls.push(url.clone());
                let _ = app.emit(
                    "bulk-import-progress",
                    BulkImportProgress {
                        current: i + 1,
                        total,
                        current_title: None,
                        succeeded,
                        failed,
                    },
                );
                continue;
            }
        };

        // Build the create request
        let create = DiscoveryReleaseCreate {
            url: url.clone(),
            source_type: Some(fetched.source_type.clone()),
            artist: fetched.artist.clone().or(page_artist.clone()),
            title: fetched.title.clone(),
            label: fetched.label.clone().or(page_label.clone()),
            release_date: fetched.release_date.clone(),
            artwork_url: fetched.artwork_url.clone(),
            notes: None,
            parent_url: fetched.parent_url.clone(),
            tracks: if fetched.tracks.is_empty() {
                None
            } else {
                Some(
                    fetched
                        .tracks
                        .iter()
                        .map(|t| DiscoveryTrackCreate {
                            name: t.name.clone(),
                            position: t.position,
                            duration_ms: t.duration_ms,
                            video_id: t.video_id.clone(),
                        })
                        .collect(),
                )
            },
        };

        // Create the release — catch UNIQUE constraint errors as skipped (not failed)
        match discovery.create_release(create) {
            Ok(release) => {
                succeeded += 1;
                spawn_stream_prefetch(&release, &discovery, &tracker, &app).await;
            }
            Err(CrateError::Database(rusqlite::Error::SqliteFailure(err, _)))
                if err.code == rusqlite::ffi::ErrorCode::ConstraintViolation =>
            {
                // Already exists — treat as skipped, not failed
                succeeded += 1;
            }
            Err(e) => {
                log::warn!("Bulk import: failed to create release for {url}: {e}");
                failed += 1;
                failed_urls.push(url.clone());
            }
        }

        let _ = app.emit(
            "bulk-import-progress",
            BulkImportProgress {
                current: i + 1,
                total,
                current_title: fetched.title.clone(),
                succeeded,
                failed,
            },
        );
    }

    Ok(BulkImportResult {
        succeeded,
        failed,
        failed_urls,
    })
}

#[tauri::command]
pub async fn cancel_bulk_import(cancel_flag: State<'_, BulkImportCancelFlag>) -> Result<()> {
    cancel_flag.0.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn cancel_scan_page(cancel_flag: State<'_, ScanPageCancelFlag>) -> Result<()> {
    cancel_flag.0.store(true, Ordering::SeqCst);
    Ok(())
}

/// Transform the `n` query parameter on YouTube CDN stream URLs (IOS client).
/// Only processes streams that have a `proxy_ua` set (i.e. non-browser-compatible).
async fn transform_youtube_n_params(
    streams: &mut [StreamInfo],
    app_handle: &tauri::AppHandle,
    app_data_dir: &std::path::Path,
) {
    for stream in streams.iter_mut() {
        if stream.proxy_ua.is_some() {
            stream.stream_url =
                n_transform::transform_n_param(&stream.stream_url, app_handle, app_data_dir).await;
        }
    }
}
