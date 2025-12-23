//! PWV5 (Waveform Color Detail) tag - detailed color waveform for .EXT files
//!
//! Contains high-resolution color waveform data with packed RGB values.

#![allow(dead_code)]

use binrw::{BinRead, BinWrite};

/// PWV5 tag header size (12 bytes standard + 8 bytes extra)
const HEADER_SIZE: u32 = 20;

/// Bytes per color entry (packed RGB + height)
const BYTES_PER_ENTRY: usize = 2;

/// Number of entries per second of audio
const ENTRIES_PER_SECOND: u32 = 150;

/// PWV5 (Waveform Color Detail) tag
///
/// High-resolution color waveform with packed RGB values.
/// Each entry is 2 bytes with packed RGB (5-6-5) and height.
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PWV5")]
pub struct WaveformColorDetailTag {
    /// Header size (always 20)
    pub len_header: u32,
    /// Total tag size
    pub len_tag: u32,
    /// Number of entries
    pub len_entries: u32,
    /// Unknown field (always 0x00960000)
    pub unknown: u32,
    /// Color waveform data
    #[br(count = len_entries as usize * BYTES_PER_ENTRY)]
    pub data: Vec<u8>,
}

impl WaveformColorDetailTag {
    /// Create a new color waveform detail tag with placeholder data
    ///
    /// # Arguments
    /// * `duration_ms` - Track duration in milliseconds
    pub fn new(duration_ms: u32) -> Self {
        let duration_sec = (duration_ms as f32) / 1000.0;
        let len_entries = (duration_sec * ENTRIES_PER_SECOND as f32) as u32;
        let len_entries = len_entries.max(1);

        // Each entry is 2 bytes - use a placeholder blue color
        let mut data = Vec::with_capacity(len_entries as usize * BYTES_PER_ENTRY);
        for _ in 0..len_entries {
            // Packed RGB 5-6-5 format with blue dominant
            data.push(0x00);
            data.push(0x1F); // Blue
        }

        Self {
            len_header: HEADER_SIZE,
            len_tag: HEADER_SIZE + (len_entries * BYTES_PER_ENTRY as u32),
            len_entries,
            unknown: 0x00960000,
            data,
        }
    }

    /// Create a color waveform detail tag from actual data
    pub fn from_data(data: Vec<u8>, len_entries: u32) -> Self {
        assert_eq!(data.len(), len_entries as usize * BYTES_PER_ENTRY);
        Self {
            len_header: HEADER_SIZE,
            len_tag: HEADER_SIZE + (len_entries * BYTES_PER_ENTRY as u32),
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
    fn test_color_detail_magic() {
        let tag = WaveformColorDetailTag::new(60000);
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(&data[0..4], b"PWV5");
    }

    #[test]
    fn test_color_detail_entries() {
        let tag = WaveformColorDetailTag::new(60000); // 1 minute = 60s * 150 = 9000 entries
        assert_eq!(tag.len_entries, 9000);
    }
}
