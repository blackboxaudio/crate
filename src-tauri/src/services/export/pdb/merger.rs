//! PDB merger for combining existing PDB data with new exports
//!
//! This module provides functionality to merge new export data with
//! existing PDB files, preserving IDs where possible for stable
//! track references on the device.

#![allow(dead_code)]

use std::collections::HashMap;

use super::reader::{ParsedPdb, ParsedTrack};

/// Merger for combining existing PDB data with new exports
///
/// This struct maintains lookup tables from an existing PDB file
/// and provides ID allocation that preserves existing IDs.
#[derive(Debug)]
pub struct PdbMerger {
    /// Existing tracks indexed by normalized file path
    existing_tracks: HashMap<String, ParsedTrack>,
    /// Existing artists indexed by lowercase name
    existing_artists: HashMap<String, u32>,
    /// Existing albums indexed by (lowercase name, artist_id)
    existing_albums: HashMap<(String, u32), u32>,
    /// Existing genres indexed by lowercase name
    existing_genres: HashMap<String, u32>,
    /// Existing keys indexed by lowercase name
    existing_keys: HashMap<String, u32>,
    /// Next track ID to allocate
    next_track_id: u32,
    /// Next artist ID to allocate
    next_artist_id: u32,
    /// Next album ID to allocate
    next_album_id: u32,
    /// Next genre ID to allocate
    next_genre_id: u32,
    /// Next key ID to allocate
    next_key_id: u32,
    /// Next playlist ID to allocate
    next_playlist_id: u32,
}

impl PdbMerger {
    /// Create a new merger from parsed PDB data
    pub fn from_parsed(parsed: ParsedPdb) -> Self {
        // Build track lookup by normalized path
        let mut existing_tracks = HashMap::new();
        for track in parsed.tracks {
            let normalized = normalize_path(&track.file_path);
            existing_tracks.insert(normalized, track);
        }

        // Build artist lookup by lowercase name
        let mut existing_artists = HashMap::new();
        for (id, name) in &parsed.artists {
            existing_artists.insert(name.to_lowercase(), *id);
        }

        // Build album lookup by (lowercase name, artist_id)
        let mut existing_albums = HashMap::new();
        for (id, (name, artist_id)) in &parsed.albums {
            existing_albums.insert((name.to_lowercase(), *artist_id), *id);
        }

        // Build genre lookup by lowercase name
        let mut existing_genres = HashMap::new();
        for (id, name) in &parsed.genres {
            existing_genres.insert(name.to_lowercase(), *id);
        }

        // Build key lookup by lowercase name
        let mut existing_keys = HashMap::new();
        for (id, name) in &parsed.keys {
            existing_keys.insert(name.to_lowercase(), *id);
        }

        Self {
            existing_tracks,
            existing_artists,
            existing_albums,
            existing_genres,
            existing_keys,
            next_track_id: parsed.next_track_id,
            next_artist_id: parsed.next_artist_id,
            next_album_id: parsed.next_album_id,
            next_genre_id: parsed.next_genre_id,
            next_key_id: parsed.next_key_id,
            next_playlist_id: parsed.next_playlist_id,
        }
    }

    /// Create an empty merger (for fresh exports)
    pub fn empty() -> Self {
        Self {
            existing_tracks: HashMap::new(),
            existing_artists: HashMap::new(),
            existing_albums: HashMap::new(),
            existing_genres: HashMap::new(),
            existing_keys: HashMap::new(),
            next_track_id: 1,
            next_artist_id: 1,
            next_album_id: 1,
            next_genre_id: 1,
            next_key_id: 1,
            next_playlist_id: 1,
        }
    }

    /// Get an existing track by its USB file path
    pub fn get_existing_track(&self, usb_path: &str) -> Option<&ParsedTrack> {
        let normalized = normalize_path(usb_path);
        self.existing_tracks.get(&normalized)
    }

    /// Get an existing track ID or allocate a new one
    ///
    /// If a track with the same USB path exists, returns its ID.
    /// Otherwise, allocates and returns a new ID.
    pub fn get_or_allocate_track_id(&mut self, usb_path: &str) -> u32 {
        let normalized = normalize_path(usb_path);

        if let Some(track) = self.existing_tracks.get(&normalized) {
            track.id
        } else {
            let id = self.next_track_id;
            self.next_track_id += 1;
            id
        }
    }

    /// Get an existing artist ID or allocate a new one
    pub fn get_or_create_artist(&mut self, name: &str) -> u32 {
        let key = name.to_lowercase();

        if let Some(&id) = self.existing_artists.get(&key) {
            id
        } else {
            let id = self.next_artist_id;
            self.next_artist_id += 1;
            self.existing_artists.insert(key, id);
            id
        }
    }

    /// Get an existing album ID or allocate a new one
    pub fn get_or_create_album(&mut self, name: &str, artist_id: u32) -> u32 {
        let key = (name.to_lowercase(), artist_id);

        if let Some(&id) = self.existing_albums.get(&key) {
            id
        } else {
            let id = self.next_album_id;
            self.next_album_id += 1;
            self.existing_albums.insert(key, id);
            id
        }
    }

    /// Get an existing genre ID or allocate a new one
    pub fn get_or_create_genre(&mut self, name: &str) -> u32 {
        let key = name.to_lowercase();

        if let Some(&id) = self.existing_genres.get(&key) {
            id
        } else {
            let id = self.next_genre_id;
            self.next_genre_id += 1;
            self.existing_genres.insert(key, id);
            id
        }
    }

    /// Get an existing key ID or allocate a new one
    pub fn get_or_create_key(&mut self, name: &str) -> u32 {
        let key = name.to_lowercase();

        if let Some(&id) = self.existing_keys.get(&key) {
            id
        } else {
            let id = self.next_key_id;
            self.next_key_id += 1;
            self.existing_keys.insert(key, id);
            id
        }
    }

    /// Allocate a new playlist ID
    pub fn allocate_playlist_id(&mut self) -> u32 {
        let id = self.next_playlist_id;
        self.next_playlist_id += 1;
        id
    }

    /// Get the count of existing tracks
    pub fn existing_track_count(&self) -> usize {
        self.existing_tracks.len()
    }

    /// Get all existing track paths
    pub fn existing_track_paths(&self) -> impl Iterator<Item = &String> {
        self.existing_tracks.keys()
    }

    /// Check if a track exists at the given path
    pub fn has_track(&self, usb_path: &str) -> bool {
        let normalized = normalize_path(usb_path);
        self.existing_tracks.contains_key(&normalized)
    }
}

/// Normalize a file path for comparison
///
/// - Converts to lowercase
/// - Normalizes path separators
fn normalize_path(path: &str) -> String {
    path.to_lowercase().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_merger() {
        let mut merger = PdbMerger::empty();

        // First allocations should start at 1
        assert_eq!(merger.get_or_allocate_track_id("/Contents/test.mp3"), 1);
        assert_eq!(merger.get_or_create_artist("Test Artist"), 1);
        assert_eq!(merger.get_or_create_album("Test Album", 1), 1);
        assert_eq!(merger.get_or_create_genre("Test Genre"), 1);
        assert_eq!(merger.get_or_create_key("1A"), 1);
    }

    #[test]
    fn test_artist_deduplication() {
        let mut merger = PdbMerger::empty();

        let id1 = merger.get_or_create_artist("Test Artist");
        let id2 = merger.get_or_create_artist("TEST ARTIST"); // Different case
        let id3 = merger.get_or_create_artist("test artist"); // Different case

        // All should return the same ID
        assert_eq!(id1, id2);
        assert_eq!(id2, id3);

        // A different artist should get a new ID
        let id4 = merger.get_or_create_artist("Different Artist");
        assert_ne!(id1, id4);
    }

    #[test]
    fn test_path_normalization() {
        assert_eq!(
            normalize_path("/Contents/Artist/Track.mp3"),
            "/contents/artist/track.mp3"
        );
        assert_eq!(
            normalize_path("\\Contents\\Artist\\Track.mp3"),
            "/contents/artist/track.mp3"
        );
    }
}
