//! PWVC (Extended Waveform Color) tag - color waveform for .2EX files
//!
//! Contains extended color waveform data with a minimal header.

#![allow(dead_code)]

use binrw::{BinRead, BinWrite};

/// PWVC tag header size (minimal, 14 bytes)
const HEADER_SIZE: u32 = 14;

/// PWVC (Extended Waveform Color) tag
///
/// Extended color waveform for .2EX files.
/// Uses a minimal 14-byte header format.
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PWVC")]
pub struct ExtWaveformColorTag {
    /// Header size (always 14)
    pub len_header: u32,
    /// Total tag size
    pub len_tag: u32,
    /// Unknown field (always 0)
    pub unknown1: u16,
    /// Number of entries
    pub len_entries: u32,
    /// Color waveform data
    #[br(count = len_tag.saturating_sub(HEADER_SIZE))]
    pub data: Vec<u8>,
}

impl ExtWaveformColorTag {
    /// Create an empty extended waveform color tag (placeholder)
    ///
    /// Real color waveform requires audio analysis which is
    /// planned for a later milestone.
    pub fn empty() -> Self {
        Self {
            len_header: HEADER_SIZE,
            len_tag: HEADER_SIZE,
            unknown1: 0,
            len_entries: 0,
            data: vec![],
        }
    }

    /// Create an extended waveform color tag from actual data
    pub fn from_data(data: Vec<u8>, len_entries: u32) -> Self {
        let len_tag = HEADER_SIZE + data.len() as u32;
        Self {
            len_header: HEADER_SIZE,
            len_tag,
            unknown1: 0,
            len_entries,
            data,
        }
    }

    /// Get the total size of this tag in bytes
    pub fn size(&self) -> u32 {
        self.len_tag
    }
}

impl Default for ExtWaveformColorTag {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use binrw::BinWrite;
    use std::io::Cursor;

    #[test]
    fn test_ext_waveform_color_magic() {
        let tag = ExtWaveformColorTag::empty();
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(&data[0..4], b"PWVC");
    }

    #[test]
    fn test_ext_waveform_color_empty_size() {
        let tag = ExtWaveformColorTag::empty();
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(data.len(), HEADER_SIZE as usize);
    }
}
