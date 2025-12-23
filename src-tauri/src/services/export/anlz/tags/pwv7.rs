//! PWV7 (Extended Waveform 2) tag - additional waveform data for .2EX files
//!
//! Contains extended waveform data with an additional unknown constant.

#![allow(dead_code)]

use binrw::{BinRead, BinWrite};

/// PWV7 tag header size (12 bytes standard + 8 bytes extra)
const HEADER_SIZE: u32 = 20;

/// Bytes per waveform entry
const BYTES_PER_ENTRY: usize = 3;

/// Number of entries per second of audio
const ENTRIES_PER_SECOND: u32 = 150;

/// Default waveform values
const DEFAULT_VALUE: u8 = 0x44;

/// PWV7 (Extended Waveform 2) tag
///
/// Additional high-resolution waveform for .2EX files.
/// Similar to PWV6 but with a different unknown constant.
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PWV7")]
pub struct ExtWaveform2Tag {
    /// Header size (always 20)
    pub len_header: u32,
    /// Total tag size
    pub len_tag: u32,
    /// Number of entries
    pub len_entries: u32,
    /// Unknown field (always 0x00960000)
    pub unknown: u32,
    /// Waveform data (3 bytes per entry)
    #[br(count = len_entries as usize * BYTES_PER_ENTRY)]
    pub data: Vec<u8>,
}

impl ExtWaveform2Tag {
    /// Create a new extended waveform 2 tag with placeholder data
    ///
    /// # Arguments
    /// * `duration_ms` - Track duration in milliseconds
    pub fn new(duration_ms: u32) -> Self {
        let duration_sec = (duration_ms as f32) / 1000.0;
        let len_entries = (duration_sec * ENTRIES_PER_SECOND as f32) as u32;
        let len_entries = len_entries.max(1);

        let data = vec![DEFAULT_VALUE; len_entries as usize * BYTES_PER_ENTRY];

        Self {
            len_header: HEADER_SIZE,
            len_tag: HEADER_SIZE + (len_entries * BYTES_PER_ENTRY as u32),
            len_entries,
            unknown: 0x00960000,
            data,
        }
    }

    /// Create an extended waveform 2 tag from actual data
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
    fn test_ext_waveform2_magic() {
        let tag = ExtWaveform2Tag::new(60000);
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(&data[0..4], b"PWV7");
    }
}
