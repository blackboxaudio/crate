//! PWV2 (Tiny Waveform Preview) tag - very low-resolution waveform
//!
//! Contains 100 bytes of waveform data for the tiniest preview display.

#![allow(dead_code)]

use binrw::{BinRead, BinWrite};

/// Number of tiny waveform entries
const TINY_WAVEFORM_WIDTH: usize = 100;

/// PWV2 tag header size (12 bytes standard + 8 bytes extra)
const HEADER_SIZE: u32 = 20;

/// Default waveform byte value (height=8, whiteness=4 encoded as 0x44)
const DEFAULT_WAVEFORM_VALUE: u8 = 0x44;

/// PWV2 (Tiny Waveform Preview) tag
///
/// Stores a very low-resolution waveform preview (100 bytes).
/// Each byte encodes height (5 bits) and whiteness (3 bits).
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PWV2")]
pub struct TinyWaveformTag {
    /// Header size (always 20)
    pub len_header: u32,
    /// Total tag size (always 120 = 20 + 100)
    pub len_tag: u32,
    /// Length of waveform data (always 100)
    pub len_preview: u32,
    /// Unknown field (always 0x00100000)
    pub unknown: u32,
    /// Waveform data (100 bytes)
    #[br(count = len_preview)]
    pub data: Vec<u8>,
}

impl TinyWaveformTag {
    /// Total size of this tag in bytes
    pub const SIZE: u32 = HEADER_SIZE + TINY_WAVEFORM_WIDTH as u32;

    /// Create a new tiny waveform tag with placeholder data
    pub fn new() -> Self {
        Self {
            len_header: HEADER_SIZE,
            len_tag: Self::SIZE,
            len_preview: TINY_WAVEFORM_WIDTH as u32,
            unknown: 0x00100000,
            data: vec![DEFAULT_WAVEFORM_VALUE; TINY_WAVEFORM_WIDTH],
        }
    }

    /// Create a tiny waveform tag from actual waveform data
    ///
    /// # Arguments
    /// * `waveform` - 100 bytes of waveform data
    ///
    /// # Panics
    /// Panics if waveform length is not 100
    pub fn from_data(waveform: Vec<u8>) -> Self {
        assert_eq!(waveform.len(), TINY_WAVEFORM_WIDTH);
        Self {
            len_header: HEADER_SIZE,
            len_tag: Self::SIZE,
            len_preview: TINY_WAVEFORM_WIDTH as u32,
            unknown: 0x00100000,
            data: waveform,
        }
    }

    /// Get the total size of this tag in bytes
    pub fn size(&self) -> u32 {
        Self::SIZE
    }
}

impl Default for TinyWaveformTag {
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
    fn test_tiny_waveform_size() {
        let tag = TinyWaveformTag::new();
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(data.len(), TinyWaveformTag::SIZE as usize);
    }

    #[test]
    fn test_tiny_waveform_magic() {
        let tag = TinyWaveformTag::new();
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(&data[0..4], b"PWV2");
    }
}
