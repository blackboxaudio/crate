//! PCOB (Cue List) tag - cue points for memory and hot cues
//!
//! Contains a list of cue points with PCPT (Cue Point) entries.
//!
//! Reference: pyrekordbox and Deep Symmetry analysis

#![allow(dead_code)]

use binrw::{BinRead, BinWrite};

use crate::models::{Cue, CueType};

/// PCOB tag header size (12 bytes standard + 12 bytes extra)
const HEADER_SIZE: u32 = 24;

/// PCPT cue entry size in bytes
const CUE_ENTRY_SIZE: u32 = 56;

/// PCPT header size (includes magic through unknown1)
const PCPT_HEADER_SIZE: u32 = 0x1C; // 28 bytes

/// Cue list type for memory cues
pub const CUE_LIST_TYPE_MEMORY: u32 = 0;

/// Cue list type for hot cues
pub const CUE_LIST_TYPE_HOT: u32 = 1;

/// Cue point status - enabled
pub const CUE_STATUS_ENABLED: u32 = 4;

/// Cue point status - disabled
pub const CUE_STATUS_DISABLED: u32 = 0;

/// Constant value u2 in PCPT entries (always 1000 = 0x03E8)
const PCPT_CONST_U2: u16 = 1000;

/// A single cue point entry (PCPT)
///
/// Format from pyrekordbox/Deep Symmetry:
/// - Bytes 0-3: "PCPT" magic
/// - Bytes 4-7: len_header (0x1C = 28)
/// - Bytes 8-11: len_entry (0x38 = 56)
/// - Bytes 12-15: hot_cue (0 for memory)
/// - Bytes 16-19: status (0=disabled, 4=enabled)
/// - Bytes 20-23: unknown1 (0x00010000)
/// - Bytes 24-25: order_first
/// - Bytes 26-27: order_last
/// - Byte 28: cue_type (1=single, 2=loop)
/// - Byte 29: padding
/// - Bytes 30-31: const_u2 (1000 = 0x03E8)
/// - Bytes 32-35: time_ms
/// - Bytes 36-39: loop_time_ms (-1 if not loop)
/// - Bytes 40-55: padding (16 bytes)
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PCPT")]
pub struct CuePointEntry {
    /// Header size (always 0x1C = 28)
    pub len_header: u32,
    /// Total entry size (always 56)
    pub len_entry: u32,
    /// Hot cue index (0-7 for hot cues, 0 for memory cues)
    pub hot_cue: u32,
    /// Status flags (0=disabled, 4=enabled/active loop)
    pub status: u32,
    /// Unknown field (always 0x00010000)
    pub unknown1: u32,
    /// Order index within cue list
    pub order_first: u16,
    /// Order index within cue list (duplicate)
    pub order_last: u16,
    /// Cue type (1 = regular cue, 2 = loop)
    pub cue_type: u8,
    /// Padding (1 byte)
    pub padding1: u8,
    /// Constant field (always 1000 = 0x03E8)
    pub const_u2: u16,
    /// Time position in milliseconds
    pub time_ms: u32,
    /// Loop end time in milliseconds (-1 if not a loop)
    pub loop_time_ms: i32,
    /// Padding to reach 56 bytes total
    pub padding2: [u8; 16],
}

impl CuePointEntry {
    /// Size of each cue entry in bytes
    pub const SIZE: u32 = CUE_ENTRY_SIZE;

    /// Create a new cue point entry from a Cue
    ///
    /// # Arguments
    /// * `cue` - The cue to convert
    /// * `index` - The order index within the cue list
    pub fn from_cue(cue: &Cue, index: u16) -> Self {
        let (hot_cue, cue_type, loop_time_ms, status) = match cue.cue_type {
            CueType::Memory => (0, 1, -1, CUE_STATUS_ENABLED),
            CueType::Hot => (cue.hot_cue_index.unwrap_or(0) as u32, 1, -1, CUE_STATUS_ENABLED),
            CueType::Loop => (0, 2, cue.loop_end_ms.unwrap_or(-1) as i32, CUE_STATUS_ENABLED),
        };

        Self {
            len_header: PCPT_HEADER_SIZE,
            len_entry: CUE_ENTRY_SIZE,
            hot_cue,
            status,
            unknown1: 0x00010000,
            order_first: index,
            order_last: index,
            cue_type,
            padding1: 0,
            const_u2: PCPT_CONST_U2,
            time_ms: cue.position_ms as u32,
            loop_time_ms,
            padding2: [0; 16],
        }
    }
}

/// PCOB (Cue List) tag
///
/// Contains a list of cue points (memory cues, hot cues, or loops).
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"PCOB")]
pub struct CueListTag {
    /// Header size (always 24)
    pub len_header: u32,
    /// Total tag size including entries
    pub len_tag: u32,
    /// Cue list type (0 = memory/loop, 1 = hot cue)
    pub list_type: u32,
    /// Number of cue entries
    pub entry_count: u16,
    /// Unknown field (always 0)
    pub unknown: u16,
    /// Memory cue marker (0xFFFFFFFF)
    pub memory_marker: u32,
    /// Cue point entries
    #[br(count = entry_count)]
    pub entries: Vec<CuePointEntry>,
}

impl CueListTag {
    /// Create an empty cue list
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
            memory_marker: 0xFFFFFFFF,
            entries: vec![],
        }
    }

    /// Create a cue list for memory cues (including loops)
    ///
    /// # Arguments
    /// * `cues` - All cues for the track (will filter to memory and loop cues)
    pub fn memory_cues(cues: &[Cue]) -> Self {
        let filtered: Vec<&Cue> = cues
            .iter()
            .filter(|c| matches!(c.cue_type, CueType::Memory | CueType::Loop))
            .collect();

        let entries: Vec<CuePointEntry> = filtered
            .iter()
            .enumerate()
            .map(|(i, cue)| CuePointEntry::from_cue(cue, i as u16))
            .collect();

        let entry_count = entries.len() as u16;
        let tag_size = HEADER_SIZE + (entry_count as u32 * CUE_ENTRY_SIZE);

        Self {
            len_header: HEADER_SIZE,
            len_tag: tag_size,
            list_type: CUE_LIST_TYPE_MEMORY,
            entry_count,
            unknown: 0,
            memory_marker: 0xFFFFFFFF,
            entries,
        }
    }

    /// Create a cue list for hot cues
    ///
    /// # Arguments
    /// * `cues` - All cues for the track (will filter to hot cues)
    pub fn hot_cues(cues: &[Cue]) -> Self {
        let filtered: Vec<&Cue> = cues
            .iter()
            .filter(|c| matches!(c.cue_type, CueType::Hot))
            .collect();

        let entries: Vec<CuePointEntry> = filtered
            .iter()
            .enumerate()
            .map(|(i, cue)| CuePointEntry::from_cue(cue, i as u16))
            .collect();

        let entry_count = entries.len() as u16;
        let tag_size = HEADER_SIZE + (entry_count as u32 * CUE_ENTRY_SIZE);

        Self {
            len_header: HEADER_SIZE,
            len_tag: tag_size,
            list_type: CUE_LIST_TYPE_HOT,
            entry_count,
            unknown: 0,
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
    fn test_empty_cue_list_size() {
        let tag = CueListTag::empty(0);
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(data.len(), HEADER_SIZE as usize);
    }

    #[test]
    fn test_cue_list_magic() {
        let tag = CueListTag::empty(0);
        let mut buf = Cursor::new(Vec::new());
        tag.write(&mut buf).unwrap();
        let data = buf.into_inner();
        assert_eq!(&data[0..4], b"PCOB");
    }

    #[test]
    fn test_cue_entry_from_memory_cue() {
        let cue = Cue::new_memory("track1".to_string(), 5000);
        let entry = CuePointEntry::from_cue(&cue, 0);
        assert_eq!(entry.time_ms, 5000);
        assert_eq!(entry.hot_cue, 0);
        assert_eq!(entry.cue_type, 1);
        assert_eq!(entry.loop_time_ms, -1);
    }

    #[test]
    fn test_cue_entry_from_hot_cue() {
        let cue = Cue::new_hot("track1".to_string(), 10000, 3);
        let entry = CuePointEntry::from_cue(&cue, 1);
        assert_eq!(entry.time_ms, 10000);
        assert_eq!(entry.hot_cue, 3);
        assert_eq!(entry.cue_type, 1);
    }

    #[test]
    fn test_cue_entry_from_loop() {
        let cue = Cue::new_loop("track1".to_string(), 5000, 9000);
        let entry = CuePointEntry::from_cue(&cue, 2);
        assert_eq!(entry.time_ms, 5000);
        assert_eq!(entry.cue_type, 2);
        assert_eq!(entry.loop_time_ms, 9000);
    }
}
