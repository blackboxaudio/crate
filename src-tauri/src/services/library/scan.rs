//! Recursive scan of the device's configured music folder.
//!
//! The user picks one folder per device; it is stored as the device-local mapping of a
//! well-known logical library root. Scanning walks that tree, imports every supported
//! audio file, and silently skips files whose content hash already exists (no duplicate
//! payloads are created).

use super::*;
use crate::services::cloud_sync::resolution;

use walkdir::WalkDir;

/// Logical library root that represents this device's music folder.
pub const LOCAL_MUSIC_ROOT_ID: &str = "local-music-library";
/// Name stored on that root. It syncs to other devices; each device maps its own folder.
pub const LOCAL_MUSIC_ROOT_NAME: &str = "Music Library";

impl LibraryService {
    /// The local absolute folder mapped to this device's music library root, if set.
    pub fn music_library_folder(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        resolution::root_mapping(&conn, LOCAL_MUSIC_ROOT_ID)
    }

    /// Persist the device-local music folder for the well-known root.
    ///
    /// Returns the canonical value that was actually stored, so the caller sees exactly
    /// what later scans will read back.
    pub fn set_music_library_folder(&self, path: &str) -> Result<String> {
        if !std::path::Path::new(path).is_dir() {
            return Err(CrateError::InvalidOperation(format!(
                "Not a directory: {path}"
            )));
        }

        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;
        resolution::ensure_root(&conn, LOCAL_MUSIC_ROOT_ID, LOCAL_MUSIC_ROOT_NAME)?;
        resolution::set_root_mapping(&conn, LOCAL_MUSIC_ROOT_ID, path)?;
        resolution::root_mapping(&conn, LOCAL_MUSIC_ROOT_ID)?.ok_or_else(|| {
            CrateError::InvalidOperation("Failed to persist music library folder".to_string())
        })
    }

    /// Recursively import every supported audio file under the configured music folder.
    ///
    /// Files whose content hash already exists in the library are skipped silently — no
    /// duplicate payload is created and no duplicate flow is opened.
    pub fn scan_music_library_folder(&self) -> Result<LibraryFolderScanResult> {
        let root = self.music_library_folder()?.ok_or_else(|| {
            CrateError::InvalidOperation("No music library folder configured".to_string())
        })?;

        if !std::path::Path::new(&root).is_dir() {
            return Err(CrateError::InvalidOperation(format!(
                "Music library folder is unavailable: {root}"
            )));
        }

        let mut result = LibraryFolderScanResult {
            scanned_count: 0,
            imported_count: 0,
            skipped_existing_count: 0,
            failed_count: 0,
            imported_track_ids: Vec::new(),
            errors: Vec::new(),
        };

        // Walk without following symlinks: this avoids symlink cycles and guarantees every
        // produced path stays under the canonical root prefix, so `strip_prefix` matches.
        let mut files: Vec<PathBuf> = Vec::new();
        for entry in WalkDir::new(&root).follow_links(false) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    result.failed_count += 1;
                    result.errors.push(e.to_string());
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let extension = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            if !SUPPORTED_AUDIO_EXTENSIONS.contains(&extension.as_str()) {
                continue;
            }
            files.push(entry.into_path());
        }

        // Sort so the run (and its result) is deterministic and testable.
        files.sort();
        result.scanned_count = files.len();

        // No connection lock is held across the walk or the hashing. Each read/write below
        // acquires it only for the single operation it needs, matching the per-track
        // locking of the existing import path.
        for path in files {
            let hash = match compute_audio_hash(&path) {
                Ok(hash) => hash,
                Err(e) => {
                    result.failed_count += 1;
                    result.errors.push(format!("{}: {e}", path.display()));
                    continue;
                }
            };

            // Absorb a lookup error like any other per-file failure: propagating it would
            // discard the counts and imported ids already accumulated for this run.
            match self.find_track_by_hash(&hash) {
                Ok(Some(_)) => {
                    result.skipped_existing_count += 1;
                    continue;
                }
                Ok(None) => {}
                Err(e) => {
                    result.failed_count += 1;
                    result.errors.push(format!("{}: {e}", path.display()));
                    continue;
                }
            }

            match self.import_single_track_with_hash(&path, hash) {
                Ok(track) => {
                    result.imported_count += 1;
                    result.imported_track_ids.push(track.id);
                }
                Err(e) => {
                    result.failed_count += 1;
                    result.errors.push(format!("{}: {e}", path.display()));
                }
            }
        }

        Ok(result)
    }
}
