//! Tauri command handlers for cloud sync (Phase 2: auth + push).
//!
//! Thin wrappers over [`CloudSyncState`]. The backend/session live in Tauri-managed
//! state (built at startup, gated on a present config file).

use std::sync::Arc;

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{CrateError, Result};
use crate::services::cloud_sync::backend::types::DeviceRecord;
use crate::services::cloud_sync::runtime::{CloudSyncState, SyncStatus};

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

/// List devices registered against the signed-in account.
#[tauri::command]
pub async fn list_devices(state: State<'_, Arc<CloudSyncState>>) -> Result<Vec<DeviceRecord>> {
    state.list_devices().await
}
