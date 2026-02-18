use std::path::PathBuf;

use tauri::State;

use crate::error::Result;
use crate::models::{
    DiscoveryFilter, DiscoveryRelease, DiscoveryReleaseCreate, DiscoveryReleaseUpdate,
    ImportResultWithDuplicates,
};
use crate::services::discovery::metadata::{self, FetchedMetadata};
use crate::services::{DiscoveryService, LibraryService, TagService};

#[tauri::command]
pub async fn create_discovery_release(
    create: DiscoveryReleaseCreate,
    discovery: State<'_, DiscoveryService>,
) -> Result<DiscoveryRelease> {
    discovery.create_release(create)
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
