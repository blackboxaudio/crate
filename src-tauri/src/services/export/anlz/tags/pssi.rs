//! PSSI (Song Structure) tag - phrase analysis for Rekordbox 6+
//!
//! Contains song structure information with XOR encryption.
//! Currently generates an empty placeholder.

use binrw::{BinRead, BinWrite};

/// PSSI tag header size (12 bytes standard + 8 bytes extra)
const HEADER_SIZE: u32 = 20;

/// PSSI (Song Structure) tag
///
/// Contains phrase/song structure analysis data.
/// Data after offset 18 is XOR encrypted in Rekordbox 6+.
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PSSI")]
pub struct SongStructureTag {
    /// Header size (always 20)
    pub len_header: u32,
    /// Total tag size
    pub len_tag: u32,
    /// Number of structure entries
    pub len_entries: u32,
    /// Unknown field (always 0)
    pub unknown: u32,
    /// Structure data (XOR encrypted after first 2 bytes if non-empty)
    #[br(count = len_tag.saturating_sub(HEADER_SIZE))]
    pub data: Vec<u8>,
}

impl SongStructureTag {
    /// Create an empty song structure tag (placeholder)
    ///
    /// Real song structure analysis requires audio processing
    /// which is planned for a later milestone.
    pub fn empty() -> Self {
        Self {
            len_header: HEADER_SIZE,
            len_tag: HEADER_SIZE,
            len_entries: 0,
            unknown: 0,
            data: vec![],
        }
    }

    /// Get the total size of this tag in bytes
    pub fn size(&self) -> u32 {
        self.len_tag
    }
}

impl Default for SongStructureTag {
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
    fn test_song_structure_magic() {
        let tag = SongStructureTag::empty();
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(&data[0..4], b"PSSI");
    }

    #[test]
    fn test_song_structure_empty_size() {
        let tag = SongStructureTag::empty();
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(data.len(), HEADER_SIZE as usize);
    }
}
