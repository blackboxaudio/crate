//! ANLZ file writer for Pioneer CDJ/XDJ equipment
//!
//! Generates minimal .DAT analysis files required for tracks to appear on Pioneer gear.
//! Based on the Deep Symmetry analysis: https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html
//!
//! ANLZ files use **big-endian** byte order (opposite of PDB files).

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::error::Result;

/// Fixed waveform preview width (400 columns for CDJ display)
const WAVEFORM_PREVIEW_WIDTH: usize = 400;

/// Generate a unique ANLZ directory path based on track ID
///
/// Format: /PIONEER/USBANLZ/Pxxx/xxxxxxxx/
/// - Pxxx: First 3 hex chars of ID
/// - xxxxxxxx: Full 8-char hex ID
pub fn generate_anlz_dir(track_id: u32) -> String {
    let hex_id = format!("{:08X}", track_id);
    let prefix = &hex_id[0..3];
    format!("/PIONEER/USBANLZ/P{}/{}", prefix, hex_id)
}

/// Generate the full ANLZ file path
pub fn generate_anlz_path(track_id: u32) -> String {
    format!("{}/ANLZ0000.DAT", generate_anlz_dir(track_id))
}

/// Write a complete ANLZ file for a track
///
/// The file contains:
/// - PMAI header (file section)
/// - PPTH section (file path)
/// - PVBR section (VBR seek index - 400 entries)
/// - PQTZ section (beat grid with tempo)
/// - PWAV section (waveform preview)
/// - PCOB section (empty cue list for memory cues)
/// - PCOB section (empty cue list for hot cues)
pub fn write_anlz_file(
    output_path: &Path,
    device_audio_path: &str,
    duration_ms: u32,
    bpm: Option<f32>,
) -> Result<()> {
    let file = File::create(output_path)?;
    let mut writer = BufWriter::new(file);

    // Build sections first to calculate sizes
    let path_section = build_path_section(device_audio_path);
    let vbr_section = build_vbr_section();
    let beat_grid_section = build_beat_grid_section(bpm.unwrap_or(120.0), duration_ms);
    let waveform_section = build_waveform_preview_section();
    let memory_cue_section = build_cue_section(0); // Memory cues (type 0)
    let hot_cue_section = build_cue_section(1); // Hot cues (type 1)

    // Calculate total content size (all sections combined)
    let content_size = path_section.len()
        + vbr_section.len()
        + beat_grid_section.len()
        + waveform_section.len()
        + memory_cue_section.len()
        + hot_cue_section.len();

    // Write PMAI header
    write_file_header(&mut writer, content_size as u32)?;

    // Write sections in order matching rekordbox
    writer.write_all(&path_section)?;
    writer.write_all(&vbr_section)?;
    writer.write_all(&beat_grid_section)?;
    writer.write_all(&waveform_section)?;
    writer.write_all(&memory_cue_section)?;
    writer.write_all(&hot_cue_section)?;

    writer.flush()?;
    Ok(())
}

/// Write the PMAI file header (big-endian)
fn write_file_header<W: Write>(writer: &mut W, content_size: u32) -> Result<()> {
    // Magic: "PMAI"
    writer.write_all(b"PMAI")?;

    // Header size: 28 bytes (12 for standard header + 16 padding)
    writer.write_all(&28u32.to_be_bytes())?;

    // Total size: header + content
    let total_size = 28 + content_size;
    writer.write_all(&total_size.to_be_bytes())?;

    // Header data (16 bytes) - must have specific flags for XDJ compatibility
    // Known-good rekordbox files have: 00 00 00 01 | 00 01 00 00 | 00 01 00 00 | 00 00 00 00
    writer.write_all(&[0x00, 0x00, 0x00, 0x01])?; // Unknown1 flag
    writer.write_all(&[0x00, 0x01, 0x00, 0x00])?; // Unknown2 flag
    writer.write_all(&[0x00, 0x01, 0x00, 0x00])?; // Unknown3 flag
    writer.write_all(&[0x00, 0x00, 0x00, 0x00])?; // Padding

    Ok(())
}

/// Build the PPTH (path) section
fn build_path_section(path: &str) -> Vec<u8> {
    let mut section = Vec::new();

    // Convert path to UTF-16BE with null terminator
    let utf16_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let path_bytes: Vec<u8> = utf16_path
        .iter()
        .flat_map(|&c| c.to_be_bytes())
        .collect();

    let path_len = path_bytes.len() as u32;
    let header_size: u32 = 16; // 12 standard + 4 for path length
    let total_size = header_size + path_len;

    // Magic: "PPTH"
    section.extend_from_slice(b"PPTH");
    // Header size
    section.extend_from_slice(&header_size.to_be_bytes());
    // Total size
    section.extend_from_slice(&total_size.to_be_bytes());
    // Path length
    section.extend_from_slice(&path_len.to_be_bytes());
    // Path data (UTF-16BE)
    section.extend_from_slice(&path_bytes);

    section
}

/// Number of seek index entries in PVBR section (per Kaitai spec)
const VBR_INDEX_ENTRIES: usize = 400;

/// Build the PVBR (VBR seek index) section
///
/// Per Kaitai Struct spec: "Stores an index allowing rapid seeking to particular
/// times within a variable-bitrate audio file."
/// Contains 400 u32 index values for VBR seeking support.
fn build_vbr_section() -> Vec<u8> {
    let mut section = Vec::new();

    let header_size: u32 = 16; // 12 standard + 4 for unknown field
    let data_size: u32 = (VBR_INDEX_ENTRIES * 4) as u32; // 400 × 4 = 1600 bytes
    let total_size: u32 = header_size + data_size;

    // Magic: "PVBR"
    section.extend_from_slice(b"PVBR");
    // Header size
    section.extend_from_slice(&header_size.to_be_bytes());
    // Total size
    section.extend_from_slice(&total_size.to_be_bytes());
    // Unknown field (always 0)
    section.extend_from_slice(&0u32.to_be_bytes());

    // Write 400 u32 seek index entries (zeros = linear/constant bitrate seeking)
    for _ in 0..VBR_INDEX_ENTRIES {
        section.extend_from_slice(&0u32.to_be_bytes());
    }

    section
}

/// Build the PWAV (waveform preview) section
///
/// Creates a flat waveform (all zeros) - sufficient for track visibility
fn build_waveform_preview_section() -> Vec<u8> {
    let mut section = Vec::new();

    let header_size: u32 = 20; // 12 standard + 4 len_preview + 4 unknown
    let waveform_data_len = WAVEFORM_PREVIEW_WIDTH as u32;
    let total_size = header_size + waveform_data_len;

    // Magic: "PWAV"
    section.extend_from_slice(b"PWAV");
    // Header size
    section.extend_from_slice(&header_size.to_be_bytes());
    // Total size
    section.extend_from_slice(&total_size.to_be_bytes());
    // len_preview
    section.extend_from_slice(&waveform_data_len.to_be_bytes());
    // Unknown field (apparently always 0x00100000)
    section.extend_from_slice(&0x00100000u32.to_be_bytes());
    // Waveform data: 400 bytes, each byte encodes height (5 bits) + whiteness (3 bits)
    // Use a minimal flat waveform (height=8, whiteness=4 = 0x44)
    section.extend(std::iter::repeat(0x44u8).take(WAVEFORM_PREVIEW_WIDTH));

    section
}

/// Build the PQTZ (beat grid) section
///
/// Per Kaitai Struct spec: Contains beat entries with time position,
/// beat number (1-4), and tempo (BPM × 100).
fn build_beat_grid_section(bpm: f32, duration_ms: u32) -> Vec<u8> {
    let mut section = Vec::new();

    // Calculate beat timing
    let tempo = (bpm * 100.0) as u16; // BPM × 100
    let beat_duration_ms = 60000.0 / bpm; // ms per beat
    let num_beats = ((duration_ms as f32) / beat_duration_ms) as u32;

    // Limit to reasonable number of beats (max ~10000 for very long tracks)
    let num_beats = num_beats.min(10000);

    let header_size: u32 = 24; // PQTZ has 24-byte header
    let beat_entry_size: u32 = 8; // Each beat entry is 8 bytes
    let data_size: u32 = num_beats * beat_entry_size;
    let total_size: u32 = header_size + data_size;

    // Magic: "PQTZ"
    section.extend_from_slice(b"PQTZ");
    // Header size
    section.extend_from_slice(&header_size.to_be_bytes());
    // Total size
    section.extend_from_slice(&total_size.to_be_bytes());
    // Unknown1 (always 0)
    section.extend_from_slice(&0u32.to_be_bytes());
    // Unknown2 (seems to be 0 or small number)
    section.extend_from_slice(&0u16.to_be_bytes());
    // Number of beats
    section.extend_from_slice(&(num_beats as u16).to_be_bytes());

    // Write beat entries
    let mut time_ms: f32 = 0.0;
    for i in 0..num_beats {
        // Time in milliseconds (u16)
        section.extend_from_slice(&(time_ms as u16).to_be_bytes());
        // Beat number (1-4 cycling)
        let beat_num = ((i % 4) + 1) as u16;
        section.extend_from_slice(&beat_num.to_be_bytes());
        // Tempo (BPM × 100)
        section.extend_from_slice(&tempo.to_be_bytes());
        // Padding (always 0)
        section.extend_from_slice(&0u16.to_be_bytes());

        time_ms += beat_duration_ms;
    }

    section
}

/// Build a PCOB (cue list) section
///
/// Creates an empty cue section. Type 0 = memory cues, type 1 = hot cues.
fn build_cue_section(cue_type: u8) -> Vec<u8> {
    let mut section = Vec::new();

    let header_size: u32 = 24; // PCOB has 24-byte header
    let total_size: u32 = header_size; // Empty, no cue entries

    // Magic: "PCOB"
    section.extend_from_slice(b"PCOB");
    // Header size
    section.extend_from_slice(&header_size.to_be_bytes());
    // Total size (same as header for empty section)
    section.extend_from_slice(&total_size.to_be_bytes());
    // Cue list type (0 = memory cues, 1 = hot cues)
    section.extend_from_slice(&(cue_type as u32).to_be_bytes());
    // Entry count (0 = no cues)
    section.extend_from_slice(&0u16.to_be_bytes());
    // Unknown/padding
    section.extend_from_slice(&0u16.to_be_bytes());
    // Memory cue marker (0xFFFFFFFF)
    section.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes());

    section
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_anlz_path() {
        assert_eq!(
            generate_anlz_path(1),
            "/PIONEER/USBANLZ/P000/00000001/ANLZ0000.DAT"
        );
        assert_eq!(
            generate_anlz_path(0x12345678),
            "/PIONEER/USBANLZ/P123/12345678/ANLZ0000.DAT"
        );
    }

    #[test]
    fn test_build_path_section() {
        let section = build_path_section("/test/path.mp3");
        // Should start with "PPTH"
        assert_eq!(&section[0..4], b"PPTH");
    }
}
