use std::collections::HashMap;
use std::fs;

use tauri::{AppHandle, Emitter};

use super::*;

impl ExportService {
    /// Main entry point for exporting playlists to a device
    pub fn export_playlists(
        &self,
        app_handle: &AppHandle,
        request: ExportRequest,
    ) -> Result<ExportResult> {
        // Reset cancel flag
        self.cancel_flag.store(false, Ordering::SeqCst);

        // Validate device
        self.validate_device(&request.mount_point)?;

        // Collect all tracks from selected playlists (including folder contents)
        let playlists_with_tracks = self.collect_tracks_for_export(&request.playlist_ids)?;

        // Get unique tracks across all playlists
        let mut unique_tracks: HashMap<String, Track> = HashMap::new();
        for (_, tracks) in &playlists_with_tracks {
            for track in tracks {
                unique_tracks.insert(track.id.clone(), track.clone());
            }
        }
        let all_tracks: Vec<Track> = unique_tracks.values().cloned().collect();

        // Check available space
        self.check_available_space(&request.mount_point, &all_tracks)?;

        // Create PIONEER folder structure
        self.create_pioneer_folders(&request.mount_point)?;

        // Calculate total bytes
        let total_bytes: u64 = all_tracks
            .iter()
            .filter_map(|t| fs::metadata(&t.file_path).ok())
            .map(|m| m.len())
            .sum();

        // Initialize progress
        let mut progress = ExportProgress::new(all_tracks.len() as u32, total_bytes);
        let _ = app_handle.emit("export-progress", &progress);

        // Copy tracks to USB
        let device_tracks = self.copy_tracks(
            app_handle,
            &request.mount_point,
            &all_tracks,
            &request.device_id,
            &mut progress,
        )?;

        // Check for cancellation before PDB generation
        if self.cancel_flag.load(Ordering::SeqCst) {
            return Ok(ExportResult {
                success: false,
                tracks_copied: progress.files_copied,
                tracks_skipped: 0,
                errors: vec!["Export cancelled by user".to_string()],
            });
        }

        // Update progress for database generation
        progress = progress.generating_database();
        let _ = app_handle.emit("export-progress", &progress);

        // Generate database (PDB or Device Library Plus based on request)
        if request.use_device_library_plus {
            self.generate_device_library_plus(
                &request.mount_point,
                &playlists_with_tracks,
                &device_tracks,
            )?;
        } else {
            self.generate_rekordbox_pdb(
                &request.mount_point,
                &playlists_with_tracks,
                &device_tracks,
            )?;
        }

        // Record export state in database
        self.record_export_state(&request)?;

        // Mark progress as complete
        progress = progress.completed();
        let _ = app_handle.emit("export-progress", &progress);

        Ok(ExportResult {
            success: true,
            tracks_copied: progress.files_copied,
            tracks_skipped: 0,
            errors: vec![],
        })
    }
}
