//! PWV3 (Waveform Detail) tag - high-resolution waveform for .EXT files
//!
//! Contains detailed waveform data scaled to track length.

#![allow(dead_code)]

use binrw::{BinRead, BinWrite};

/// PWV3 tag header size (12 bytes standard + 8 bytes extra)
const HEADER_SIZE: u32 = 20;

/// Default waveform byte value (height=8, whiteness=4 encoded as 0x44)
const DEFAULT_WAVEFORM_VALUE: u8 = 0x44;

/// Number of entries per second of audio (at 150 BPM reference)
const ENTRIES_PER_SECOND: u32 = 75;

/// PWV3 (Waveform Detail) tag
///
/// High-resolution waveform data for detailed display.
/// Number of entries scales with track duration.
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PWV3")]
pub struct WaveformDetailTag {
    /// Header size (always 20)
    pub len_header: u32,
    /// Total tag size
    pub len_tag: u32,
    /// Number of waveform entries
    pub len_entries: u32,
    /// Unknown field (always 0x00960000)
    pub unknown: u32,
    /// Waveform data
    #[br(count = len_entries)]
    pub data: Vec<u8>,
}

impl WaveformDetailTag {
    /// Create a new waveform detail tag with placeholder data
    ///
    /// # Arguments
    /// * `duration_ms` - Track duration in milliseconds
    pub fn new(duration_ms: u32) -> Self {
        // Calculate number of entries based on duration
        let duration_sec = (duration_ms as f32) / 1000.0;
        let len_entries = (duration_sec * ENTRIES_PER_SECOND as f32) as u32;
        let len_entries = len_entries.max(1); // At least 1 entry

        Self {
            len_header: HEADER_SIZE,
            len_tag: HEADER_SIZE + len_entries,
            len_entries,
            unknown: 0x00960000,
            data: vec![DEFAULT_WAVEFORM_VALUE; len_entries as usize],
        }
    }

    /// Create a waveform detail tag from actual waveform data
    pub fn from_data(data: Vec<u8>) -> Self {
        let len_entries = data.len() as u32;
        Self {
            len_header: HEADER_SIZE,
            len_tag: HEADER_SIZE + len_entries,
            len_entries,
            unknown: 0x00960000,
            data,
        }
    }

    /// Get the total size of this tag in bytes
    pub fn size(&self) -> u32 {
        self.len_tag
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use binrw::BinWrite;
    use std::io::Cursor;

    #[test]
    fn test_waveform_detail_magic() {
        let tag = WaveformDetailTag::new(60000); // 1 minute
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(&data[0..4], b"PWV3");
    }

    #[test]
    fn test_waveform_detail_entries() {
        let tag = WaveformDetailTag::new(60000); // 1 minute = 60s * 75 = 4500 entries
        assert_eq!(tag.len_entries, 4500);
    }
}
