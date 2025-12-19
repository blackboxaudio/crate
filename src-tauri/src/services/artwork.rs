use std::fs;
use std::path::PathBuf;

use image::imageops::FilterType;
use image::ImageFormat;
use lofty::file::TaggedFile;
use lofty::picture::PictureType;
use lofty::prelude::*;

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
            log::warn!("Failed to create artwork directory: {}", e);
        }
        Self { artwork_dir }
    }

    /// Extracts album art from a tagged audio file and saves it as WEBP.
    /// Returns the relative path (e.g., "artwork/{track_id}.webp") if successful.
    /// Iterates through ALL tags to find pictures (important for AIFF which has dual tag systems).
    pub fn extract_and_save(&self, tagged_file: &TaggedFile, track_id: &str) -> Option<String> {
        // Search through ALL tags to find pictures
        // This is important for formats like AIFF that can have metadata in multiple places
        // (native AIFF chunks + ID3v2 tags), where pictures are only in ID3v2
        for tag in tagged_file.tags() {
            // Find the front cover picture, or fall back to first available
            let picture = tag
                .pictures()
                .iter()
                .find(|p| p.pic_type() == PictureType::CoverFront)
                .or_else(|| tag.pictures().first());

            if let Some(pic) = picture {
                if let Some(path) = self.save_picture(pic.data(), track_id) {
                    return Some(path);
                }
            }
        }
        None
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
        let filename = format!("{}.webp", track_id);
        let path = self.artwork_dir.join(&filename);

        // Save as WEBP
        if let Err(e) = img.save_with_format(&path, ImageFormat::WebP) {
            log::warn!("Failed to save artwork for track {}: {}", track_id, e);
            return None;
        }

        // Return the relative path for database storage
        Some(format!("artwork/{}", filename))
    }

    /// Deletes the artwork file for a track.
    pub fn delete(&self, track_id: &str) {
        let path = self.artwork_dir.join(format!("{}.webp", track_id));
        if let Err(e) = fs::remove_file(&path) {
            // Only log if the file existed (ignore NotFound errors)
            if e.kind() != std::io::ErrorKind::NotFound {
                log::warn!("Failed to delete artwork for track {}: {}", track_id, e);
            }
        }
    }

    /// Returns the full filesystem path for an artwork file.
    pub fn get_full_path(&self, relative_path: &str) -> PathBuf {
        self.artwork_dir
            .parent()
            .unwrap_or(&self.artwork_dir)
            .join(relative_path)
    }
}
