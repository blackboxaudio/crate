pub mod anlz;
pub mod rekordbox;

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::{AppHandle, Emitter};

use crate::error::{CrateError, Result};
use crate::models::{
    DeviceExport, DeviceTrack, ExportProgress, ExportRequest, ExportResult, Playlist, Track,
};

use self::rekordbox::RekordboxPdbWriter;

/// Service for exporting playlists to USB devices in Rekordbox-compatible format
pub struct ExportService {
    conn: Arc<Mutex<Connection>>,
    /// Flag to signal export cancellation
    cancel_flag: Arc<AtomicBool>,
}

impl ExportService {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self {
            conn,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

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

        // Update progress for PDB generation
        progress = progress.generating_database();
        let _ = app_handle.emit("export-progress", &progress);

        // Generate Rekordbox PDB
        self.generate_rekordbox_pdb(&request.mount_point, &playlists_with_tracks, &device_tracks)?;

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

    /// Request cancellation of the current export
    pub fn cancel_export(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Validate that the device is mounted and writable
    fn validate_device(&self, mount_point: &str) -> Result<()> {
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
    fn check_available_space(&self, mount_point: &str, tracks: &[Track]) -> Result<()> {
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

    /// Collect all tracks from selected playlists, recursively including folder contents
    fn collect_tracks_for_export(
        &self,
        playlist_ids: &[String],
    ) -> Result<Vec<(Playlist, Vec<Track>)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        let mut result: Vec<(Playlist, Vec<Track>)> = Vec::new();

        for playlist_id in playlist_ids {
            // Get the playlist
            let playlist: Playlist = conn.query_row(
                r#"
                SELECT id, name, parent_id, is_folder, is_smart, smart_rules,
                       sort_order, date_created, date_modified,
                       COALESCE((SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = id), 0)
                FROM playlists WHERE id = ?1
                "#,
                [playlist_id],
                |row| {
                    Ok(Playlist {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        parent_id: row.get(2)?,
                        is_folder: row.get(3)?,
                        is_smart: row.get(4)?,
                        smart_rules: row.get(5)?,
                        sort_order: row.get(6)?,
                        date_created: row.get(7)?,
                        date_modified: row.get(8)?,
                        track_count: row.get(9)?,
                    })
                },
            )?;

            if playlist.is_folder {
                // Recursively get all child playlists
                let child_ids = self.get_all_descendant_playlist_ids(&conn, playlist_id)?;
                for child_id in child_ids {
                    let child_playlist: Playlist = conn.query_row(
                        r#"
                        SELECT id, name, parent_id, is_folder, is_smart, smart_rules,
                               sort_order, date_created, date_modified,
                               COALESCE((SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = id), 0)
                        FROM playlists WHERE id = ?1
                        "#,
                        [&child_id],
                        |row| {
                            Ok(Playlist {
                                id: row.get(0)?,
                                name: row.get(1)?,
                                parent_id: row.get(2)?,
                                is_folder: row.get(3)?,
                                is_smart: row.get(4)?,
                                smart_rules: row.get(5)?,
                                sort_order: row.get(6)?,
                                date_created: row.get(7)?,
                                date_modified: row.get(8)?,
                                track_count: row.get(9)?,
                            })
                        },
                    )?;

                    if !child_playlist.is_folder {
                        let tracks = self.get_playlist_tracks(&conn, &child_id)?;
                        result.push((child_playlist, tracks));
                    }
                }
                // Also add the folder itself (for tree structure in PDB)
                result.push((playlist, vec![]));
            } else {
                // Regular playlist - get its tracks
                let tracks = self.get_playlist_tracks(&conn, playlist_id)?;
                result.push((playlist, tracks));
            }
        }

        Ok(result)
    }

    /// Get all descendant playlist IDs (recursive)
    fn get_all_descendant_playlist_ids(
        &self,
        conn: &Connection,
        parent_id: &str,
    ) -> Result<Vec<String>> {
        get_all_descendant_playlist_ids_impl(conn, parent_id)
    }

    /// Get tracks for a playlist in order
    fn get_playlist_tracks(&self, conn: &Connection, playlist_id: &str) -> Result<Vec<Track>> {
        let mut stmt = conn.prepare(
            r#"
            SELECT t.id, t.file_path, t.file_hash,
                   t.title, t.artist, t.album, t.year, t.genre, t.label, t.catalog_number,
                   t.duration_ms, t.bpm, t.key, t.bitrate, t.sample_rate, t.format,
                   t.analysis_source, t.waveform_data, t.rating, t.play_count,
                   t.date_added, t.date_modified, t.last_played, t.rekordbox_id,
                   t.artwork_path, t.artwork_source, t.color
            FROM tracks t
            JOIN playlist_tracks pt ON t.id = pt.track_id
            WHERE pt.playlist_id = ?1
            ORDER BY pt.position
            "#,
        )?;

        let tracks = stmt
            .query_map([playlist_id], |row| {
                Ok(Track {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    file_hash: row.get(2)?,
                    title: row.get(3)?,
                    artist: row.get(4)?,
                    album: row.get(5)?,
                    year: row.get(6)?,
                    genre: row.get(7)?,
                    label: row.get(8)?,
                    catalog_number: row.get(9)?,
                    duration_ms: row.get(10)?,
                    bpm: row.get(11)?,
                    key: row.get(12)?,
                    bitrate: row.get(13)?,
                    sample_rate: row.get(14)?,
                    format: row.get(15)?,
                    analysis_source: row.get(16)?,
                    waveform_data: row.get(17)?,
                    rating: row.get(18)?,
                    play_count: row.get(19)?,
                    date_added: row.get(20)?,
                    date_modified: row.get(21)?,
                    last_played: row.get(22)?,
                    rekordbox_id: row.get(23)?,
                    artwork_path: row.get(24)?,
                    artwork_source: row.get(25)?,
                    color: row.get(26)?,
                    tags: vec![],
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tracks)
    }

    /// Create the PIONEER folder structure on the USB device
    fn create_pioneer_folders(&self, mount_point: &str) -> Result<()> {
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
    fn copy_tracks(
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

            // Update progress
            let filename = Path::new(&track.file_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            *progress = progress.clone().copying(filename);
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
            };
            device_tracks.push(device_track);
        }

        // Save device tracks to database
        self.save_device_tracks(&device_tracks)?;

        Ok(device_tracks)
    }

    /// Generate the Rekordbox PDB file
    fn generate_rekordbox_pdb(
        &self,
        mount_point: &str,
        playlists_with_tracks: &[(Playlist, Vec<Track>)],
        device_tracks: &[DeviceTrack],
    ) -> Result<()> {
        let pdb_path = Path::new(mount_point)
            .join("PIONEER")
            .join("rekordbox")
            .join("export.pdb");

        // Check if existing PDB exists for merging
        let existing_pdb = if pdb_path.exists() {
            Some(
                fs::read(&pdb_path)
                    .map_err(|e| CrateError::Device(format!("Failed to read existing PDB: {e}")))?,
            )
        } else {
            None
        };

        // Create PDB writer
        let mut writer = if let Some(ref data) = existing_pdb {
            RekordboxPdbWriter::from_existing(data)?
        } else {
            RekordboxPdbWriter::new()
        };

        // Build track ID to USB path mapping
        let track_paths: HashMap<String, String> = device_tracks
            .iter()
            .map(|dt| (dt.track_id.clone(), dt.usb_path.clone()))
            .collect();

        // Track PDB ID counter for ANLZ path generation
        let mut next_pdb_id: u32 = 1;

        // Add tracks to PDB
        let mut track_pdb_ids: HashMap<String, u32> = HashMap::new();
        for (_, tracks) in playlists_with_tracks {
            for track in tracks {
                if let Some(usb_path) = track_paths.get(&track.id) {
                    if !track_pdb_ids.contains_key(&track.id) {
                        // Generate ANLZ file for this track
                        let anlz_path = self.generate_anlz_file(
                            mount_point,
                            next_pdb_id,
                            usb_path,
                            track.duration_ms as u32,
                            track.bpm.map(|b| b as f32),
                        )?;

                        let pdb_id = writer.add_track(track, usb_path, &anlz_path);
                        track_pdb_ids.insert(track.id.clone(), pdb_id);
                        next_pdb_id += 1;
                    }
                }
            }
        }

        // Add playlists to PDB
        for (playlist, tracks) in playlists_with_tracks {
            let track_ids: Vec<u32> = tracks
                .iter()
                .filter_map(|t| track_pdb_ids.get(&t.id).copied())
                .collect();
            writer.add_playlist(playlist, &track_ids);
        }

        // Write PDB file
        writer.write(&pdb_path)?;

        // Update device_tracks with PDB IDs
        self.update_device_track_pdb_ids(&track_pdb_ids)?;

        Ok(())
    }

    /// Generate an ANLZ file for a track and return the device path
    fn generate_anlz_file(
        &self,
        mount_point: &str,
        pdb_track_id: u32,
        usb_audio_path: &str,
        duration_ms: u32,
        bpm: Option<f32>,
    ) -> Result<String> {
        use crate::services::export::anlz;

        // Generate the ANLZ directory and file paths
        let anlz_dir = anlz::generate_anlz_dir(pdb_track_id);
        let anlz_path = anlz::generate_anlz_path(pdb_track_id);

        // Create the directory on the USB
        let full_dir = Path::new(mount_point).join(&anlz_dir[1..]); // Remove leading /
        fs::create_dir_all(&full_dir)
            .map_err(|e| CrateError::Device(format!("Failed to create ANLZ directory: {e}")))?;

        // Write the ANLZ file with beat grid based on BPM
        let full_path = Path::new(mount_point).join(&anlz_path[1..]); // Remove leading /
        let device_audio_path = format!("/Contents/{usb_audio_path}");
        anlz::write_anlz_file(&full_path, &device_audio_path, duration_ms, bpm)?;

        Ok(anlz_path)
    }

    /// Record export state in the database
    fn record_export_state(&self, request: &ExportRequest) -> Result<()> {
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
    fn save_device_tracks(&self, device_tracks: &[DeviceTrack]) -> Result<()> {
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
    fn update_device_track_pdb_ids(&self, track_pdb_ids: &HashMap<String, u32>) -> Result<()> {
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
            SELECT device_id, track_id, usb_path, file_hash, pdb_track_id, exported_at
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
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tracks)
    }

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

/// Get all descendant playlist IDs (recursive implementation)
fn get_all_descendant_playlist_ids_impl(conn: &Connection, parent_id: &str) -> Result<Vec<String>> {
    let mut result = Vec::new();

    let mut stmt = conn
        .prepare("SELECT id, is_folder FROM playlists WHERE parent_id = ?1 ORDER BY sort_order")?;

    let children: Vec<(String, bool)> = stmt
        .query_map([parent_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for (child_id, is_folder) in children {
        result.push(child_id.clone());
        if is_folder {
            result.extend(get_all_descendant_playlist_ids_impl(conn, &child_id)?);
        }
    }

    Ok(result)
}

/// Sanitize a path component by replacing invalid characters
fn sanitize_path_component(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();

    if sanitized.is_empty() || sanitized.trim().is_empty() {
        "Unknown".to_string()
    } else {
        sanitized.trim().to_string()
    }
}

/// Build the USB path for a track: Contents/{Artist}/{Album}/{filename}
fn build_usb_path(track: &Track) -> String {
    let artist = sanitize_path_component(track.artist.as_deref().unwrap_or("Unknown Artist"));
    let album = sanitize_path_component(track.album.as_deref().unwrap_or("Unknown Album"));
    let filename = Path::new(&track.file_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    format!("{artist}/{album}/{filename}")
}

/// Recursively remove empty directories
fn cleanup_empty_dirs(path: &Path) {
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                cleanup_empty_dirs(&entry_path);
                // Try to remove if empty
                let _ = fs::remove_dir(&entry_path);
            }
        }
    }
}
