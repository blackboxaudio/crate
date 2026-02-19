use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::error::{CrateError, Result};
use crate::services::ExportService;

/// Result of a sync operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub success: bool,
    pub tracks_synced: u32,
    pub tracks_skipped: u32,
    pub playlists_synced: Vec<String>,
    pub errors: Vec<String>,
}

/// Service for handling automatic sync of playlists to USB devices
pub struct SyncService {
    conn: Arc<Mutex<Connection>>,
    export_service: Arc<ExportService>,
    /// Flag to signal sync cancellation
    cancel_flag: Arc<AtomicBool>,
    /// Flag to prevent concurrent syncs
    sync_in_progress: Arc<AtomicBool>,
}

impl SyncService {
    pub fn new(conn: Arc<Mutex<Connection>>, export_service: Arc<ExportService>) -> Self {
        Self {
            conn,
            export_service,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            sync_in_progress: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if a sync is currently in progress
    pub fn is_syncing(&self) -> bool {
        self.sync_in_progress.load(Ordering::SeqCst)
    }

    /// Cancel the current sync operation
    pub fn cancel_sync(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Get playlists that have pending changes for a device
    /// A playlist has pending changes if:
    /// 1. It's exported to the device with sync_enabled = true
    /// 2. Either the playlist or any of its tracks have been modified since last_sync_at
    pub fn get_pending_playlists_for_device(&self, device_id: &str) -> Result<Vec<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        // Get all exports for this device that have sync enabled
        let mut stmt = conn.prepare(
            r#"
            SELECT de.playlist_id, de.last_export_at, de.last_sync_at, p.date_modified
            FROM device_exports de
            JOIN playlists p ON de.playlist_id = p.id
            WHERE de.device_id = ?1 AND de.sync_enabled = 1
            "#,
        )?;

        let mut pending_playlists = Vec::new();

        let exports: Vec<(String, String, Option<String>, String)> = stmt
            .query_map([device_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for (playlist_id, last_export_at, last_sync_at, playlist_modified) in exports {
            // Use last_sync_at if available, otherwise use last_export_at
            let last_sync = last_sync_at.unwrap_or(last_export_at);

            // Check if playlist itself was modified
            if playlist_modified > last_sync {
                pending_playlists.push(playlist_id.clone());
                continue;
            }

            // Check if any tracks in the playlist were modified
            let has_modified_tracks: bool = conn
                .query_row(
                    r#"
                    SELECT EXISTS(
                        SELECT 1 FROM playlist_tracks pt
                        JOIN tracks t ON pt.track_id = t.id
                        WHERE pt.playlist_id = ?1 AND t.date_modified > ?2
                    )
                    "#,
                    rusqlite::params![playlist_id, last_sync],
                    |row| row.get(0),
                )
                .unwrap_or(false);

            if has_modified_tracks {
                pending_playlists.push(playlist_id);
            }
        }

        Ok(pending_playlists)
    }

    /// Check if a device has any pending changes
    pub fn has_pending_changes(&self, device_id: &str) -> Result<bool> {
        let pending = self.get_pending_playlists_for_device(device_id)?;
        Ok(!pending.is_empty())
    }

    /// Sync playlists to a device (incremental sync)
    /// This reuses the export service but only syncs changed files
    pub fn sync_device(
        &self,
        app_handle: &AppHandle,
        device_id: &str,
        device_name: &str,
        mount_point: &str,
        playlist_ids: &[String],
    ) -> Result<SyncResult> {
        // Check if already syncing
        if self
            .sync_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(CrateError::Device(
                "A sync operation is already in progress".to_string(),
            ));
        }

        // Reset cancel flag
        self.cancel_flag.store(false, Ordering::SeqCst);

        let result = self.do_sync(
            app_handle,
            device_id,
            device_name,
            mount_point,
            playlist_ids,
        );

        // Always clear the in-progress flag
        self.sync_in_progress.store(false, Ordering::SeqCst);

        result
    }

    fn do_sync(
        &self,
        app_handle: &AppHandle,
        device_id: &str,
        device_name: &str,
        mount_point: &str,
        playlist_ids: &[String],
    ) -> Result<SyncResult> {
        use crate::models::ExportRequest;

        // Create an export request for the sync
        let request = ExportRequest {
            device_id: device_id.to_string(),
            mount_point: mount_point.to_string(),
            device_name: device_name.to_string(),
            playlist_ids: playlist_ids.to_vec(),
            enable_sync: true,
            use_device_library_plus: false,
        };

        // Use the export service to perform the sync
        // The export service already handles:
        // - Skipping unchanged files (via hash comparison)
        // - Updating the PDB
        // - Updating device_tracks records
        let export_result = self.export_service.export_playlists(app_handle, request)?;

        // Update last_sync_at for synced playlists
        if export_result.success {
            self.update_last_sync_at(device_id, playlist_ids)?;
        }

        Ok(SyncResult {
            success: export_result.success,
            tracks_synced: export_result.tracks_copied,
            tracks_skipped: export_result.tracks_skipped,
            playlists_synced: playlist_ids.to_vec(),
            errors: export_result.errors,
        })
    }

    /// Update last_sync_at timestamp for playlists
    fn update_last_sync_at(&self, device_id: &str, playlist_ids: &[String]) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();

        for playlist_id in playlist_ids {
            conn.execute(
                "UPDATE device_exports SET last_sync_at = ?1 WHERE device_id = ?2 AND playlist_id = ?3",
                rusqlite::params![now, device_id, playlist_id],
            )?;
        }

        Ok(())
    }

    /// Get all playlists containing a specific track
    pub fn get_playlists_containing_track(&self, track_id: &str) -> Result<Vec<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut stmt =
            conn.prepare("SELECT DISTINCT playlist_id FROM playlist_tracks WHERE track_id = ?1")?;

        let playlist_ids = stmt
            .query_map([track_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;

        Ok(playlist_ids)
    }

    /// Get all playlists containing any of the specified tracks
    pub fn get_playlists_containing_tracks(&self, track_ids: &[String]) -> Result<Vec<String>> {
        if track_ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let placeholders: Vec<String> = (1..=track_ids.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "SELECT DISTINCT playlist_id FROM playlist_tracks WHERE track_id IN ({})",
            placeholders.join(", ")
        );

        let mut stmt = conn.prepare(&sql)?;

        let params: Vec<&dyn rusqlite::ToSql> = track_ids
            .iter()
            .map(|s| s as &dyn rusqlite::ToSql)
            .collect();

        let playlist_ids = stmt
            .query_map(params.as_slice(), |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;

        Ok(playlist_ids)
    }

    /// Get devices that have exported a specific playlist with sync enabled
    pub fn get_devices_for_playlist(&self, playlist_id: &str) -> Result<Vec<DeviceInfo>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut stmt = conn.prepare(
            r#"
            SELECT DISTINCT device_id, device_name
            FROM device_exports
            WHERE playlist_id = ?1 AND sync_enabled = 1
            "#,
        )?;

        let devices = stmt
            .query_map([playlist_id], |row| {
                Ok(DeviceInfo {
                    device_id: row.get(0)?,
                    device_name: row.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(devices)
    }

    /// Get all devices that have any of the specified playlists exported with sync enabled
    pub fn get_devices_for_playlists(&self, playlist_ids: &[String]) -> Result<Vec<DeviceInfo>> {
        if playlist_ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let placeholders: Vec<String> = (1..=playlist_ids.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            r#"
            SELECT DISTINCT device_id, device_name
            FROM device_exports
            WHERE playlist_id IN ({}) AND sync_enabled = 1
            "#,
            placeholders.join(", ")
        );

        let mut stmt = conn.prepare(&sql)?;

        let params: Vec<&dyn rusqlite::ToSql> = playlist_ids
            .iter()
            .map(|s| s as &dyn rusqlite::ToSql)
            .collect();

        let devices = stmt
            .query_map(params.as_slice(), |row| {
                Ok(DeviceInfo {
                    device_id: row.get(0)?,
                    device_name: row.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(devices)
    }
}

/// Basic device info for sync operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_name: String,
}
