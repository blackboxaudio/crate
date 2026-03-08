use std::fs;
use std::path::Path;

use super::*;

impl ExportService {
    /// Validate that the device is mounted and writable
    pub(super) fn validate_device(&self, mount_point: &str) -> Result<()> {
        let path = Path::new(mount_point);

        if !path.exists() {
            return Err(CrateError::Device(format!(
                "Device not mounted: {mount_point}"
            )));
        }

        if !path.is_dir() {
            return Err(CrateError::Device(format!(
                "Mount point is not a directory: {mount_point}"
            )));
        }

        // Try to create a test file to verify write access
        let test_file = path.join(".crate_write_test");
        match fs::write(&test_file, b"test") {
            Ok(_) => {
                let _ = fs::remove_file(&test_file);
                Ok(())
            }
            Err(e) => Err(CrateError::Device(format!("Device is not writable: {e}"))),
        }
    }

    /// Check if there's enough space on the device for all tracks
    pub(super) fn check_available_space(&self, mount_point: &str, tracks: &[Track]) -> Result<()> {
        // Calculate total size needed
        let total_size: u64 = tracks
            .iter()
            .filter_map(|t| fs::metadata(&t.file_path).ok())
            .map(|m| m.len())
            .sum();

        // Get available space using sysinfo
        let disks = sysinfo::Disks::new_with_refreshed_list();
        let available = disks
            .iter()
            .find(|d| d.mount_point().to_string_lossy() == mount_point)
            .map(|d| d.available_space())
            .unwrap_or(0);

        // Add 10MB buffer for PDB and other overhead
        let required = total_size + 10 * 1024 * 1024;

        if available < required {
            return Err(CrateError::Device(format!(
                "Not enough space on device. Required: {} MB, Available: {} MB",
                required / (1024 * 1024),
                available / (1024 * 1024)
            )));
        }

        Ok(())
    }
}
