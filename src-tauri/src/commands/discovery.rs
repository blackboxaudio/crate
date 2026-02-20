use std::path::PathBuf;

use tauri::State;

use crate::error::{CrateError, Result};
use crate::models::{
    DiscoveryFilter, DiscoveryRelease, DiscoveryReleaseCreate, DiscoveryReleaseUpdate,
    DiscoveryTrackCreate, ImportResultWithDuplicates,
};
use crate::services::discovery::metadata::{self, FetchedMetadata};
use crate::services::discovery::streams;
use crate::services::discovery::CachedStream;
use crate::services::{DiscoveryService, LibraryService, TagService};
use crate::ProxyServerPort;

#[tauri::command]
pub async fn create_discovery_release(
    create: DiscoveryReleaseCreate,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    let release = discovery.create_release(create)?;

    // Spawn background prefetch of stream URLs for streamable sources
    if matches!(
        release.source_type.as_str(),
        "bandcamp" | "soundcloud" | "youtube"
    ) && !release.tracks.is_empty()
    {
        let conn = discovery.connection();
        let app_data_dir = discovery.app_data_dir();
        let release_id = release.id.clone();
        let release_url = release.url.clone();
        let source_type = release.source_type.clone();
        tokio::spawn(async move {
            let svc = DiscoveryService::new(conn, app_data_dir);
            if let Err(e) = prefetch_streams(&svc, &release_id, &release_url, &source_type).await {
                log::warn!("Background stream prefetch failed for {release_id}: {e}");
            }
        });
    }

    // Spawn incremental background prefetch for Discogs (per-track, skips cached entries)
    if release.source_type == "discogs" && !release.tracks.is_empty() {
        let conn = discovery.connection();
        let app_data_dir = discovery.app_data_dir();
        let release_id = release.id.clone();
        tokio::spawn(async move {
            let svc = DiscoveryService::new(conn, app_data_dir);
            if let Err(e) = prefetch_discogs_streams(&svc, &release_id).await {
                log::warn!("Background Discogs stream prefetch failed for {release_id}: {e}");
            }
        });
    }

    Ok(release)
}

/// Prefetch and cache stream URLs for a release.
async fn prefetch_streams(
    discovery: &DiscoveryService,
    release_id: &str,
    url: &str,
    source_type: &str,
) -> Result<()> {
    let stream_infos = match source_type {
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
async fn prefetch_discogs_streams(discovery: &DiscoveryService, release_id: &str) -> Result<()> {
    let tracks = discovery.get_all_video_ids_for_release(release_id)?;
    for (position, video_id) in tracks {
        // Skip tracks already cached (e.g. fetched on-demand by a prior user click)
        if discovery.get_cached_stream(release_id, position)?.is_some() {
            continue;
        }
        match streams::extract_single_youtube_stream(&video_id, position).await {
            Ok(stream) => {
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
    discovery: State<'_, DiscoveryService>,
    proxy_port: State<'_, ProxyServerPort>,
) -> Result<String> {
    let port = proxy_port.0;

    // Check cache first
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
            let stream = streams::extract_single_youtube_stream(&video_id, track_position).await?;
            discovery.cache_streams(&release_id, std::slice::from_ref(&stream))?;

            let cached = CachedStream {
                stream_url: stream.stream_url.clone(),
                proxy_ua: stream.proxy_ua.clone(),
            };
            let result = resolve_stream_url(&cached, &release_id, track_position, port);

            // Background re-prefetch all tracks
            let conn = discovery.connection();
            let app_data_dir = discovery.app_data_dir();
            let url = release.url.clone();
            let rid = release_id.clone();
            tokio::spawn(async move {
                let svc = DiscoveryService::new(conn, app_data_dir);
                if let Err(e) = prefetch_streams(&svc, &rid, &url, "youtube").await {
                    log::warn!("Background YouTube re-prefetch failed for {rid}: {e}");
                }
            });

            return Ok(result);
        }
        // Fall through to full extraction for pre-migration releases without video_id
    }

    // Discogs: stream via stored YouTube video_id
    if release.source_type == "discogs" {
        return match discovery.get_video_id_for_track(&release_id, track_position)? {
            Some(video_id) => {
                let stream =
                    streams::extract_single_youtube_stream(&video_id, track_position).await?;
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

    let stream_infos = match release.source_type.as_str() {
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

/// Return a `http://127.0.0.1:{proxy_port}` proxy URL when the stream requires a specific
/// user-agent, or the raw stream URL when it can be played directly by the HTML5 Audio element.
fn resolve_stream_url(
    cached: &CachedStream,
    release_id: &str,
    track_position: i32,
    proxy_port: u16,
) -> String {
    match &cached.proxy_ua {
        Some(_) => format!("http://127.0.0.1:{proxy_port}/{release_id}/{track_position}"),
        None => cached.stream_url.clone(),
    }
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
