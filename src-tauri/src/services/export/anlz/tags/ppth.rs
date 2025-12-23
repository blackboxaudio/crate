//! PPTH (Path) tag - device path for the audio file
//!
//! Contains the UTF-16-BE encoded path to the audio file on the device.

use binrw::{BinRead, BinWrite};

use crate::services::export::anlz::utils::to_utf16_be;

/// PPTH tag header size (12 bytes standard + 4 bytes path length)
const HEADER_SIZE: u32 = 16;

/// PPTH (Path) tag
///
/// Stores the device path to the audio file in UTF-16 big-endian encoding.
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PPTH")]
pub struct PathTag {
    /// Header size (always 16)
    pub len_header: u32,
    /// Total tag size including header and path data
    pub len_tag: u32,
    /// Length of path data in bytes
    pub len_path: u32,
    /// Path data (UTF-16-BE encoded with null terminator)
    #[br(count = len_path)]
    pub path: Vec<u8>,
}

impl PathTag {
    /// Create a new path tag
    ///
    /// # Arguments
    /// * `path` - The device path (e.g., "/Contents/Artist/Album/track.mp3")
    pub fn new(path: &str) -> Self {
        let path_bytes = to_utf16_be(path);
        let len_path = path_bytes.len() as u32;

        Self {
            len_header: HEADER_SIZE,
            len_tag: HEADER_SIZE + len_path,
            len_path,
            path: path_bytes,
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
    fn test_path_tag() {
        let tag = PathTag::new("/Contents/test.mp3");
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();

        // Check magic
        assert_eq!(&data[0..4], b"PPTH");
        // Check header size
        assert_eq!(u32::from_be_bytes([data[4], data[5], data[6], data[7]]), 16);
    }
}
