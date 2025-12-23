//! PWAV (Waveform Preview) tag - low-resolution waveform for display
//!
//! Contains 400 bytes of waveform data for the preview display on CDJ/XDJ.

#![allow(dead_code)]

use binrw::{BinRead, BinWrite};

/// Number of waveform preview entries (columns on CDJ display)
const WAVEFORM_PREVIEW_WIDTH: usize = 400;

/// PWAV tag header size (12 bytes standard + 8 bytes extra)
const HEADER_SIZE: u32 = 20;

/// Default waveform byte value (height=8, whiteness=4 encoded as 0x44)
const DEFAULT_WAVEFORM_VALUE: u8 = 0x44;

/// PWAV (Waveform Preview) tag
///
/// Stores a low-resolution waveform preview for display on CDJ/XDJ equipment.
/// Each byte encodes height (5 bits) and whiteness (3 bits).
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PWAV")]
pub struct WaveformPreviewTag {
    /// Header size (always 20)
    pub len_header: u32,
    /// Total tag size (always 420 = 20 + 400)
    pub len_tag: u32,
    /// Length of waveform data (always 400)
    pub len_preview: u32,
    /// Unknown field (always 0x00100000)
    pub unknown: u32,
    /// Waveform data (400 bytes)
    #[br(count = len_preview)]
    pub data: Vec<u8>,
}

impl WaveformPreviewTag {
    /// Total size of this tag in bytes
    pub const SIZE: u32 = HEADER_SIZE + WAVEFORM_PREVIEW_WIDTH as u32;

    /// Create a new waveform preview tag with placeholder data
    ///
    /// The placeholder uses a flat waveform (height=8, whiteness=4).
    pub fn new() -> Self {
        Self {
            len_header: HEADER_SIZE,
            len_tag: Self::SIZE,
            len_preview: WAVEFORM_PREVIEW_WIDTH as u32,
            unknown: 0x00100000,
            data: vec![DEFAULT_WAVEFORM_VALUE; WAVEFORM_PREVIEW_WIDTH],
        }
    }

    /// Create a waveform preview tag from actual waveform data
    ///
    /// # Arguments
    /// * `waveform` - 400 bytes of waveform data
    ///
    /// # Panics
    /// Panics if waveform length is not 400
    pub fn from_data(waveform: Vec<u8>) -> Self {
        assert_eq!(waveform.len(), WAVEFORM_PREVIEW_WIDTH);
        Self {
            len_header: HEADER_SIZE,
            len_tag: Self::SIZE,
            len_preview: WAVEFORM_PREVIEW_WIDTH as u32,
            unknown: 0x00100000,
            data: waveform,
        }
    }

    /// Get the total size of this tag in bytes
    pub fn size(&self) -> u32 {
        Self::SIZE
    }
}

impl Default for WaveformPreviewTag {
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
    fn test_waveform_preview_size() {
        let tag = WaveformPreviewTag::new();
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(data.len(), WaveformPreviewTag::SIZE as usize);
    }

    #[test]
    fn test_waveform_preview_magic() {
        let tag = WaveformPreviewTag::new();
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(&data[0..4], b"PWAV");
    }
}
