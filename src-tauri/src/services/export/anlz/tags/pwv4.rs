//! PWV4 (Waveform Color Preview) tag - color waveform preview for .EXT files
//!
//! Contains color waveform data with RGB values and luminance.

#![allow(dead_code)]

use binrw::{BinRead, BinWrite};

/// PWV4 tag header size (12 bytes standard + 8 bytes extra)
const HEADER_SIZE: u32 = 20;

/// Number of color waveform preview entries
const COLOR_PREVIEW_WIDTH: usize = 400;

/// Bytes per color entry (red, green, blue, luminance, padding x2)
const BYTES_PER_ENTRY: usize = 6;

/// PWV4 (Waveform Color Preview) tag
///
/// Color waveform preview with 400 entries, each containing RGB and luminance.
/// Each entry is 6 bytes: red, green, blue, luminance, padding x2.
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PWV4")]
pub struct WaveformColorPreviewTag {
    /// Header size (always 20)
    pub len_header: u32,
    /// Total tag size (always 2420 = 20 + 400*6)
    pub len_tag: u32,
    /// Number of entries (always 400)
    pub len_entries: u32,
    /// Unknown field (always 0x00100000)
    pub unknown: u32,
    /// Color waveform data (400 * 6 = 2400 bytes)
    #[br(count = len_entries as usize * BYTES_PER_ENTRY)]
    pub data: Vec<u8>,
}

impl WaveformColorPreviewTag {
    /// Total size of this tag in bytes
    pub const SIZE: u32 = HEADER_SIZE + (COLOR_PREVIEW_WIDTH * BYTES_PER_ENTRY) as u32;

    /// Create a new color waveform preview tag with placeholder data
    ///
    /// The placeholder uses a blue-ish color (typical DJ waveform).
    pub fn new() -> Self {
        // Each entry: R, G, B, Luminance, padding x2
        // Use a default blue-ish color
        let mut data = Vec::with_capacity(COLOR_PREVIEW_WIDTH * BYTES_PER_ENTRY);
        for _ in 0..COLOR_PREVIEW_WIDTH {
            data.push(0x20); // Red
            data.push(0x60); // Green
            data.push(0xFF); // Blue
            data.push(0x40); // Luminance
            data.push(0x00); // Padding
            data.push(0x00); // Padding
        }

        Self {
            len_header: HEADER_SIZE,
            len_tag: Self::SIZE,
            len_entries: COLOR_PREVIEW_WIDTH as u32,
            unknown: 0x00100000,
            data,
        }
    }

    /// Create a color waveform preview from actual data
    pub fn from_data(data: Vec<u8>) -> Self {
        assert_eq!(data.len(), COLOR_PREVIEW_WIDTH * BYTES_PER_ENTRY);
        Self {
            len_header: HEADER_SIZE,
            len_tag: Self::SIZE,
            len_entries: COLOR_PREVIEW_WIDTH as u32,
            unknown: 0x00100000,
            data,
        }
    }

    /// Get the total size of this tag in bytes
    pub fn size(&self) -> u32 {
        Self::SIZE
    }
}

impl Default for WaveformColorPreviewTag {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use binrw::BinWrite;
    use std::io::Cursor;

    #[test]
    fn test_color_preview_size() {
        let tag = WaveformColorPreviewTag::new();
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(data.len(), WaveformColorPreviewTag::SIZE as usize);
    }

    #[test]
    fn test_color_preview_magic() {
        let tag = WaveformColorPreviewTag::new();
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(&data[0..4], b"PWV4");
    }
}
