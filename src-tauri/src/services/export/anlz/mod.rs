//! ANLZ file writer for Pioneer CDJ/XDJ equipment
//!
//! Generates .DAT, .EXT, and .2EX analysis files required for tracks to appear on Pioneer gear.
//! Based on the Deep Symmetry analysis: https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html
//!
//! ANLZ files use **big-endian** byte order (opposite of PDB files).

#![allow(dead_code)]

mod crypto;
mod error;
mod header;
pub mod tags;
mod utils;

use std::fs::File;
use std::io::{BufWriter, Cursor, Write};
use std::path::Path;

use binrw::BinWrite;

pub use error::{AnlzError, Result};
pub use header::AnlzFileHeader;
pub use utils::{generate_anlz_dir, generate_anlz_path, AnlzVariant};

use crate::models::Cue;
use tags::*;

/// An ANLZ file containing header and tags
#[derive(Debug, Clone)]
pub struct AnlzFile {
    /// File header (PMAI)
    pub header: AnlzFileHeader,
    /// All tags in the file
    pub tags: Vec<AnlzTag>,
}

impl AnlzFile {
    /// Parse an ANLZ file from bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        use binrw::BinReaderExt;

        let mut cursor = Cursor::new(data);

        // Read header
        let header: AnlzFileHeader = cursor.read_be()?;

        // Read tags until end of file
        let mut tags = Vec::new();
        while cursor.position() < data.len() as u64 {
            match tags::read_tag(&mut cursor) {
                Ok(tag) => tags.push(tag),
                Err(_) => break, // Stop on unknown/invalid tag
            }
        }

        Ok(Self { header, tags })
    }

    /// Parse an ANLZ file from a file path
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let data = std::fs::read(path)?;
        Self::parse(&data)
    }

    /// Convert this file to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());

        // Write header
        self.header.write(&mut buf)?;

        // Write all tags
        for tag in &self.tags {
            tag.write_to(&mut buf)?;
        }

        Ok(buf.into_inner())
    }

    /// Write this file to a path
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        // Write header
        self.header.write(&mut writer)?;

        // Write all tags
        for tag in &self.tags {
            tag.write_to(&mut writer)?;
        }

        writer.flush()?;
        Ok(())
    }
}

/// Builder for constructing ANLZ files
pub struct AnlzFileBuilder {
    variant: AnlzVariant,
    path: Option<String>,
    bpm: Option<f32>,
    duration_ms: Option<u32>,
    cues: Vec<Cue>,
}

impl AnlzFileBuilder {
    /// Create a new builder for the specified variant
    pub fn new(variant: AnlzVariant) -> Self {
        Self {
            variant,
            path: None,
            bpm: None,
            duration_ms: None,
            cues: Vec::new(),
        }
    }

    /// Set the device path for the audio file
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Set the BPM (beats per minute)
    pub fn bpm(mut self, bpm: f32) -> Self {
        self.bpm = Some(bpm);
        self
    }

    /// Set the track duration in milliseconds
    pub fn duration_ms(mut self, duration: u32) -> Self {
        self.duration_ms = Some(duration);
        self
    }

    /// Set the cue points for the track
    pub fn cues(mut self, cues: Vec<Cue>) -> Self {
        self.cues = cues;
        self
    }

    /// Build the ANLZ file
    pub fn build(self) -> Result<AnlzFile> {
        let path = self
            .path
            .ok_or_else(|| AnlzError::MissingField("path".to_string()))?;
        let duration_ms = self.duration_ms.unwrap_or(0);
        let bpm = self.bpm.unwrap_or(120.0);

        let mut tags: Vec<AnlzTag> = Vec::new();

        // Common tags for all variants
        // PPTH - Path
        tags.push(AnlzTag::Path(PathTag::new(&path)));

        // PVBR - VBR seek index
        tags.push(AnlzTag::Vbr(VbrTag::new()));

        // PQTZ - Beat grid (for .DAT) or placeholder for others
        if self.bpm.is_some() {
            tags.push(AnlzTag::BeatGrid(BeatGridTag::new(bpm, duration_ms)));
        } else {
            tags.push(AnlzTag::BeatGrid(BeatGridTag::empty()));
        }

        // PWAV - Waveform preview
        tags.push(AnlzTag::WaveformPreview(WaveformPreviewTag::new()));

        // PWV2 - Tiny waveform
        tags.push(AnlzTag::TinyWaveform(TinyWaveformTag::new()));

        // PCOB - Memory/loop cue list
        tags.push(AnlzTag::CueList(CueListTag::memory_cues(&self.cues)));

        // PCOB - Hot cue list
        tags.push(AnlzTag::CueList(CueListTag::hot_cues(&self.cues)));

        // Add .EXT-specific tags
        if matches!(self.variant, AnlzVariant::Ext | AnlzVariant::Ext2) {
            // PQT2 - Extended beat grid
            if self.bpm.is_some() {
                tags.push(AnlzTag::ExtBeatGrid(ExtBeatGridTag::new(bpm, duration_ms)));
            } else {
                tags.push(AnlzTag::ExtBeatGrid(ExtBeatGridTag::empty()));
            }

            // PCO2 - Extended memory/loop cue list
            tags.push(AnlzTag::ExtCueList(ExtCueListTag::memory_cues(&self.cues)));

            // PCO2 - Extended hot cue list
            tags.push(AnlzTag::ExtCueList(ExtCueListTag::hot_cues(&self.cues)));

            // PWV3 - Waveform detail
            tags.push(AnlzTag::WaveformDetail(WaveformDetailTag::new(duration_ms)));

            // PWV4 - Waveform color preview
            tags.push(AnlzTag::WaveformColorPreview(WaveformColorPreviewTag::new()));

            // PWV5 - Waveform color detail
            tags.push(AnlzTag::WaveformColorDetail(WaveformColorDetailTag::new(
                duration_ms,
            )));

            // PSSI - Song structure (empty placeholder)
            tags.push(AnlzTag::SongStructure(SongStructureTag::empty()));
        }

        // Add .2EX-specific tags
        if matches!(self.variant, AnlzVariant::Ext2) {
            // PWV6 - Extended waveform 1
            tags.push(AnlzTag::ExtWaveform1(ExtWaveform1Tag::new(duration_ms)));

            // PWV7 - Extended waveform 2
            tags.push(AnlzTag::ExtWaveform2(ExtWaveform2Tag::new(duration_ms)));

            // PWVC - Extended waveform color
            tags.push(AnlzTag::ExtWaveformColor(ExtWaveformColorTag::empty()));
        }

        // Calculate total content size
        let content_size: u32 = tags.iter().map(|t| t.size()).sum();

        // Create header
        let header = AnlzFileHeader::new(content_size);

        Ok(AnlzFile { header, tags })
    }
}

/// Write ANLZ files for a track (all variants: .DAT, .EXT, .2EX)
///
/// # Arguments
/// * `mount_point` - USB device mount point
/// * `pdb_track_id` - Track ID in the PDB database (used for path generation)
/// * `device_audio_path` - Path to audio file on device (e.g., "/Contents/Artist/Album/track.mp3")
/// * `duration_ms` - Track duration in milliseconds
/// * `bpm` - Track BPM (optional)
/// * `cues` - Track cue points
///
/// # Returns
/// The ANLZ directory path (e.g., "/PIONEER/USBANLZ/P000/00000001/ANLZ0000.DAT")
pub fn write_anlz_files(
    mount_point: &str,
    pdb_track_id: u32,
    device_audio_path: &str,
    duration_ms: u32,
    bpm: Option<f32>,
    cues: &[Cue],
) -> crate::error::Result<String> {
    use std::fs;

    let anlz_dir = generate_anlz_dir(pdb_track_id);

    // Create directory
    let full_dir = Path::new(mount_point).join(&anlz_dir[1..]); // Remove leading /
    fs::create_dir_all(&full_dir).map_err(|e| {
        crate::error::CrateError::Device(format!("Failed to create ANLZ directory: {e}"))
    })?;

    // Write all three variants
    for variant in [AnlzVariant::Dat, AnlzVariant::Ext, AnlzVariant::Ext2] {
        let mut builder = AnlzFileBuilder::new(variant)
            .path(device_audio_path)
            .duration_ms(duration_ms)
            .cues(cues.to_vec());

        if let Some(b) = bpm {
            builder = builder.bpm(b);
        }

        let file = builder.build()?;

        let file_path = full_dir.join(variant.filename());
        file.write_to_file(&file_path)?;
    }

    // Return the .DAT path (primary ANLZ path)
    Ok(generate_anlz_path(pdb_track_id, AnlzVariant::Dat))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_dat() {
        let file = AnlzFileBuilder::new(AnlzVariant::Dat)
            .path("/Contents/test.mp3")
            .bpm(128.0)
            .duration_ms(180000) // 3 minutes
            .cues(vec![])
            .build()
            .unwrap();

        // .DAT should have 7 tags (PPTH, PVBR, PQTZ, PWAV, PWV2, PCOB x2)
        assert_eq!(file.tags.len(), 7);
    }

    #[test]
    fn test_builder_ext() {
        let file = AnlzFileBuilder::new(AnlzVariant::Ext)
            .path("/Contents/test.mp3")
            .bpm(128.0)
            .duration_ms(180000)
            .cues(vec![])
            .build()
            .unwrap();

        // .EXT should have 14 tags (7 .DAT + 7 .EXT tags)
        assert_eq!(file.tags.len(), 14);
    }

    #[test]
    fn test_builder_ext2() {
        let file = AnlzFileBuilder::new(AnlzVariant::Ext2)
            .path("/Contents/test.mp3")
            .bpm(128.0)
            .duration_ms(180000)
            .cues(vec![])
            .build()
            .unwrap();

        // .2EX should have 17 tags (14 .EXT + 3 .2EX tags)
        assert_eq!(file.tags.len(), 17);
    }

    #[test]
    fn test_roundtrip() {
        let file = AnlzFileBuilder::new(AnlzVariant::Dat)
            .path("/Contents/test.mp3")
            .bpm(128.0)
            .duration_ms(60000)
            .cues(vec![])
            .build()
            .unwrap();

        let bytes = file.to_bytes().unwrap();
        let parsed = AnlzFile::parse(&bytes).unwrap();

        assert_eq!(file.tags.len(), parsed.tags.len());
    }
}
