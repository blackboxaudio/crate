//! Tauri command handlers for cloud sync.
//!
//! Thin wrappers over [`CloudSyncState`] and the resolution/library-root helpers.
//! The backend/session live in Tauri-managed state (built at startup, gated on a
//! present config file).

use std::sync::Arc;

use rusqlite::OptionalExtension;
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{CrateError, Result};
use crate::services::cloud_sync::backend::types::DeviceRecord;
use crate::services::cloud_sync::resolution;
use crate::services::cloud_sync::runtime::{CloudSyncState, OverrideNotice, SyncStatus};
use crate::services::LibraryService;

// =============================================================================
// Auth + sync commands (Phase 2)
// =============================================================================

/// Sign in with an identity provider (v1: `"google"`). Opens the system browser for
/// the consent screen; Crate never sees the user's password.
#[tauri::command]
pub async fn sign_in(
    provider_id: String,
    state: State<'_, Arc<CloudSyncState>>,
    app_handle: AppHandle,
) -> Result<SyncStatus> {
    let app = app_handle.clone();
    let open_url = move |url: &str| -> Result<()> {
        app.opener()
            .open_url(url.to_string(), None::<String>)
            .map_err(|e| CrateError::CloudSyncAuth(format!("failed to open browser: {e}")))
    };
    state.sign_in(&provider_id, open_url).await
}

/// Sign out and clear the stored refresh token.
#[tauri::command]
pub async fn sign_out(state: State<'_, Arc<CloudSyncState>>) -> Result<()> {
    state.sign_out().await
}

/// Current sync status (phase, account email, device identity, last sync/error).
#[tauri::command]
pub async fn get_sync_status(state: State<'_, Arc<CloudSyncState>>) -> Result<SyncStatus> {
    Ok(state.get_status().await)
}

/// Trigger an immediate push ("Sync now").
#[tauri::command]
pub async fn sync_now(state: State<'_, Arc<CloudSyncState>>) -> Result<()> {
    state.run_push().await
}

/// Trigger an immediate pull.
#[tauri::command]
pub async fn pull_now(state: State<'_, Arc<CloudSyncState>>) -> Result<()> {
    state.run_pull().await
}

/// The recent override notices kept in memory (diagnostics; no audit-log UI in v1).
#[tauri::command]
pub async fn get_recent_overrides(
    state: State<'_, Arc<CloudSyncState>>,
) -> Result<Vec<OverrideNotice>> {
    Ok(state.recent_overrides().await)
}

/// List devices registered against the signed-in account.
#[tauri::command]
pub async fn list_devices(state: State<'_, Arc<CloudSyncState>>) -> Result<Vec<DeviceRecord>> {
    state.list_devices().await
}

/// Rename this device (updates the local name + best-effort heartbeat).
#[tauri::command]
pub async fn rename_device(
    name: String,
    state: State<'_, Arc<CloudSyncState>>,
) -> Result<()> {
    state.rename_device(&name).await;
    Ok(())
}

/// Revoke a device. If `device_id` is the current device, also signs out.
#[tauri::command]
pub async fn revoke_device(
    device_id: String,
    state: State<'_, Arc<CloudSyncState>>,
) -> Result<()> {
    state.revoke_device(&device_id).await
}

/// Delete the user's entire cloud vault (manifest + devices + GC + all blobs) and sign
/// out. Local library data is untouched.
#[tauri::command]
pub async fn delete_cloud_vault(state: State<'_, Arc<CloudSyncState>>) -> Result<()> {
    state.delete_cloud_vault().await
}

// =============================================================================
// Library roots (Phase 4)
// =============================================================================

#[derive(Clone, Debug, Serialize)]
pub struct LibraryRootInfo {
    pub id: String,
    pub name: String,
    pub local_path: Option<String>,
}

/// List all library roots with their device-local mapping (if any).
#[tauri::command]
pub async fn list_library_roots(
    state: State<'_, Arc<CloudSyncState>>,
) -> Result<Vec<LibraryRootInfo>> {
    state.with_conn(|conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT lr.id, lr.name, srm.local_absolute_path
            FROM library_roots lr
            LEFT JOIN sync_root_mappings srm ON lr.id = srm.library_root_id
            ORDER BY lr.name
            "#,
        )?;
        let roots = stmt
            .query_map([], |row| {
                Ok(LibraryRootInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    local_path: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(roots)
    })
}

/// Create a new library root (synced).
#[tauri::command]
pub async fn create_library_root(
    name: String,
    state: State<'_, Arc<CloudSyncState>>,
) -> Result<String> {
    state.with_conn(|conn| resolution::register_root(conn, &name))
}

/// Rename a library root (synced).
#[tauri::command]
pub async fn rename_library_root(
    id: String,
    name: String,
    state: State<'_, Arc<CloudSyncState>>,
) -> Result<()> {
    state.with_conn(|conn| resolution::rename_root(conn, &id, &name))
}

/// Delete a library root (synced).
#[tauri::command]
pub async fn remove_library_root(
    id: String,
    state: State<'_, Arc<CloudSyncState>>,
) -> Result<()> {
    state.with_conn(|conn| resolution::remove_root(conn, &id))
}

/// Set the device-local folder mapping for a library root.
#[tauri::command]
pub async fn set_library_root_mapping(
    root_id: String,
    local_path: String,
    state: State<'_, Arc<CloudSyncState>>,
) -> Result<()> {
    state.with_conn(|conn| resolution::set_root_mapping(conn, &root_id, &local_path))
}

/// Suggest library root paths by finding common prefixes of existing track paths.
#[tauri::command]
pub async fn suggest_library_roots(
    state: State<'_, Arc<CloudSyncState>>,
) -> Result<Vec<String>> {
    state.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT file_path FROM tracks WHERE library_root_id IS NULL LIMIT 2000",
        )?;
        let paths: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(find_common_prefixes(&paths))
    })
}

/// Locate a track whose file is unavailable. If the track has a `library_root_id`,
/// sets that root's local mapping (all tracks under it become available). Otherwise
/// falls back to relocating the individual track.
#[tauri::command]
pub async fn locate_track(
    track_id: String,
    local_path: String,
    state: State<'_, Arc<CloudSyncState>>,
    library: State<'_, LibraryService>,
) -> Result<()> {
    let root_id: Option<String> = state.with_conn(|conn| {
        let outer: Option<Option<String>> = conn
            .query_row(
                "SELECT library_root_id FROM tracks WHERE id = ?1",
                [&track_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?;
        Ok(outer.flatten())
    })?;

    match root_id {
        Some(rid) => {
            // The user pointed us to a directory; set that as the root mapping.
            state.with_conn(|conn| resolution::set_root_mapping(conn, &rid, &local_path))
        }
        None => {
            // No root association — relocate the individual track file.
            library.relocate_track(&track_id, std::path::Path::new(&local_path), false)?;
            Ok(())
        }
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Derive common directory prefixes from a list of absolute file paths.
fn find_common_prefixes(paths: &[String]) -> Vec<String> {
    use std::collections::HashMap;
    use std::path::Path;

    let mut dir_counts: HashMap<String, usize> = HashMap::new();
    for p in paths {
        if let Some(parent) = Path::new(p).parent() {
            let dir = parent.to_string_lossy().to_string();
            *dir_counts.entry(dir).or_insert(0) += 1;
        }
    }

    // Keep directories that contain at least 3 tracks, then collapse to the
    // shortest prefix that still captures the majority.
    let mut prefixes: Vec<(String, usize)> = dir_counts
        .into_iter()
        .filter(|(_, count)| *count >= 3)
        .collect();
    prefixes.sort_by(|a, b| b.1.cmp(&a.1));

    // Deduplicate: if a parent of an entry is already in the list, skip the child.
    let mut result: Vec<String> = Vec::new();
    for (dir, _) in &prefixes {
        let dominated = result.iter().any(|existing| dir.starts_with(existing.as_str()));
        if !dominated {
            result.push(dir.clone());
        }
        if result.len() >= 5 {
            break;
        }
    }
    result
}
