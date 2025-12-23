//! PVBR (VBR Seek Index) tag - index for seeking in variable bitrate files
//!
//! Contains 400 u32 entries that allow seeking to specific times in VBR audio.

use binrw::{BinRead, BinWrite};

/// Number of VBR index entries
const VBR_INDEX_ENTRIES: usize = 400;

/// PVBR tag header size (12 bytes standard + 4 bytes unknown)
const HEADER_SIZE: u32 = 16;

/// Size of VBR index data (400 u32 entries)
const DATA_SIZE: u32 = (VBR_INDEX_ENTRIES * 4) as u32;

/// PVBR (VBR Seek Index) tag
///
/// Stores an index allowing rapid seeking to particular times within
/// a variable-bitrate audio file. Contains 400 u32 entries.
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PVBR")]
pub struct VbrTag {
    /// Header size (always 16)
    pub len_header: u32,
    /// Total tag size (always 1616 = 16 + 1600)
    pub len_tag: u32,
    /// Unknown field (always 0)
    pub unknown: u32,
    /// VBR seek index entries (400 u32 values)
    #[br(count = VBR_INDEX_ENTRIES)]
    pub entries: Vec<u32>,
}

impl VbrTag {
    /// Total size of this tag in bytes
    pub const SIZE: u32 = HEADER_SIZE + DATA_SIZE;

    /// Create a new VBR tag with zeroed entries
    ///
    /// Zeroed entries indicate constant bitrate or linear seeking.
    pub fn new() -> Self {
        Self {
            len_header: HEADER_SIZE,
            len_tag: Self::SIZE,
            unknown: 0,
            entries: vec![0; VBR_INDEX_ENTRIES],
        }
    }

    /// Get the total size of this tag in bytes
    pub fn size(&self) -> u32 {
        Self::SIZE
    }
}

impl Default for VbrTag {
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
    fn test_vbr_tag_size() {
        let tag = VbrTag::new();
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(data.len(), VbrTag::SIZE as usize);
    }

    #[test]
    fn test_vbr_tag_magic() {
        let tag = VbrTag::new();
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(&data[0..4], b"PVBR");
    }
}
