//! Sync diff calculator for determining track changes
//!
//! This module calculates the difference between the library state
//! and the device state to determine which tracks need to be added,
//! updated, or removed during sync operations.

#![allow(dead_code)]

use std::collections::HashSet;

use crate::models::{DeviceTrack, Track};

/// Strategy for handling tracks that are no longer in the export
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrackRemovalStrategy {
    /// Remove tracks that are no longer in any synced playlist
    RemoveOrphaned,
    /// Only remove tracks that were deleted from the library (default)
    #[default]
    Conservative,
    /// Never remove tracks from the device
    NeverRemove,
}

/// Type of change for a track
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackChangeType {
    /// Track needs to be added to the device
    Add,
    /// Track exists but needs to be updated (file or metadata changed)
    Update,
    /// Track should be removed from the device
    Remove,
    /// Track is unchanged
    Unchanged,
}

/// A track with its calculated change type
#[derive(Debug, Clone)]
pub struct TrackChange {
    pub track: Track,
    pub usb_path: String,
    pub change_type: TrackChangeType,
}

/// Summary of changes between library and device
#[derive(Debug, Clone)]
pub struct SyncDiff {
    /// Tracks that need to be added to the device
    pub tracks_to_add: Vec<(Track, String)>,
    /// Tracks that need to be updated on the device
    pub tracks_to_update: Vec<(Track, String)>,
    /// USB paths of tracks to remove from the device
    pub tracks_to_remove: Vec<String>,
    /// Total bytes that will be copied
    pub bytes_to_copy: u64,
    /// Total bytes that will be freed by removal
    pub bytes_to_remove: u64,
}

impl SyncDiff {
    /// Calculate the sync diff between library tracks and device tracks
    pub fn calculate(
        library_tracks: &[Track],
        device_tracks: &[DeviceTrack],
        removal_strategy: TrackRemovalStrategy,
    ) -> Self {
        let mut tracks_to_add = Vec::new();
        let mut tracks_to_update = Vec::new();
        let mut tracks_to_remove = Vec::new();
        let mut bytes_to_copy: u64 = 0;
        let mut bytes_to_remove: u64 = 0;

        // Build a set of device tracks by track_id for quick lookup
        let device_track_map: std::collections::HashMap<&str, &DeviceTrack> = device_tracks
            .iter()
            .map(|dt| (dt.track_id.as_str(), dt))
            .collect();

        // Build a set of library track IDs
        let library_track_ids: HashSet<&str> =
            library_tracks.iter().map(|t| t.id.as_str()).collect();

        // Check each library track
        for track in library_tracks {
            let usb_path = generate_usb_path(track);

            if let Some(device_track) = device_track_map.get(track.id.as_str()) {
                // Track exists on device - check if it needs updating
                if track_needs_update(track, device_track) {
                    let file_size = std::fs::metadata(&track.file_path)
                        .map(|m| m.len())
                        .unwrap_or(0);
                    bytes_to_copy += file_size;
                    tracks_to_update.push((track.clone(), usb_path));
                }
                // Otherwise, track is unchanged - no action needed
            } else {
                // Track doesn't exist on device - needs to be added
                let file_size = std::fs::metadata(&track.file_path)
                    .map(|m| m.len())
                    .unwrap_or(0);
                bytes_to_copy += file_size;
                tracks_to_add.push((track.clone(), usb_path));
            }
        }

        // Check for tracks to remove based on strategy
        match removal_strategy {
            TrackRemovalStrategy::RemoveOrphaned | TrackRemovalStrategy::Conservative => {
                for device_track in device_tracks {
                    if !library_track_ids.contains(device_track.track_id.as_str()) {
                        // Track is on device but not in library
                        if removal_strategy == TrackRemovalStrategy::RemoveOrphaned
                            || (removal_strategy == TrackRemovalStrategy::Conservative
                                && track_was_deleted(&device_track.track_id))
                        {
                            // Estimate file size (we don't have it stored, so estimate from typical track size)
                            bytes_to_remove += 10 * 1024 * 1024; // Estimate 10MB per track
                            tracks_to_remove.push(device_track.usb_path.clone());
                        }
                    }
                }
            }
            TrackRemovalStrategy::NeverRemove => {
                // Don't remove any tracks
            }
        }

        SyncDiff {
            tracks_to_add,
            tracks_to_update,
            tracks_to_remove,
            bytes_to_copy,
            bytes_to_remove,
        }
    }

    /// Get the total number of tracks that will be copied
    pub fn tracks_to_copy_count(&self) -> usize {
        self.tracks_to_add.len() + self.tracks_to_update.len()
    }

    /// Check if there are any changes
    pub fn has_changes(&self) -> bool {
        !self.tracks_to_add.is_empty()
            || !self.tracks_to_update.is_empty()
            || !self.tracks_to_remove.is_empty()
    }

    /// Get all tracks that need to be copied (add + update)
    pub fn all_tracks_to_copy(&self) -> Vec<&(Track, String)> {
        self.tracks_to_add
            .iter()
            .chain(self.tracks_to_update.iter())
            .collect()
    }
}

/// Generate the USB path for a track
fn generate_usb_path(track: &Track) -> String {
    // Use artist/album/filename structure
    let artist = track
        .artist
        .as_deref()
        .unwrap_or("Unknown Artist")
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");

    let album = track
        .album
        .as_deref()
        .unwrap_or("Unknown Album")
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");

    let filename = std::path::Path::new(&track.file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.mp3")
        .to_string();

    format!("{artist}/{album}/{filename}")
}

/// Check if a track needs to be updated on the device
fn track_needs_update(track: &Track, device_track: &DeviceTrack) -> bool {
    // Check file hash
    if let Some(ref current_hash) = track.file_hash {
        if current_hash != &device_track.file_hash {
            return true;
        }
    }

    // Could also check metadata hash if available
    // For now, just check file hash

    false
}

/// Check if a track was deleted from the library
fn track_was_deleted(_track_id: &str) -> bool {
    // In Conservative mode, we would check if the track still exists in the library
    // but was just removed from the export playlists.
    // For now, we assume the caller has already filtered library_tracks to only
    // include tracks that should be on the device, so any missing track is "deleted"
    // from the perspective of the export.
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_track(id: &str, title: &str, artist: &str) -> Track {
        Track {
            id: id.to_string(),
            file_path: format!("/path/to/{}.mp3", title.to_lowercase().replace(' ', "_")),
            file_hash: Some("hash123".to_string()),
            title: Some(title.to_string()),
            artist: Some(artist.to_string()),
            album: Some("Test Album".to_string()),
            year: None,
            genre: None,
            label: None,
            catalog_number: None,
            duration_ms: 180000,
            bpm: Some(120.0),
            key: None,
            bitrate: Some(320),
            sample_rate: Some(44100),
            format: Some("mp3".to_string()),
            analysis_source: None,
            waveform_data: None,
            rating: 0,
            play_count: 0,
            date_added: "2024-01-01T00:00:00Z".to_string(),
            date_modified: "2024-01-01T00:00:00Z".to_string(),
            last_played: None,
            rekordbox_id: None,
            artwork_path: None,
            artwork_source: None,
            color: None,
        }
    }

    fn make_device_track(id: &str, usb_path: &str, hash: &str) -> DeviceTrack {
        DeviceTrack {
            device_id: "device1".to_string(),
            track_id: id.to_string(),
            usb_path: usb_path.to_string(),
            file_hash: hash.to_string(),
            pdb_track_id: Some(1),
            exported_at: "2024-01-01T00:00:00Z".to_string(),
            metadata_hash: None,
        }
    }

    #[test]
    fn test_empty_sync() {
        let diff = SyncDiff::calculate(&[], &[], TrackRemovalStrategy::Conservative);

        assert!(diff.tracks_to_add.is_empty());
        assert!(diff.tracks_to_update.is_empty());
        assert!(diff.tracks_to_remove.is_empty());
        assert!(!diff.has_changes());
    }

    #[test]
    fn test_add_new_track() {
        let tracks = vec![make_track("1", "New Track", "Artist")];
        let device_tracks = vec![];

        let diff = SyncDiff::calculate(&tracks, &device_tracks, TrackRemovalStrategy::Conservative);

        assert_eq!(diff.tracks_to_add.len(), 1);
        assert_eq!(diff.tracks_to_add[0].0.id, "1");
        assert!(diff.tracks_to_update.is_empty());
        assert!(diff.tracks_to_remove.is_empty());
    }

    #[test]
    fn test_unchanged_track() {
        let tracks = vec![make_track("1", "Existing Track", "Artist")];
        let device_tracks =
            vec![make_device_track("1", "Artist/Test Album/existing_track.mp3", "hash123")];

        let diff = SyncDiff::calculate(&tracks, &device_tracks, TrackRemovalStrategy::Conservative);

        assert!(diff.tracks_to_add.is_empty());
        assert!(diff.tracks_to_update.is_empty());
        assert!(diff.tracks_to_remove.is_empty());
        assert!(!diff.has_changes());
    }

    #[test]
    fn test_remove_orphaned() {
        let tracks = vec![];
        let device_tracks =
            vec![make_device_track("1", "Artist/Album/track.mp3", "hash123")];

        let diff =
            SyncDiff::calculate(&tracks, &device_tracks, TrackRemovalStrategy::RemoveOrphaned);

        assert!(diff.tracks_to_add.is_empty());
        assert!(diff.tracks_to_update.is_empty());
        assert_eq!(diff.tracks_to_remove.len(), 1);
    }

    #[test]
    fn test_never_remove() {
        let tracks = vec![];
        let device_tracks =
            vec![make_device_track("1", "Artist/Album/track.mp3", "hash123")];

        let diff = SyncDiff::calculate(&tracks, &device_tracks, TrackRemovalStrategy::NeverRemove);

        assert!(diff.tracks_to_add.is_empty());
        assert!(diff.tracks_to_update.is_empty());
        assert!(diff.tracks_to_remove.is_empty());
    }
}
