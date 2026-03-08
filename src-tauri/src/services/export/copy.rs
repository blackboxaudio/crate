use std::collections::HashMap;
use std::fs;
use std::path::Path;

use tauri::{AppHandle, Emitter};

use super::*;

impl ExportService {
    /// Create the PIONEER folder structure on the USB device
    pub(super) fn create_pioneer_folders(&self, mount_point: &str) -> Result<()> {
        let pioneer_path = Path::new(mount_point).join("PIONEER").join("rekordbox");
        let usbanlz_path = Path::new(mount_point).join("PIONEER").join("USBANLZ");
        let contents_path = Path::new(mount_point).join("Contents");

        fs::create_dir_all(&pioneer_path)
            .map_err(|e| CrateError::Device(format!("Failed to create PIONEER folder: {e}")))?;

        fs::create_dir_all(&usbanlz_path)
            .map_err(|e| CrateError::Device(format!("Failed to create USBANLZ folder: {e}")))?;

        fs::create_dir_all(&contents_path)
            .map_err(|e| CrateError::Device(format!("Failed to create Contents folder: {e}")))?;

        Ok(())
    }

    /// Copy tracks to the USB device
    pub(super) fn copy_tracks(
        &self,
        app_handle: &AppHandle,
        mount_point: &str,
        tracks: &[Track],
        device_id: &str,
        progress: &mut ExportProgress,
    ) -> Result<Vec<DeviceTrack>> {
        let contents_path = Path::new(mount_point).join("Contents");
        let mut device_tracks = Vec::new();
        let now = chrono::Utc::now().to_rfc3339();

        // Get existing device tracks to check for duplicates
        let existing_tracks = self.get_device_tracks(device_id)?;
        let existing_map: HashMap<String, DeviceTrack> = existing_tracks
            .into_iter()
            .map(|dt| (dt.track_id.clone(), dt))
            .collect();

        for track in tracks {
            // Check for cancellation
            if self.cancel_flag.load(Ordering::SeqCst) {
                return Err(CrateError::Device("Export cancelled".to_string()));
            }

            // Build USB path
            let usb_path = build_usb_path(track);
            let dest_path = contents_path.join(&usb_path);

            // Check if already exported with same hash
            if let Some(existing) = existing_map.get(&track.id) {
                if let Some(ref hash) = track.file_hash {
                    if &existing.file_hash == hash {
                        // Skip - already exported with same content
                        device_tracks.push(existing.clone());
                        continue;
                    }
                }
            }

            // Update progress with track metadata (fall back to filename without extension if unavailable)
            let display_name = match (&track.artist, &track.title) {
                (Some(artist), Some(title)) => format!("{artist} - {title}"),
                (None, Some(title)) => title.clone(),
                _ => {
                    let path = Path::new(&track.file_path);
                    path.file_stem()
                        .unwrap_or_else(|| path.file_name().unwrap_or_default())
                        .to_string_lossy()
                        .to_string()
                }
            };
            *progress = progress.clone().copying(display_name);
            let _ = app_handle.emit("export-progress", &progress);

            // Create parent directories
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| CrateError::Device(format!("Failed to create directory: {e}")))?;
            }

            // Copy the file
            let bytes_copied = match fs::copy(&track.file_path, &dest_path) {
                Ok(bytes) => bytes,
                Err(e) => {
                    log::error!("Failed to copy {}: {}", track.file_path, e);
                    continue;
                }
            };

            // Update progress
            *progress = progress.clone().file_copied(bytes_copied);
            let _ = app_handle.emit("export-progress", &progress);

            // Record device track
            let device_track = DeviceTrack {
                device_id: device_id.to_string(),
                track_id: track.id.clone(),
                usb_path: usb_path.clone(),
                file_hash: track.file_hash.clone().unwrap_or_default(),
                pdb_track_id: None, // Will be set during PDB generation
                exported_at: now.clone(),
                metadata_hash: None,
            };
            device_tracks.push(device_track);
        }

        // Save device tracks to database
        self.save_device_tracks(&device_tracks)?;

        Ok(device_tracks)
    }
}
