use std::path::PathBuf;

use tauri::State;

use crate::error::{CrateError, Result};
use crate::models::{
    DiscoveryFilter, DiscoveryRelease, DiscoveryReleaseCreate, DiscoveryReleaseUpdate,
    DiscoveryTrackCreate, ImportResultWithDuplicates,
};
use crate::services::discovery::metadata::{self, FetchedMetadata};
use crate::services::discovery::streams;
use crate::services::{DiscoveryService, LibraryService, TagService};

#[tauri::command]
pub async fn create_discovery_release(
    create: DiscoveryReleaseCreate,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    let release = discovery.create_release(create)?;

    // Spawn background prefetch of stream URLs for streamable sources
    if matches!(release.source_type.as_str(), "bandcamp" | "soundcloud")
        && !release.tracks.is_empty()
    {
        let conn = discovery.connection();
        let release_id = release.id.clone();
        let release_url = release.url.clone();
        let source_type = release.source_type.clone();
        tokio::spawn(async move {
            let svc = DiscoveryService::new(conn);
            if let Err(e) = prefetch_streams(&svc, &release_id, &release_url, &source_type).await {
                log::warn!("Background stream prefetch failed for {release_id}: {e}");
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
        _ => return Ok(()),
    };
    discovery.cache_streams(release_id, &stream_infos)?;
    log::info!(
        "Prefetched {} stream URLs for release {release_id}",
        stream_infos.len()
    );
    Ok(())
}

#[tauri::command]
pub async fn fetch_preview_stream(
    release_id: String,
    track_position: i32,
    discovery: State<'_, DiscoveryService>,
) -> Result<String> {
    // Check cache first
    if let Some(url) = discovery.get_cached_stream(&release_id, track_position)? {
        return Ok(url);
    }

    // Get release to determine source type and URL
    let release = discovery.get_release(&release_id)?;

    let stream_infos = match release.source_type.as_str() {
        "bandcamp" => streams::extract_bandcamp_streams(&release.url).await?,
        "soundcloud" => {
            let cached_cid = discovery.get_cached_sc_client_id()?;
            let (infos, new_cid) =
                streams::extract_soundcloud_streams(&release.url, cached_cid).await?;
            discovery.cache_sc_client_id(&new_cid)?;
            infos
        }
        other => {
            return Err(CrateError::Discovery(format!(
                "Preview not supported for source type: {other}"
            )));
        }
    };

    // Cache all extracted streams
    discovery.cache_streams(&release_id, &stream_infos)?;

    // Return the requested track's URL
    stream_infos
        .iter()
        .find(|s| s.track_position == track_position)
        .map(|s| s.stream_url.clone())
        .ok_or_else(|| {
            CrateError::Discovery(format!(
                "No stream found for track position {track_position}"
            ))
        })
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

    // Backfill any missing track durations from fetched data
    discovery.update_track_durations(&id, &fetched.tracks)?;

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
