//! PQT2 (Extended Beat Grid) tag - beat timing for .EXT files
//!
//! Same format as PQTZ but used in .EXT and .2EX files.

use binrw::{BinRead, BinWrite};

use super::pqtz::BeatGridEntry;

/// PQT2 tag header size (12 bytes standard + 12 bytes content header)
const HEADER_SIZE: u32 = 24;

/// Beat grid entry size in bytes
const ENTRY_SIZE: u32 = 8;

/// Maximum number of beats to prevent excessive memory usage
const MAX_BEATS: u32 = 10000;

/// PQT2 (Extended Beat Grid) tag
///
/// Same structure as PQTZ but used in .EXT and .2EX files.
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PQT2")]
pub struct ExtBeatGridTag {
    /// Header size (always 24)
    pub len_header: u32,
    /// Total tag size including entries
    pub len_tag: u32,
    /// Unknown field (always 0)
    pub unknown1: u32,
    /// Unknown field (always 0x00080000)
    pub unknown2: u32,
    /// Number of beat entries
    pub entry_count: u32,
    /// Beat grid entries
    #[br(count = entry_count)]
    pub entries: Vec<BeatGridEntry>,
}

impl ExtBeatGridTag {
    /// Create a new extended beat grid tag from BPM and duration
    ///
    /// # Arguments
    /// * `bpm` - Beats per minute
    /// * `duration_ms` - Track duration in milliseconds
    pub fn new(bpm: f32, duration_ms: u32) -> Self {
        let tempo = (bpm * 100.0) as u16;
        let beat_duration_ms = 60000.0 / bpm;
        let num_beats = ((duration_ms as f32) / beat_duration_ms) as u32;
        let num_beats = num_beats.min(MAX_BEATS);

        let mut entries = Vec::with_capacity(num_beats as usize);
        let mut time_ms: f32 = 0.0;

        for i in 0..num_beats {
            entries.push(BeatGridEntry {
                beat: ((i % 4) + 1) as u16,
                tempo,
                time_ms: time_ms as u32,
            });
            time_ms += beat_duration_ms;
        }

        let entry_count = entries.len() as u32;

        Self {
            len_header: HEADER_SIZE,
            len_tag: HEADER_SIZE + (entry_count * ENTRY_SIZE),
            unknown1: 0,
            unknown2: 0x00080000,
            entry_count,
            entries,
        }
    }

    /// Create an empty extended beat grid
    pub fn empty() -> Self {
        Self {
            len_header: HEADER_SIZE,
            len_tag: HEADER_SIZE,
            unknown1: 0,
            unknown2: 0x00080000,
            entry_count: 0,
            entries: vec![],
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
    fn test_ext_beat_grid_magic() {
        let tag = ExtBeatGridTag::new(128.0, 1000);
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(&data[0..4], b"PQT2");
    }
}
