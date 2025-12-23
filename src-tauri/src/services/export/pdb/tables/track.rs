//! Track row building for PDB files
//!
//! Track rows are the most complex row type, with a 92-byte fixed header
//! followed by 22 string fields.

use crate::services::export::pdb::strings::DeviceSQLString;

/// Internal track data for PDB generation
#[derive(Debug, Clone)]
pub struct PdbTrack {
    pub id: u32,
    pub title: String,
    pub artist_id: u32,
    pub album_id: u32,
    pub genre_id: u32,
    pub key_id: u32,
    pub color_id: u8,
    pub duration_seconds: u16,
    pub tempo: u32, // BPM * 100
    pub bitrate: u32,
    pub sample_rate: u32,
    pub file_size: u32,
    pub file_path: String,
    pub filename: String,
    pub rating: u8,
    pub year: u16,
    pub file_type: u16,
    pub date_added: String,
    pub comment: String,
    pub anlz_path: String,
}

/// Track row header constants
const TRACK_HEADER_SIZE: u16 = 0x5C; // 92 bytes
const TRACK_STRING_COUNT: usize = 22;
const TRACK_OFFSET_ARRAY_SIZE: u16 = (TRACK_STRING_COUNT * 2) as u16; // 44 bytes

/// Build a track row (most complex row type)
///
/// Format:
/// - 0x00-0x5B: Fixed header (92 bytes)
/// - 0x5C-0x87: String offset array (22 u16 = 44 bytes)
/// - 0x88+: String data
pub fn build_track_row(track: &PdbTrack, row_index: u16) -> Vec<u8> {
    let mut row = Vec::new();

    // Fixed header (0x5C = 92 bytes)
    // 0x00-0x01: Subtype (0x24 = U16 offsets)
    row.extend_from_slice(&0x0024u16.to_le_bytes());
    // 0x02-0x03: Index shift (row_index * 0x20)
    row.extend_from_slice(&row_index.wrapping_mul(0x20).to_le_bytes());
    // 0x04-0x07: Bitmask (0x000c0700)
    row.extend_from_slice(&0x000c0700u32.to_le_bytes());
    // 0x08-0x0B: Sample rate
    row.extend_from_slice(&track.sample_rate.to_le_bytes());
    // 0x0C-0x0F: Composer ID (0 = none)
    row.extend_from_slice(&0u32.to_le_bytes());
    // 0x10-0x13: File size
    row.extend_from_slice(&track.file_size.to_le_bytes());
    // 0x14-0x17: Unknown2
    row.extend_from_slice(&0u32.to_le_bytes());
    // 0x18-0x1B: Unknown3/4 (2x u16)
    row.extend_from_slice(&0u32.to_le_bytes());
    // 0x1C-0x1F: Artwork ID (0 = none)
    row.extend_from_slice(&0u32.to_le_bytes());
    // 0x20-0x23: Key ID
    row.extend_from_slice(&track.key_id.to_le_bytes());
    // 0x24-0x27: Original artist ID (0 = none)
    row.extend_from_slice(&0u32.to_le_bytes());
    // 0x28-0x2B: Label ID (0 = none)
    row.extend_from_slice(&0u32.to_le_bytes());
    // 0x2C-0x2F: Remixer ID (0 = none)
    row.extend_from_slice(&0u32.to_le_bytes());
    // 0x30-0x33: Bitrate
    row.extend_from_slice(&track.bitrate.to_le_bytes());
    // 0x34-0x37: Track number (0)
    row.extend_from_slice(&0u32.to_le_bytes());
    // 0x38-0x3B: Tempo (BPM * 100)
    row.extend_from_slice(&track.tempo.to_le_bytes());
    // 0x3C-0x3F: Genre ID
    row.extend_from_slice(&track.genre_id.to_le_bytes());
    // 0x40-0x43: Album ID
    row.extend_from_slice(&track.album_id.to_le_bytes());
    // 0x44-0x47: Artist ID
    row.extend_from_slice(&track.artist_id.to_le_bytes());
    // 0x48-0x4B: Track ID
    row.extend_from_slice(&track.id.to_le_bytes());
    // 0x4C-0x4D: Disc number
    row.extend_from_slice(&0u16.to_le_bytes());
    // 0x4E-0x4F: Play count
    row.extend_from_slice(&0u16.to_le_bytes());
    // 0x50-0x51: Year
    row.extend_from_slice(&track.year.to_le_bytes());
    // 0x52-0x53: Sample depth (16 or 24)
    row.extend_from_slice(&16u16.to_le_bytes());
    // 0x54-0x55: Duration (seconds)
    row.extend_from_slice(&track.duration_seconds.to_le_bytes());
    // 0x56-0x57: Unknown5 (0x0029)
    row.extend_from_slice(&0x0029u16.to_le_bytes());
    // 0x58: Color ID
    row.push(track.color_id);
    // 0x59: Rating (0-5)
    row.push(track.rating);
    // 0x5A-0x5B: File type
    row.extend_from_slice(&track.file_type.to_le_bytes());

    // Build the 22 strings
    let strings: [DeviceSQLString; TRACK_STRING_COUNT] = [
        DeviceSQLString::empty(),                // 0: ISRC (placeholder)
        DeviceSQLString::empty(),                // 1: Lyricist
        DeviceSQLString::empty(),                // 2: Unknown
        DeviceSQLString::empty(),                // 3: Unknown
        DeviceSQLString::empty(),                // 4: Unknown
        DeviceSQLString::empty(),                // 5: Message
        DeviceSQLString::empty(),                // 6: Publish info
        DeviceSQLString::empty(),                // 7: Autoload hotcues
        DeviceSQLString::empty(),                // 8: Unknown
        DeviceSQLString::empty(),                // 9: Unknown
        DeviceSQLString::new(&track.date_added), // 10: Date added
        DeviceSQLString::empty(),                // 11: Release date
        DeviceSQLString::empty(),                // 12: Mix name
        DeviceSQLString::empty(),                // 13: Unknown
        DeviceSQLString::new(&track.anlz_path),  // 14: Analyze path
        DeviceSQLString::empty(),                // 15: Analyze date
        DeviceSQLString::new(&track.comment),    // 16: Comment
        DeviceSQLString::new(&track.title),      // 17: Title
        DeviceSQLString::empty(),                // 18: Unknown
        DeviceSQLString::new(&track.filename),   // 19: Filename
        DeviceSQLString::new(&track.file_path),  // 20: File path
        DeviceSQLString::empty(),                // 21: Extra (padding)
    ];

    // Calculate string offsets
    // String data starts after header + offset array
    let string_data_start: u16 = TRACK_HEADER_SIZE + TRACK_OFFSET_ARRAY_SIZE;

    let mut offsets: Vec<u16> = Vec::with_capacity(TRACK_STRING_COUNT);
    let mut current_pos = string_data_start;

    // First offset is always 3 (convention)
    offsets.push(3);

    // Calculate remaining offsets
    for (i, s) in strings.iter().enumerate() {
        if i > 0 {
            offsets.push(current_pos);
        }
        current_pos += s.binary_size() as u16;
    }

    // Write the 22 u16 offsets (44 bytes)
    for offset in &offsets {
        row.extend_from_slice(&offset.to_le_bytes());
    }

    // Write the strings
    let mut string_buf = Vec::new();
    for s in &strings {
        s.write_to(&mut string_buf).expect("write to Vec should not fail");
    }
    row.extend_from_slice(&string_buf);

    row
}
