use std::fs;
use std::path::Path;

use super::*;

impl ExportService {
    /// Clean up a failed export by removing copied files
    pub fn cleanup_failed_export(&self, device_id: &str, mount_point: &str) -> Result<()> {
        // Get all tracks that were exported to this device
        let device_tracks = self.get_device_tracks(device_id)?;

        let contents_path = Path::new(mount_point).join("Contents");

        // Remove each file
        for dt in &device_tracks {
            let file_path = contents_path.join(&dt.usb_path);
            if file_path.exists() {
                let _ = fs::remove_file(&file_path);
            }
        }

        // Clean up empty directories
        cleanup_empty_dirs(&contents_path);

        // Remove from database
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        conn.execute(
            "DELETE FROM device_tracks WHERE device_id = ?1",
            [device_id],
        )?;
        conn.execute(
            "DELETE FROM device_exports WHERE device_id = ?1",
            [device_id],
        )?;

        Ok(())
    }
}
