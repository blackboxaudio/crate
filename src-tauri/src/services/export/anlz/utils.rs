//! ANLZ utility functions for encoding and path generation

#![allow(dead_code)]

/// ANLZ file variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnlzVariant {
    /// Standard .DAT file with basic tags
    Dat,
    /// Extended .EXT file with color waveforms and extended cues
    Ext,
    /// Extended .2EX file with additional waveform data
    Ext2,
}

impl AnlzVariant {
    /// Get the file extension for this variant
    pub fn extension(&self) -> &'static str {
        match self {
            AnlzVariant::Dat => "DAT",
            AnlzVariant::Ext => "EXT",
            AnlzVariant::Ext2 => "2EX",
        }
    }

    /// Get the filename for this variant
    pub fn filename(&self) -> &'static str {
        match self {
            AnlzVariant::Dat => "ANLZ0000.DAT",
            AnlzVariant::Ext => "ANLZ0000.EXT",
            AnlzVariant::Ext2 => "ANLZ0000.2EX",
        }
    }
}

/// Encode a string as UTF-16 big-endian bytes with null terminator
pub fn to_utf16_be(s: &str) -> Vec<u8> {
    let utf16: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
    utf16.iter().flat_map(|&c| c.to_be_bytes()).collect()
}

/// Generate a unique ANLZ directory path based on track ID
///
/// Format: /PIONEER/USBANLZ/Pxxx/xxxxxxxx/
/// - Pxxx: First 3 hex chars of ID
/// - xxxxxxxx: Full 8-char hex ID
pub fn generate_anlz_dir(track_id: u32) -> String {
    let hex_id = format!("{track_id:08X}");
    let prefix = &hex_id[0..3];
    format!("/PIONEER/USBANLZ/P{prefix}/{hex_id}")
}

/// Generate the full ANLZ file path for a specific variant
pub fn generate_anlz_path(track_id: u32, variant: AnlzVariant) -> String {
    format!("{}/{}", generate_anlz_dir(track_id), variant.filename())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_anlz_path() {
        assert_eq!(
            generate_anlz_path(1, AnlzVariant::Dat),
            "/PIONEER/USBANLZ/P000/00000001/ANLZ0000.DAT"
        );
        assert_eq!(
            generate_anlz_path(1, AnlzVariant::Ext),
            "/PIONEER/USBANLZ/P000/00000001/ANLZ0000.EXT"
        );
        assert_eq!(
            generate_anlz_path(0x12345678, AnlzVariant::Ext2),
            "/PIONEER/USBANLZ/P123/12345678/ANLZ0000.2EX"
        );
    }

    #[test]
    fn test_to_utf16_be() {
        let result = to_utf16_be("AB");
        // 'A' = 0x0041, 'B' = 0x0042, null = 0x0000
        assert_eq!(result, vec![0x00, 0x41, 0x00, 0x42, 0x00, 0x00]);
    }
}
