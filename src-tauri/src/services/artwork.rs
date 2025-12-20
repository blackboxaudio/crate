use std::fs;
use std::path::PathBuf;

use image::imageops::FilterType;
use image::ImageFormat;
use lofty::file::TaggedFile;
use lofty::picture::PictureType;
use lofty::prelude::*;
use lofty::tag::{Tag, TagType};

/// Service for managing album artwork extraction and storage.
/// Artwork is stored as 500x500 WEBP images in the app data directory.
pub struct ArtworkService {
    artwork_dir: PathBuf,
}

impl ArtworkService {
    /// Creates a new ArtworkService with the given app data directory.
    /// The artwork subdirectory will be created if it doesn't exist.
    pub fn new(app_data_dir: PathBuf) -> Self {
        let artwork_dir = app_data_dir.join("artwork");
        // Ensure artwork directory exists
        if let Err(e) = fs::create_dir_all(&artwork_dir) {
            log::warn!("Failed to create artwork directory: {e}");
        }
        Self { artwork_dir }
    }

    /// Extracts album art from a tagged audio file and saves it as WEBP.
    /// Returns the relative path (e.g., "artwork/{track_id}.webp") if successful.
    /// Uses multiple strategies to find pictures across different tag systems.
    pub fn extract_and_save(&self, tagged_file: &TaggedFile, track_id: &str) -> Option<String> {
        // First try the primary tag (most reliable for common formats)
        if let Some(tag) = tagged_file.primary_tag() {
            if let Some(path) = self.extract_from_tag(tag, track_id) {
                return Some(path);
            }
        }

        // Then try all available tags through the generic interface
        for tag in tagged_file.tags() {
            if let Some(path) = self.extract_from_tag(tag, track_id) {
                return Some(path);
            }
        }

        // For AIFF and MP3 files, explicitly check ID3v2 tag
        // (pictures are stored in ID3v2, not in native AIFF chunks)
        if let Some(tag) = tagged_file.tag(TagType::Id3v2) {
            if let Some(path) = self.extract_from_tag(tag, track_id) {
                return Some(path);
            }
        }

        // Also try APE tags (sometimes used in MP3 files)
        if let Some(tag) = tagged_file.tag(TagType::Ape) {
            if let Some(path) = self.extract_from_tag(tag, track_id) {
                return Some(path);
            }
        }

        log::debug!("No artwork found for track {track_id}");
        None
    }

    /// Extracts a picture from a single tag and saves it.
    fn extract_from_tag(&self, tag: &Tag, track_id: &str) -> Option<String> {
        let pictures = tag.pictures();
        if pictures.is_empty() {
            return None;
        }

        // Prefer front cover, fall back to first available
        let picture = pictures
            .iter()
            .find(|p| p.pic_type() == PictureType::CoverFront)
            .or_else(|| pictures.first())?;

        self.save_picture(picture.data(), track_id)
    }

    /// Saves picture data to disk as a WEBP image.
    /// Resizes to 500x500 max while maintaining aspect ratio.
    fn save_picture(&self, data: &[u8], track_id: &str) -> Option<String> {
        // Load the image from raw bytes
        let img = image::load_from_memory(data).ok()?;

        // Resize if larger than 500x500, maintaining aspect ratio
        let img = if img.width() > 500 || img.height() > 500 {
            img.resize(500, 500, FilterType::Lanczos3)
        } else {
            img
        };

        // Build the output path
        let filename = format!("{track_id}.webp");
        let path = self.artwork_dir.join(&filename);

        // Save as WEBP
        if let Err(e) = img.save_with_format(&path, ImageFormat::WebP) {
            log::warn!("Failed to save artwork for track {track_id}: {e}");
            return None;
        }

        // Return the relative path for database storage
        Some(format!("artwork/{filename}"))
    }

    /// Deletes the artwork file for a track.
    pub fn delete(&self, track_id: &str) {
        let path = self.artwork_dir.join(format!("{track_id}.webp"));
        if let Err(e) = fs::remove_file(&path) {
            // Only log if the file existed (ignore NotFound errors)
            if e.kind() != std::io::ErrorKind::NotFound {
                log::warn!("Failed to delete artwork for track {track_id}: {e}");
            }
        }
    }

    /// Saves artwork from a user-provided image file.
    /// Returns the relative path (e.g., "artwork/{track_id}.webp") if successful.
    pub fn save_from_file(&self, source_path: &std::path::Path, track_id: &str) -> Option<String> {
        // Load the image from the source file
        let img = image::open(source_path).ok()?;

        // Resize if larger than 500x500, maintaining aspect ratio
        let img = if img.width() > 500 || img.height() > 500 {
            img.resize(500, 500, FilterType::Lanczos3)
        } else {
            img
        };

        // Build the output path
        let filename = format!("{track_id}.webp");
        let path = self.artwork_dir.join(&filename);

        // Save as WEBP
        if let Err(e) = img.save_with_format(&path, ImageFormat::WebP) {
            log::warn!("Failed to save user-provided artwork for track {track_id}: {e}");
            return None;
        }

        // Return the relative path for database storage
        Some(format!("artwork/{filename}"))
    }

    #[allow(dead_code)]
    /// Returns the full filesystem path for an artwork file.
    pub fn get_full_path(&self, relative_path: &str) -> PathBuf {
        self.artwork_dir
            .parent()
            .unwrap_or(&self.artwork_dir)
            .join(relative_path)
    }
}
