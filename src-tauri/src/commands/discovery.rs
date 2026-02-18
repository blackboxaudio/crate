use tauri::State;

use crate::error::Result;
use crate::models::{
    DiscoveryFilter, DiscoveryRelease, DiscoveryReleaseCreate, DiscoveryReleaseUpdate,
};
use crate::services::DiscoveryService;

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
