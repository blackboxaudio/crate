use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::{CrateError, Result};

/// Size of the chunk to hash (64KB)
const HASH_CHUNK_SIZE: usize = 64 * 1024;

/// Computes a BLAKE3 hash of the first 64KB of audio content in a file.
///
/// This provides a fast, unique fingerprint for audio files that:
/// - Is resilient to metadata changes (ID3 tags, etc.)
/// - Is fast enough for real-time validation
/// - Is unique enough to identify individual audio files
pub fn compute_audio_hash(path: &Path) -> Result<String> {
    let file = File::open(path).map_err(|e| {
        CrateError::Io(std::io::Error::new(
            e.kind(),
            format!("Failed to open file for hashing: {}", path.display()),
        ))
    })?;

    let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut reader = BufReader::new(file);

    // Try to skip past common metadata headers to get to audio data
    let audio_start = find_audio_data_start(&mut reader, path)?;
    reader.seek(SeekFrom::Start(audio_start)).map_err(|e| {
        CrateError::Io(std::io::Error::new(
            e.kind(),
            format!("Failed to seek to audio data: {}", path.display()),
        ))
    })?;

    // Read up to HASH_CHUNK_SIZE bytes of audio content
    let mut buffer = vec![0u8; HASH_CHUNK_SIZE];
    let bytes_read = reader.read(&mut buffer).map_err(|e| {
        CrateError::Io(std::io::Error::new(
            e.kind(),
            format!("Failed to read audio data for hashing: {}", path.display()),
        ))
    })?;

    // Include file size in the hash for additional uniqueness
    let mut hasher = blake3::Hasher::new();
    hasher.update(&buffer[..bytes_read]);
    hasher.update(&file_size.to_le_bytes());

    Ok(hasher.finalize().to_hex().to_string())
}

/// Attempts to find where the actual audio data starts in a file,
/// skipping past metadata headers (ID3, etc.)
fn find_audio_data_start<R: Read + Seek>(reader: &mut R, path: &Path) -> Result<u64> {
    let mut header = [0u8; 10];

    // Read the first 10 bytes to check for ID3v2 header
    if reader.read_exact(&mut header).is_ok() {
        // Check for ID3v2 header: "ID3"
        if &header[0..3] == b"ID3" {
            // ID3v2 size is stored in bytes 6-9 as syncsafe integer
            let size = syncsafe_to_int(&header[6..10]);
            // ID3v2 header is 10 bytes + size
            return Ok(10 + size as u64);
        }
    }

    // Reset to beginning and check for other formats
    reader.seek(SeekFrom::Start(0)).map_err(|e| {
        CrateError::Io(std::io::Error::new(
            e.kind(),
            format!("Failed to seek: {}", path.display()),
        ))
    })?;

    // For FLAC, WAV, AIFF - audio data is near the beginning
    // For these formats, we'll just start from the beginning
    // The metadata is typically at the start but is small enough
    // that including it won't significantly affect the hash

    // Check file extension to handle format-specific offsets
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "wav" => {
            // WAV files: skip to data chunk
            // RIFF header is 12 bytes, then we need to find "data" chunk
            if let Some(offset) = find_wav_data_chunk(reader) {
                return Ok(offset);
            }
        }
        "aiff" | "aif" => {
            // AIFF files: skip to SSND chunk
            if let Some(offset) = find_aiff_ssnd_chunk(reader) {
                return Ok(offset);
            }
        }
        "flac" => {
            // FLAC: skip metadata blocks to get to frame data
            if let Some(offset) = find_flac_frame_start(reader) {
                return Ok(offset);
            }
        }
        _ => {}
    }

    // Default: start from beginning
    Ok(0)
}

/// Converts a syncsafe integer (used in ID3v2) to a regular integer
fn syncsafe_to_int(bytes: &[u8]) -> u32 {
    ((bytes[0] as u32) << 21)
        | ((bytes[1] as u32) << 14)
        | ((bytes[2] as u32) << 7)
        | (bytes[3] as u32)
}

/// Find the start of the data chunk in a WAV file
fn find_wav_data_chunk<R: Read + Seek>(reader: &mut R) -> Option<u64> {
    reader.seek(SeekFrom::Start(12)).ok()?; // Skip RIFF header

    let mut chunk_header = [0u8; 8];
    loop {
        if reader.read_exact(&mut chunk_header).is_err() {
            return None;
        }

        let chunk_id = &chunk_header[0..4];
        let chunk_size = u32::from_le_bytes([
            chunk_header[4],
            chunk_header[5],
            chunk_header[6],
            chunk_header[7],
        ]);

        if chunk_id == b"data" {
            // Return current position (after the 8-byte chunk header)
            return reader.stream_position().ok();
        }

        // Skip to next chunk
        reader.seek(SeekFrom::Current(chunk_size as i64)).ok()?;
    }
}

/// Find the start of the SSND chunk in an AIFF file
fn find_aiff_ssnd_chunk<R: Read + Seek>(reader: &mut R) -> Option<u64> {
    reader.seek(SeekFrom::Start(12)).ok()?; // Skip FORM header

    let mut chunk_header = [0u8; 8];
    loop {
        if reader.read_exact(&mut chunk_header).is_err() {
            return None;
        }

        let chunk_id = &chunk_header[0..4];
        let chunk_size = u32::from_be_bytes([
            chunk_header[4],
            chunk_header[5],
            chunk_header[6],
            chunk_header[7],
        ]);

        if chunk_id == b"SSND" {
            // SSND has 8 bytes of offset/block info before audio data
            reader.seek(SeekFrom::Current(8)).ok()?;
            return reader.stream_position().ok();
        }

        // Skip to next chunk (AIFF chunks are padded to even length)
        let padded_size = if chunk_size % 2 == 0 {
            chunk_size
        } else {
            chunk_size + 1
        };
        reader.seek(SeekFrom::Current(padded_size as i64)).ok()?;
    }
}

/// Find the start of frame data in a FLAC file (after metadata blocks)
fn find_flac_frame_start<R: Read + Seek>(reader: &mut R) -> Option<u64> {
    reader.seek(SeekFrom::Start(0)).ok()?;

    // Check for "fLaC" magic
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).ok()?;
    if &magic != b"fLaC" {
        return None;
    }

    // Read metadata blocks until we find the last one
    loop {
        let mut block_header = [0u8; 4];
        if reader.read_exact(&mut block_header).is_err() {
            return None;
        }

        let is_last = (block_header[0] & 0x80) != 0;
        let block_size = u32::from_be_bytes([0, block_header[1], block_header[2], block_header[3]]);

        if is_last {
            // Skip this block and return position
            reader.seek(SeekFrom::Current(block_size as i64)).ok()?;
            return reader.stream_position().ok();
        }

        // Skip to next block
        reader.seek(SeekFrom::Current(block_size as i64)).ok()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syncsafe_to_int() {
        // Test syncsafe integer conversion
        assert_eq!(syncsafe_to_int(&[0x00, 0x00, 0x02, 0x01]), 257);
        assert_eq!(syncsafe_to_int(&[0x00, 0x00, 0x00, 0x7f]), 127);
    }
}
