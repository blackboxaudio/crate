//! PMAI file header for ANLZ files
//!
//! All ANLZ files start with a 28-byte PMAI header.

use binrw::{BinRead, BinWrite};

/// PMAI file header (28 bytes)
///
/// This header appears at the start of every ANLZ file (.DAT, .EXT, .2EX).
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PMAI")]
pub struct AnlzFileHeader {
    /// Header size (always 28)
    pub len_header: u32,
    /// Total file size including header
    pub len_file: u32,
    /// Unknown field (always 0x00000001)
    #[brw(pad_before = 0)]
    pub unknown1: u32,
    /// Unknown field (always 0x00010000)
    pub unknown2: u32,
    /// Unknown field (always 0x00010000)
    pub unknown3: u32,
    /// Padding (always 0x00000000)
    pub unknown4: u32,
}

impl AnlzFileHeader {
    /// Header size in bytes
    pub const SIZE: u32 = 28;

    /// Create a new ANLZ file header
    ///
    /// # Arguments
    /// * `content_size` - Size of all tags combined (not including this header)
    pub fn new(content_size: u32) -> Self {
        Self {
            len_header: Self::SIZE,
            len_file: Self::SIZE + content_size,
            unknown1: 0x00000001,
            unknown2: 0x00010000,
            unknown3: 0x00010000,
            unknown4: 0x00000000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use binrw::BinWrite;
    use std::io::Cursor;

    #[test]
    fn test_header_size() {
        let header = AnlzFileHeader::new(0);
        let mut buf = Cursor::new(Vec::new());
        header.write(&mut buf).unwrap();
        assert_eq!(buf.into_inner().len(), 28);
    }

    #[test]
    fn test_header_magic() {
        let header = AnlzFileHeader::new(100);
        let mut buf = Cursor::new(Vec::new());
        header.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(&data[0..4], b"PMAI");
    }
}
