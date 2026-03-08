use std::collections::HashMap;

use super::*;

impl ExportService {
    /// Record export state in the database
    pub(super) fn record_export_state(&self, request: &ExportRequest) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let now = chrono::Utc::now().to_rfc3339();

        for playlist_id in &request.playlist_ids {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                r#"
                INSERT INTO device_exports (id, device_id, device_name, playlist_id, last_export_at, sync_enabled)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(device_id, playlist_id) DO UPDATE SET
                    last_export_at = ?5,
                    sync_enabled = ?6
                "#,
                rusqlite::params![
                    id,
                    request.device_id,
                    request.device_name,
                    playlist_id,
                    now,
                    request.enable_sync as i32,
                ],
            )?;
        }

        Ok(())
    }

    /// Save device tracks to database
    pub(super) fn save_device_tracks(&self, device_tracks: &[DeviceTrack]) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        for dt in device_tracks {
            conn.execute(
                r#"
                INSERT INTO device_tracks (device_id, track_id, usb_path, file_hash, pdb_track_id, exported_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(device_id, track_id) DO UPDATE SET
                    usb_path = ?3,
                    file_hash = ?4,
                    pdb_track_id = ?5,
                    exported_at = ?6
                "#,
                rusqlite::params![
                    dt.device_id,
                    dt.track_id,
                    dt.usb_path,
                    dt.file_hash,
                    dt.pdb_track_id,
                    dt.exported_at,
                ],
            )?;
        }

        Ok(())
    }

    /// Update PDB track IDs after generation
    pub(super) fn update_device_track_pdb_ids(
        &self,
        track_pdb_ids: &HashMap<String, u32>,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        for (track_id, pdb_id) in track_pdb_ids {
            conn.execute(
                "UPDATE device_tracks SET pdb_track_id = ?1 WHERE track_id = ?2",
                rusqlite::params![*pdb_id as i32, track_id],
            )?;
        }

        Ok(())
    }

    /// Get all exports for a device
    pub fn get_device_exports(&self, device_id: &str) -> Result<Vec<DeviceExport>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut stmt = conn.prepare(
            r#"
            SELECT id, device_id, device_name, playlist_id, last_export_at, sync_enabled
            FROM device_exports
            WHERE device_id = ?1
            "#,
        )?;

        let exports = stmt
            .query_map([device_id], |row| {
                Ok(DeviceExport {
                    id: row.get(0)?,
                    device_id: row.get(1)?,
                    device_name: row.get(2)?,
                    playlist_id: row.get(3)?,
                    last_export_at: row.get(4)?,
                    sync_enabled: row.get::<_, i32>(5)? != 0,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(exports)
    }

    /// Get all tracks exported to a device
    pub fn get_device_tracks(&self, device_id: &str) -> Result<Vec<DeviceTrack>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut stmt = conn.prepare(
            r#"
            SELECT device_id, track_id, usb_path, file_hash, pdb_track_id, exported_at, metadata_hash
            FROM device_tracks
            WHERE device_id = ?1
            "#,
        )?;

        let tracks = stmt
            .query_map([device_id], |row| {
                Ok(DeviceTrack {
                    device_id: row.get(0)?,
                    track_id: row.get(1)?,
                    usb_path: row.get(2)?,
                    file_hash: row.get(3)?,
                    pdb_track_id: row.get(4)?,
                    exported_at: row.get(5)?,
                    metadata_hash: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tracks)
    }
}
