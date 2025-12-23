//! DeviceSQL string encoding for PDB files
//!
//! The PDB format uses three different string encodings:
//! - ShortAscii: For ASCII strings <= 126 bytes (1-byte header)
//! - LongAscii: For ASCII strings > 126 bytes (4-byte header)
//! - LongUtf16: For non-ASCII strings (4-byte header + UTF-16LE content)

#![allow(dead_code)]

use std::io::{self, Read, Write};

use binrw::{BinRead, BinWrite};

use super::constants::MAX_SHORT_STRING_LEN;

/// DeviceSQL string encoding variants
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceSQLString {
    /// Short ASCII string (length <= 126)
    /// Header byte: ((len + 1) << 1) | 1
    ShortAscii(Vec<u8>),

    /// Long ASCII string (length > 126)
    /// Format: 0x40 + u16 len (includes 4-byte header) + 0x00 + content
    LongAscii(Vec<u8>),

    /// UTF-16LE string (for non-ASCII content)
    /// Format: 0x90 + u16 len (char_count * 2 + 4) + 0x00 + UTF-16LE content
    LongUtf16(Vec<u16>),
}

impl DeviceSQLString {
    /// Create a new DeviceSQLString from a Rust string
    pub fn new(s: &str) -> Self {
        let bytes = s.as_bytes();
        if s.is_ascii() && bytes.len() <= MAX_SHORT_STRING_LEN {
            Self::ShortAscii(bytes.to_vec())
        } else if s.is_ascii() {
            Self::LongAscii(bytes.to_vec())
        } else {
            Self::LongUtf16(s.encode_utf16().collect())
        }
    }

    /// Create an empty string
    pub fn empty() -> Self {
        Self::ShortAscii(Vec::new())
    }

    /// Get the serialized size of this string
    pub fn binary_size(&self) -> usize {
        match self {
            Self::ShortAscii(bytes) => 1 + bytes.len(),
            Self::LongAscii(bytes) => 4 + bytes.len(),
            Self::LongUtf16(chars) => 4 + chars.len() * 2,
        }
    }

    /// Write the string to a writer
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::ShortAscii(bytes) => {
                // Header byte: ((len + 1) << 1) | 1
                let header = (((bytes.len() + 1) << 1) | 1) as u8;
                writer.write_all(&[header])?;
                writer.write_all(bytes)?;
            }
            Self::LongAscii(bytes) => {
                // Flags: 0x40 for ASCII
                writer.write_all(&[0x40])?;
                // Length: content length + 4 (includes header)
                let len = (bytes.len() + 4) as u16;
                writer.write_all(&len.to_le_bytes())?;
                // Padding byte
                writer.write_all(&[0x00])?;
                // Content
                writer.write_all(bytes)?;
            }
            Self::LongUtf16(chars) => {
                // Flags: 0x90 for UTF-16LE
                writer.write_all(&[0x90])?;
                // Length: (char count * 2) + 4 (includes header)
                let len = (chars.len() * 2 + 4) as u16;
                writer.write_all(&len.to_le_bytes())?;
                // Padding byte
                writer.write_all(&[0x00])?;
                // Content as little-endian UTF-16
                for c in chars {
                    writer.write_all(&c.to_le_bytes())?;
                }
            }
        }
        Ok(())
    }

    /// Convert to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.binary_size());
        self.write_to(&mut buf).expect("write to Vec should not fail");
        buf
    }

    /// Read a DeviceSQLString from a reader
    pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut header = [0u8; 1];
        reader.read_exact(&mut header)?;

        let header_byte = header[0];

        // Check if this is a short ASCII string (bit 0 is set)
        if header_byte & 1 == 1 {
            // ShortAscii: header = ((len + 1) << 1) | 1
            let len = ((header_byte >> 1) as usize).saturating_sub(1);
            let mut content = vec![0u8; len];
            reader.read_exact(&mut content)?;
            Ok(Self::ShortAscii(content))
        } else if header_byte == 0x40 {
            // LongAscii: 0x40 + u16 len + 0x00 + content
            let mut len_bytes = [0u8; 2];
            reader.read_exact(&mut len_bytes)?;
            let total_len = u16::from_le_bytes(len_bytes) as usize;

            // Read padding byte
            let mut padding = [0u8; 1];
            reader.read_exact(&mut padding)?;

            // Content length = total_len - 4 (header size)
            let content_len = total_len.saturating_sub(4);
            let mut content = vec![0u8; content_len];
            reader.read_exact(&mut content)?;
            Ok(Self::LongAscii(content))
        } else if header_byte == 0x90 {
            // LongUtf16: 0x90 + u16 len + 0x00 + UTF-16LE content
            let mut len_bytes = [0u8; 2];
            reader.read_exact(&mut len_bytes)?;
            let total_len = u16::from_le_bytes(len_bytes) as usize;

            // Read padding byte
            let mut padding = [0u8; 1];
            reader.read_exact(&mut padding)?;

            // Content length = (total_len - 4) bytes, which is (total_len - 4) / 2 chars
            let byte_len = total_len.saturating_sub(4);
            let char_count = byte_len / 2;
            let mut chars = Vec::with_capacity(char_count);

            for _ in 0..char_count {
                let mut char_bytes = [0u8; 2];
                reader.read_exact(&mut char_bytes)?;
                chars.push(u16::from_le_bytes(char_bytes));
            }
            Ok(Self::LongUtf16(chars))
        } else {
            // Unknown format - treat as empty
            Ok(Self::ShortAscii(Vec::new()))
        }
    }

    /// Convert to a Rust String
    pub fn to_string_lossy(&self) -> String {
        match self {
            Self::ShortAscii(bytes) | Self::LongAscii(bytes) => {
                String::from_utf8_lossy(bytes).into_owned()
            }
            Self::LongUtf16(chars) => String::from_utf16_lossy(chars),
        }
    }
}

impl BinWrite for DeviceSQLString {
    type Args<'a> = ();

    fn write_options<W: Write + io::Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        self.write_to(writer)?;
        Ok(())
    }
}

impl BinRead for DeviceSQLString {
    type Args<'a> = ();

    fn read_options<R: Read + io::Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        Self::read_from(reader).map_err(|e| binrw::Error::Io(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_ascii() {
        let s = DeviceSQLString::new("test");
        assert!(matches!(s, DeviceSQLString::ShortAscii(_)));
        assert_eq!(s.binary_size(), 5); // 1 header + 4 chars
    }

    #[test]
    fn test_empty_string() {
        let s = DeviceSQLString::empty();
        assert!(matches!(s, DeviceSQLString::ShortAscii(ref v) if v.is_empty()));
        assert_eq!(s.binary_size(), 1); // Just header
    }

    #[test]
    fn test_short_ascii_header() {
        let s = DeviceSQLString::new("hi");
        let bytes = s.to_bytes();
        // Header should be ((2 + 1) << 1) | 1 = 7
        assert_eq!(bytes[0], 7);
        assert_eq!(&bytes[1..], b"hi");
    }

    #[test]
    fn test_utf16_for_non_ascii() {
        let s = DeviceSQLString::new("日本語");
        assert!(matches!(s, DeviceSQLString::LongUtf16(_)));
    }

    #[test]
    fn test_long_ascii() {
        let long_str = "a".repeat(200);
        let s = DeviceSQLString::new(&long_str);
        assert!(matches!(s, DeviceSQLString::LongAscii(_)));

        let bytes = s.to_bytes();
        assert_eq!(bytes[0], 0x40); // ASCII flag
    }

    #[test]
    fn test_read_short_ascii() {
        let original = DeviceSQLString::new("test");
        let bytes = original.to_bytes();

        let mut cursor = std::io::Cursor::new(bytes);
        let read_back = DeviceSQLString::read_from(&mut cursor).unwrap();

        assert_eq!(original, read_back);
        assert_eq!(read_back.to_string_lossy(), "test");
    }

    #[test]
    fn test_read_empty() {
        let original = DeviceSQLString::empty();
        let bytes = original.to_bytes();

        let mut cursor = std::io::Cursor::new(bytes);
        let read_back = DeviceSQLString::read_from(&mut cursor).unwrap();

        assert_eq!(original, read_back);
        assert_eq!(read_back.to_string_lossy(), "");
    }

    #[test]
    fn test_read_long_ascii() {
        let long_str = "a".repeat(200);
        let original = DeviceSQLString::new(&long_str);
        let bytes = original.to_bytes();

        let mut cursor = std::io::Cursor::new(bytes);
        let read_back = DeviceSQLString::read_from(&mut cursor).unwrap();

        assert_eq!(original, read_back);
        assert_eq!(read_back.to_string_lossy(), long_str);
    }

    #[test]
    fn test_read_utf16() {
        let original = DeviceSQLString::new("日本語テスト");
        let bytes = original.to_bytes();

        let mut cursor = std::io::Cursor::new(bytes);
        let read_back = DeviceSQLString::read_from(&mut cursor).unwrap();

        assert_eq!(original, read_back);
        assert_eq!(read_back.to_string_lossy(), "日本語テスト");
    }

    #[test]
    fn test_roundtrip_various_strings() {
        let test_cases: Vec<String> = vec![
            "".to_string(),
            "A".to_string(),
            "Hello World".to_string(),
            "Track Title - Artist Name".to_string(),
            "a".repeat(126),  // Max short ASCII
            "a".repeat(127),  // Long ASCII threshold
            "日本語".to_string(),
            "Café résumé naïve".to_string(),
            "Mixed 日本語 and ASCII".to_string(),
        ];

        for case in test_cases {
            let original = DeviceSQLString::new(&case);
            let bytes = original.to_bytes();

            let mut cursor = std::io::Cursor::new(bytes);
            let read_back = DeviceSQLString::read_from(&mut cursor).unwrap();

            assert_eq!(
                original, read_back,
                "Round-trip failed for: {:?}",
                case
            );
            assert_eq!(
                read_back.to_string_lossy(),
                case,
                "String content mismatch for: {:?}",
                case
            );
        }
    }
}
