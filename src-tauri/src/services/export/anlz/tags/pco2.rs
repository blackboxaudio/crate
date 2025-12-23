//! PCO2 (Extended Cue List) tag - cue points with colors and comments
//!
//! Contains extended cue point information (PCP2 entries) with color and comment support.

#![allow(dead_code)]

use binrw::{BinRead, BinWrite};

use crate::models::{Cue, CueType};
use crate::services::export::anlz::utils::to_utf16_be;

/// PCO2 tag header size (12 bytes standard + 14 bytes extra)
const HEADER_SIZE: u32 = 26;

/// PCP2 cue entry header size
const CUE_ENTRY_HEADER_SIZE: u32 = 22;

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
}

/// A single extended cue point entry (PCP2)
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PCP2")]
pub struct ExtCuePointEntry {
    /// Header size (always 22)
    pub len_header: u32,
    /// Total entry size (variable based on comment length)
    pub len_entry: u32,
    /// Hot cue index (0-7 for hot cues, 0 for memory cues)
    pub hot_cue: u32,
    /// Cue type (1 = regular cue, 2 = loop)
    pub cue_type: u8,
    /// Padding
    pub padding1: u8,
    /// Time position in milliseconds
    pub time_ms: u32,
    /// Loop end time in milliseconds (0 if not a loop)
    pub loop_time_ms: u32,
    /// Pioneer color ID (0-8)
    pub color_id: u8,
    /// Padding
    pub padding2: u8,
    /// Loop numerator (0 if not a loop)
    pub loop_numerator: u16,
    /// Loop denominator (0 if not a loop)
    pub loop_denominator: u16,
    /// Length of comment in bytes
    pub len_comment: u32,
    /// Comment text (UTF-16-BE encoded with null terminator)
    #[br(count = len_comment)]
    pub comment: Vec<u8>,
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

        let color_id = cue
            .color
            .as_ref()
            .map(|c| PioneerColor::from_name(c) as u8)
            .unwrap_or(0);

        // Encode comment (or empty string)
        let comment_text = cue.name.as_deref().unwrap_or("");
        let comment = if comment_text.is_empty() {
            vec![0, 0] // Just null terminator
        } else {
            to_utf16_be(comment_text)
        };
        let len_comment = comment.len() as u32;
        let len_entry = CUE_ENTRY_HEADER_SIZE + len_comment;

        Self {
            len_header: CUE_ENTRY_HEADER_SIZE,
            len_entry,
            hot_cue,
            cue_type,
            padding1: 0,
            time_ms: cue.position_ms as u32,
            loop_time_ms,
            color_id,
            padding2: 0,
            loop_numerator: loop_num,
            loop_denominator: loop_denom,
            len_comment,
            comment,
        }
    }

    /// Get the total size of this entry in bytes
    pub fn size(&self) -> u32 {
        self.len_entry
    }
}

/// PCO2 (Extended Cue List) tag
///
/// Contains extended cue points with colors and comments.
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PCO2")]
pub struct ExtCueListTag {
    /// Header size (always 26)
    pub len_header: u32,
    /// Total tag size including entries
    pub len_tag: u32,
    /// Cue list type (0 = memory/loop, 1 = hot cue)
    pub list_type: u32,
    /// Number of cue entries
    pub entry_count: u16,
    /// Unknown field (always 0)
    pub unknown1: u16,
    /// Unknown field (always 0)
    pub unknown2: u16,
    /// Memory cue marker (0xFFFFFFFF as bytes)
    pub memory_marker: u32,
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
            unknown1: 0,
            unknown2: 0,
            memory_marker: 0xFFFFFFFF,
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
            unknown1: 0,
            unknown2: 0,
            memory_marker: 0xFFFFFFFF,
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
            unknown1: 0,
            unknown2: 0,
            memory_marker: 0xFFFFFFFF,
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
