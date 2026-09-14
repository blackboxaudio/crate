//! Header patching for fragmented MP4 audio so AVFoundation reports the right duration.
//!
//! YouTube's `m4a_dash` streams (itag 140) are fragmented MP4: an init `moov` with no samples,
//! then a `sidx` and a run of `moof`/`mdat` fragments. YouTube stamps the full duration into
//! `mvhd`, `tkhd` and `mdhd` anyway. FFmpeg and Chromium ignore that for fragmented files, but
//! AVFoundation adds the header duration to the fragments' duration, so `AVPlayerItem.duration`
//! comes out at exactly twice the track length and scrubbing past the midpoint plays silence.
//! Zeroing those three fields makes AVFoundation fall back to the fragment index and report
//! the true length (verified against macOS AVFoundation, which shares the demuxer with iOS).
//!
//! Chromium's parser reports an unknown duration for a fragmented file whose `mvhd` is zero
//! and which has no `mehd`, so the patch is only applied where AVFoundation is the consumer.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::error::Result;

/// Boxes whose children are walked looking for the duration-carrying headers.
const CONTAINER_BOXES: &[&[u8; 4]] = &[b"moov", b"trak", b"mdia"];

/// Upper bound on a `moov` we are willing to read into memory; a DASH init segment is ~1 KB.
const MAX_MOOV_SIZE: u64 = 16 * 1024 * 1024;

/// Patch a freshly cached stream for AVFoundation when it is MP4 audio. Errors are logged, not
/// propagated: an unpatched file still plays, just with the doubled duration.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub fn neutralize_for_avfoundation(path: &Path, content_type: &str) {
    if !(content_type.starts_with("audio/mp4") || content_type.starts_with("video/mp4")) {
        return;
    }
    match neutralize_fragmented_durations(path) {
        Ok(true) => log::info!("Zeroed fragmented-MP4 header durations for AVFoundation"),
        Ok(false) => {}
        Err(e) => log::warn!("Could not patch fragmented-MP4 header durations: {e}"),
    }
}

/// Zero the `mvhd`/`tkhd`/`mdhd` durations of a fragmented MP4 file in place.
///
/// Returns `Ok(false)` without touching the file when it is not a fragmented MP4 (no `moov`
/// followed by `sidx`/`moof`): a plain MP4 keeps its samples in the movie box, and zeroing its
/// durations would break it.
pub fn neutralize_fragmented_durations(path: &Path) -> Result<bool> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;

    let Some((moov_offset, moov_size, header_len)) = locate_fragmented_moov(&mut file)? else {
        return Ok(false);
    };

    let mut moov = vec![0u8; moov_size as usize];
    file.seek(SeekFrom::Start(moov_offset))?;
    file.read_exact(&mut moov)?;

    if patch_container(&mut moov[header_len..]) == 0 {
        return Ok(false);
    }

    file.seek(SeekFrom::Start(moov_offset))?;
    file.write_all(&moov)?;
    file.flush()?;
    Ok(true)
}

/// Walk the top-level boxes and return `(offset, size, header_len)` of the `moov` when the box
/// after it is a fragment index or fragment; `None` for anything else.
fn locate_fragmented_moov<F: Read + Seek>(file: &mut F) -> Result<Option<(u64, u64, usize)>> {
    let mut offset: u64 = 0;
    let mut moov: Option<(u64, u64, usize)> = None;
    loop {
        let mut header = [0u8; 16];
        file.seek(SeekFrom::Start(offset))?;
        let filled = read_up_to(file, &mut header)?;
        let Some(b) = parse_box_header(&header[..filled]) else {
            return Ok(None);
        };
        // A zero size means "to end of file"; the boxes we care about never use it.
        if b.size == 0 || b.size < b.header_len as u64 {
            return Ok(None);
        }
        match &b.kind {
            b"moov" if moov.is_none() => {
                if b.size > MAX_MOOV_SIZE {
                    return Ok(None);
                }
                moov = Some((offset, b.size, b.header_len));
            }
            b"sidx" | b"moof" if moov.is_some() => return Ok(moov),
            b"mdat" => return Ok(None),
            _ => {}
        }
        offset += b.size;
    }
}

fn read_up_to<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = reader.read(&mut buf[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    Ok(filled)
}

struct BoxHeader {
    size: u64,
    kind: [u8; 4],
    header_len: usize,
}

fn parse_box_header(buf: &[u8]) -> Option<BoxHeader> {
    if buf.len() < 8 {
        return None;
    }
    let size32 = u32::from_be_bytes(buf[0..4].try_into().ok()?);
    let kind: [u8; 4] = buf[4..8].try_into().ok()?;
    if size32 == 1 {
        if buf.len() < 16 {
            return None;
        }
        let size = u64::from_be_bytes(buf[8..16].try_into().ok()?);
        return Some(BoxHeader {
            size,
            kind,
            header_len: 16,
        });
    }
    Some(BoxHeader {
        size: size32 as u64,
        kind,
        header_len: 8,
    })
}

/// Zero every duration header inside a container's child region; returns how many were zeroed.
fn patch_container(children: &mut [u8]) -> usize {
    let mut patched = 0;
    let mut i = 0;
    while i + 8 <= children.len() {
        let Some(b) = parse_box_header(&children[i..]) else {
            break;
        };
        let size = if b.size == 0 {
            children.len() - i
        } else {
            b.size as usize
        };
        if size < b.header_len || i + size > children.len() {
            break;
        }
        let (start, end) = (i + b.header_len, i + size);
        if CONTAINER_BOXES.contains(&&b.kind) {
            patched += patch_container(&mut children[start..end]);
        } else if zero_duration(&b.kind, &mut children[start..end]) {
            patched += 1;
        }
        i += size;
    }
    patched
}

/// Zero the duration field of an `mvhd`/`tkhd`/`mdhd` payload (the bytes after the box header,
/// starting with the version byte). Offsets follow ISO 14496-12 for versions 0 and 1.
fn zero_duration(kind: &[u8; 4], payload: &mut [u8]) -> bool {
    let (offset, width) = match (kind, payload.first()) {
        (b"mvhd" | b"mdhd", Some(0)) => (16, 4),
        (b"mvhd" | b"mdhd", Some(1)) => (24, 8),
        (b"tkhd", Some(0)) => (20, 4),
        (b"tkhd", Some(1)) => (28, 8),
        _ => return false,
    };
    if payload.len() < offset + width {
        return false;
    }
    payload[offset..offset + width].fill(0);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boxed(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(payload);
        out
    }

    fn full_box(kind: &[u8; 4], version: u8, body: &[u8]) -> Vec<u8> {
        let mut payload = vec![version, 0, 0, 0];
        payload.extend_from_slice(body);
        boxed(kind, &payload)
    }

    /// mvhd v0: ctime, mtime, timescale, duration, then padding.
    fn mvhd_v0(duration: u32) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&[0; 8]);
        body.extend_from_slice(&44100u32.to_be_bytes());
        body.extend_from_slice(&duration.to_be_bytes());
        body.extend_from_slice(&[0xAA; 80]);
        full_box(b"mvhd", 0, &body)
    }

    /// tkhd v0: ctime, mtime, track_id, reserved, duration, then padding.
    fn tkhd_v0(duration: u32) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&[0; 8]);
        body.extend_from_slice(&1u32.to_be_bytes());
        body.extend_from_slice(&[0; 4]);
        body.extend_from_slice(&duration.to_be_bytes());
        body.extend_from_slice(&[0xBB; 60]);
        full_box(b"tkhd", 0, &body)
    }

    /// mdhd v1: ctime (8), mtime (8), timescale, duration (8), then language/padding.
    fn mdhd_v1(duration: u64) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&[0; 16]);
        body.extend_from_slice(&44100u32.to_be_bytes());
        body.extend_from_slice(&duration.to_be_bytes());
        body.extend_from_slice(&[0xCC; 4]);
        full_box(b"mdhd", 1, &body)
    }

    fn moov(duration: u32) -> Vec<u8> {
        let mdia = boxed(
            b"mdia",
            &[mdhd_v1(duration as u64), boxed(b"hdlr", &[0xDD; 24])].concat(),
        );
        let trak = boxed(b"trak", &[tkhd_v0(duration), mdia].concat());
        let mvex = boxed(b"mvex", &boxed(b"trex", &[0; 24]));
        boxed(b"moov", &[mvhd_v0(duration), mvex, trak].concat())
    }

    fn fragmented_file() -> Vec<u8> {
        [
            boxed(b"ftyp", b"dash\0\0\0\0iso6mp41"),
            moov(0x1122_3344),
            boxed(b"sidx", &[0; 32]),
            boxed(b"moof", &[0; 40]),
            boxed(b"mdat", &[1; 64]),
        ]
        .concat()
    }

    fn plain_file() -> Vec<u8> {
        [
            boxed(b"ftyp", b"M4A \0\0\0\0isom"),
            moov(12_277_760),
            boxed(b"mdat", &[1; 64]),
        ]
        .concat()
    }

    fn write_temp(bytes: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("crate-fmp4-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn durations(file: &[u8]) -> (u32, u32, u64) {
        let find = |kind: &[u8]| file.windows(4).position(|w| w == kind).unwrap();
        let mvhd = find(b"mvhd") + 4;
        let tkhd = find(b"tkhd") + 4;
        let mdhd = find(b"mdhd") + 4;
        (
            u32::from_be_bytes(file[mvhd + 16..mvhd + 20].try_into().unwrap()),
            u32::from_be_bytes(file[tkhd + 20..tkhd + 24].try_into().unwrap()),
            u64::from_be_bytes(file[mdhd + 24..mdhd + 32].try_into().unwrap()),
        )
    }

    #[test]
    fn zeroes_durations_of_fragmented_mp4() {
        let original = fragmented_file();
        assert_eq!(
            durations(&original),
            (0x1122_3344, 0x1122_3344, 0x1122_3344)
        );
        let path = write_temp(&original);

        assert!(neutralize_fragmented_durations(&path).unwrap());

        let patched = std::fs::read(&path).unwrap();
        assert_eq!(patched.len(), original.len());
        assert_eq!(durations(&patched), (0, 0, 0));
        // Everything outside the three duration fields is untouched: 4 + 4 bytes for the
        // 32-bit mvhd/tkhd fields, and the 4 non-zero low bytes of the 64-bit mdhd field.
        let diff: Vec<usize> = original
            .iter()
            .zip(patched.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(diff.len(), 4 + 4 + 4);
        // Idempotent.
        assert!(neutralize_fragmented_durations(&path).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), patched);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn leaves_plain_mp4_alone() {
        let original = plain_file();
        let path = write_temp(&original);
        assert!(!neutralize_fragmented_durations(&path).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn ignores_non_mp4_and_truncated_input() {
        for bytes in [
            b"ID3\x04\x00\x00\x00\x00\x00\x00\xff\xfb".to_vec(),
            vec![0u8; 3],
            Vec::new(),
        ] {
            let path = write_temp(&bytes);
            assert!(!neutralize_fragmented_durations(&path).unwrap());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            std::fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn handles_largesize_boxes_and_v1_headers() {
        // A 64-bit-sized ftyp ahead of the moov, and v1 mvhd inside it.
        let mut ftyp = 1u32.to_be_bytes().to_vec();
        ftyp.extend_from_slice(b"ftyp");
        ftyp.extend_from_slice(&(16u64 + 8).to_be_bytes());
        ftyp.extend_from_slice(b"dash\0\0\0\0");
        let mut mvhd_body = vec![0; 16];
        mvhd_body.extend_from_slice(&44100u32.to_be_bytes());
        mvhd_body.extend_from_slice(&99u64.to_be_bytes());
        mvhd_body.extend_from_slice(&[0; 80]);
        let moov = boxed(b"moov", &full_box(b"mvhd", 1, &mvhd_body));
        let file = [ftyp, moov, boxed(b"moof", &[0; 8])].concat();
        let path = write_temp(&file);
        assert!(neutralize_fragmented_durations(&path).unwrap());
        let patched = std::fs::read(&path).unwrap();
        let mvhd = patched.windows(4).position(|w| w == b"mvhd").unwrap() + 4;
        assert_eq!(
            u64::from_be_bytes(patched[mvhd + 24..mvhd + 32].try_into().unwrap()),
            0
        );
        std::fs::remove_file(path).unwrap();
    }
}
