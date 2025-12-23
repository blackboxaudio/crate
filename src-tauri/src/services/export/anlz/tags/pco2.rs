//! PCO2 (Extended Cue List) tag - cue points with colors and comments
//!
//! Contains extended cue point information (PCP2 entries) with color and comment support.
//!
//! Reference: pyrekordbox and Deep Symmetry analysis

#![allow(dead_code)]

use binrw::{BinRead, BinWrite};

use crate::models::{Cue, CueType};
use crate::services::export::anlz::utils::to_utf16_be;

/// PCO2 tag header size (0x14 = 20 bytes)
/// Note: This is smaller than PCOB (24 bytes) as PCO2 doesn't have memory_marker
const HEADER_SIZE: u32 = 20;

/// PCP2 cue entry header size (0x10 = 16 bytes)
const PCP2_HEADER_SIZE: u32 = 16;

/// PCP2 struct size after magic, before comment (in bytes)
/// len_header(4) + len_entry(4) + hot_cue(4) + cue_type(1) + pad(3) +
/// time(4) + loop_time(4) + color_id(1) + pad(7) + loop_num(2) + loop_denom(2) + len_comment(4) = 40
const PCP2_STRUCT_SIZE: u32 = 40;

/// PCP2 color bytes size (after comment)
const PCP2_COLOR_SIZE: u32 = 4;

/// Cue list type for memory cues
pub const CUE_LIST_TYPE_MEMORY: u32 = 0;

/// Cue list type for hot cues
pub const CUE_LIST_TYPE_HOT: u32 = 1;

/// Pioneer color IDs (1-8)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PioneerColor {
    None = 0,
    Pink = 1,
    Red = 2,
    Orange = 3,
    Yellow = 4,
    Green = 5,
    Aqua = 6,
    Blue = 7,
    Purple = 8,
}

impl PioneerColor {
    /// Map a color name to a Pioneer color ID
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "pink" | "magenta" => PioneerColor::Pink,
            "red" => PioneerColor::Red,
            "orange" => PioneerColor::Orange,
            "yellow" => PioneerColor::Yellow,
            "green" | "lime" => PioneerColor::Green,
            "aqua" | "cyan" | "teal" => PioneerColor::Aqua,
            "blue" => PioneerColor::Blue,
            "purple" | "violet" => PioneerColor::Purple,
            _ => PioneerColor::None,
        }
    }

    /// Get RGB color values for this Pioneer color ID
    pub fn to_rgb(&self) -> (u8, u8, u8) {
        match self {
            PioneerColor::None => (0, 0, 0),
            PioneerColor::Pink => (255, 0, 255),
            PioneerColor::Red => (255, 0, 0),
            PioneerColor::Orange => (255, 127, 0),
            PioneerColor::Yellow => (255, 255, 0),
            PioneerColor::Green => (0, 255, 0),
            PioneerColor::Aqua => (0, 255, 255),
            PioneerColor::Blue => (0, 0, 255),
            PioneerColor::Purple => (127, 0, 255),
        }
    }
}

/// A single extended cue point entry (PCP2)
///
/// Format from pyrekordbox/Deep Symmetry:
/// - Bytes 0-3: "PCP2" magic
/// - Bytes 4-7: len_header (0x10 = 16)
/// - Bytes 8-11: len_entry (variable)
/// - Bytes 12-15: hot_cue
/// - Byte 16: cue_type (1=single, 2=loop)
/// - Bytes 17-19: padding (3 bytes)
/// - Bytes 20-23: time_ms
/// - Bytes 24-27: loop_time_ms
/// - Byte 28: color_id
/// - Bytes 29-35: padding (7 bytes)
/// - Bytes 36-37: loop_numerator
/// - Bytes 38-39: loop_denominator
/// - Bytes 40-43: len_comment
/// - Bytes 44+: comment (UTF-16-BE)
/// - After comment: color_code, color_r, color_g, color_b (4 bytes)
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PCP2")]
pub struct ExtCuePointEntry {
    /// Header size (always 0x10 = 16)
    pub len_header: u32,
    /// Total entry size (variable based on comment length)
    pub len_entry: u32,
    /// Hot cue index (0-7 for hot cues, 0 for memory cues)
    pub hot_cue: u32,
    /// Cue type (1 = regular cue, 2 = loop)
    pub cue_type: u8,
    /// Padding (3 bytes)
    pub padding1: [u8; 3],
    /// Time position in milliseconds
    pub time_ms: u32,
    /// Loop end time in milliseconds (0 if not a loop)
    pub loop_time_ms: u32,
    /// Pioneer color ID (0-8)
    pub color_id: u8,
    /// Padding (7 bytes)
    pub padding2: [u8; 7],
    /// Loop numerator (0 if not a loop)
    pub loop_numerator: u16,
    /// Loop denominator (0 if not a loop)
    pub loop_denominator: u16,
    /// Length of comment in bytes (including null terminator)
    pub len_comment: u32,
    /// Comment text (UTF-16-BE encoded with null terminator)
    #[br(count = len_comment)]
    pub comment: Vec<u8>,
    /// Color code byte
    pub color_code: u8,
    /// Color red component
    pub color_red: u8,
    /// Color green component
    pub color_green: u8,
    /// Color blue component
    pub color_blue: u8,
}

impl ExtCuePointEntry {
    /// Create a new extended cue point entry from a Cue
    ///
    /// # Arguments
    /// * `cue` - The cue to convert
    pub fn from_cue(cue: &Cue) -> Self {
        let (hot_cue, cue_type, loop_time_ms, loop_num, loop_denom) = match cue.cue_type {
            CueType::Memory => (0, 1, 0, 0, 0),
            CueType::Hot => (cue.hot_cue_index.unwrap_or(0) as u32, 1, 0, 0, 0),
            CueType::Loop => {
                let end = cue.loop_end_ms.unwrap_or(0) as u32;
                (0, 2, end, 1, 1) // Default to 1/1 loop
            }
        };

        let pioneer_color = cue
            .color
            .as_ref()
            .map(|c| PioneerColor::from_name(c))
            .unwrap_or(PioneerColor::None);

        let color_id = pioneer_color as u8;
        let (color_red, color_green, color_blue) = pioneer_color.to_rgb();

        // Encode comment (or empty with just null terminator)
        let comment_text = cue.name.as_deref().unwrap_or("");
        let comment = if comment_text.is_empty() {
            vec![0, 0] // Just null terminator (2 bytes for UTF-16)
        } else {
            to_utf16_be(comment_text)
        };
        let len_comment = comment.len() as u32;

        // Total entry size: magic(4) + struct(40) + comment + color(4)
        let len_entry = 4 + PCP2_STRUCT_SIZE + len_comment + PCP2_COLOR_SIZE;

        Self {
            len_header: PCP2_HEADER_SIZE,
            len_entry,
            hot_cue,
            cue_type,
            padding1: [0; 3],
            time_ms: cue.position_ms as u32,
            loop_time_ms,
            color_id,
            padding2: [0; 7],
            loop_numerator: loop_num,
            loop_denominator: loop_denom,
            len_comment,
            comment,
            color_code: color_id,
            color_red,
            color_green,
            color_blue,
        }
    }

    /// Get the total size of this entry in bytes (including magic)
    pub fn size(&self) -> u32 {
        // This should equal len_entry: magic(4) + struct(40) + comment + color(4)
        4 + PCP2_STRUCT_SIZE + self.len_comment + PCP2_COLOR_SIZE
    }
}

/// PCO2 (Extended Cue List) tag
///
/// Contains extended cue points with colors and comments.
///
/// Format from pyrekordbox/Deep Symmetry:
/// - Bytes 0-3: "PCO2" magic
/// - Bytes 4-7: len_header (0x14 = 20)
/// - Bytes 8-11: len_tag
/// - Bytes 12-15: list_type (0=memory, 1=hot cue)
/// - Bytes 16-17: entry_count
/// - Bytes 18-19: unknown (always 0)
/// - Entries follow at byte 20
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PCO2")]
pub struct ExtCueListTag {
    /// Header size (always 0x14 = 20)
    pub len_header: u32,
    /// Total tag size including entries
    pub len_tag: u32,
    /// Cue list type (0 = memory/loop, 1 = hot cue)
    pub list_type: u32,
    /// Number of cue entries
    pub entry_count: u16,
    /// Unknown field (always 0)
    pub unknown: u16,
    /// Extended cue point entries
    #[br(count = entry_count)]
    pub entries: Vec<ExtCuePointEntry>,
}

impl ExtCueListTag {
    /// Create an empty extended cue list
    ///
    /// # Arguments
    /// * `list_type` - 0 for memory/loop cues, 1 for hot cues
    pub fn empty(list_type: u32) -> Self {
        Self {
            len_header: HEADER_SIZE,
            len_tag: HEADER_SIZE,
            list_type,
            entry_count: 0,
            unknown: 0,
            entries: vec![],
        }
    }

    /// Create an extended cue list for memory cues (including loops)
    ///
    /// # Arguments
    /// * `cues` - All cues for the track (will filter to memory and loop cues)
    pub fn memory_cues(cues: &[Cue]) -> Self {
        let entries: Vec<ExtCuePointEntry> = cues
            .iter()
            .filter(|c| matches!(c.cue_type, CueType::Memory | CueType::Loop))
            .map(ExtCuePointEntry::from_cue)
            .collect();

        let entry_count = entries.len() as u16;
        let entries_size: u32 = entries.iter().map(|e| e.size()).sum();
        let tag_size = HEADER_SIZE + entries_size;

        Self {
            len_header: HEADER_SIZE,
            len_tag: tag_size,
            list_type: CUE_LIST_TYPE_MEMORY,
            entry_count,
            unknown: 0,
            entries,
        }
    }

    /// Create an extended cue list for hot cues
    ///
    /// # Arguments
    /// * `cues` - All cues for the track (will filter to hot cues)
    pub fn hot_cues(cues: &[Cue]) -> Self {
        let entries: Vec<ExtCuePointEntry> = cues
            .iter()
            .filter(|c| matches!(c.cue_type, CueType::Hot))
            .map(ExtCuePointEntry::from_cue)
            .collect();

        let entry_count = entries.len() as u16;
        let entries_size: u32 = entries.iter().map(|e| e.size()).sum();
        let tag_size = HEADER_SIZE + entries_size;

        Self {
            len_header: HEADER_SIZE,
            len_tag: tag_size,
            list_type: CUE_LIST_TYPE_HOT,
            entry_count,
            unknown: 0,
            entries,
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
    fn test_empty_ext_cue_list() {
        let tag = ExtCueListTag::empty(0);
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(&data[0..4], b"PCO2");
        assert_eq!(data.len(), HEADER_SIZE as usize);
    }

    #[test]
    fn test_pioneer_color_mapping() {
        assert_eq!(PioneerColor::from_name("red") as u8, 2);
        assert_eq!(PioneerColor::from_name("BLUE") as u8, 7);
        assert_eq!(PioneerColor::from_name("cyan") as u8, 6);
        assert_eq!(PioneerColor::from_name("unknown") as u8, 0);
    }
}
