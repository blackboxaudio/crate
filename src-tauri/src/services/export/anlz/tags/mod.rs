//! ANLZ tag definitions
//!
//! All tags that can appear in ANLZ files (.DAT, .EXT, .2EX).

#![allow(dead_code)]

pub mod pco2;
pub mod pcob;
pub mod ppth;
pub mod pqt2;
pub mod pqtz;
pub mod pssi;
pub mod pvbr;
pub mod pwav;
pub mod pwv2;
pub mod pwv3;
pub mod pwv4;
pub mod pwv5;
pub mod pwv6;
pub mod pwv7;
pub mod pwvc;

pub use pco2::ExtCueListTag;
pub use pcob::CueListTag;
pub use ppth::PathTag;
pub use pqt2::ExtBeatGridTag;
pub use pqtz::BeatGridTag;
pub use pssi::SongStructureTag;
pub use pvbr::VbrTag;
pub use pwav::WaveformPreviewTag;
pub use pwv2::TinyWaveformTag;
pub use pwv3::WaveformDetailTag;
pub use pwv4::WaveformColorPreviewTag;
pub use pwv5::WaveformColorDetailTag;
pub use pwv6::ExtWaveform1Tag;
pub use pwv7::ExtWaveform2Tag;
pub use pwvc::ExtWaveformColorTag;

use binrw::BinWrite;
use std::io::{Read, Seek, Write};

/// All possible ANLZ tag types
///
/// Tags are dispatched based on their 4-byte magic identifier.
#[derive(Debug, Clone)]
pub enum AnlzTag {
    /// PPTH - Path to audio file
    Path(PathTag),
    /// PVBR - VBR seek index
    Vbr(VbrTag),
    /// PQTZ - Beat grid
    BeatGrid(BeatGridTag),
    /// PWAV - Waveform preview
    WaveformPreview(WaveformPreviewTag),
    /// PWV2 - Tiny waveform preview
    TinyWaveform(TinyWaveformTag),
    /// PCOB - Cue list
    CueList(CueListTag),
    /// PQT2 - Extended beat grid
    ExtBeatGrid(ExtBeatGridTag),
    /// PCO2 - Extended cue list
    ExtCueList(ExtCueListTag),
    /// PWV3 - Waveform detail
    WaveformDetail(WaveformDetailTag),
    /// PWV4 - Waveform color preview
    WaveformColorPreview(WaveformColorPreviewTag),
    /// PWV5 - Waveform color detail
    WaveformColorDetail(WaveformColorDetailTag),
    /// PSSI - Song structure
    SongStructure(SongStructureTag),
    /// PWV6 - Extended waveform 1
    ExtWaveform1(ExtWaveform1Tag),
    /// PWV7 - Extended waveform 2
    ExtWaveform2(ExtWaveform2Tag),
    /// PWVC - Extended waveform color
    ExtWaveformColor(ExtWaveformColorTag),
}

impl AnlzTag {
    /// Get the size of this tag in bytes
    pub fn size(&self) -> u32 {
        match self {
            AnlzTag::Path(t) => t.size(),
            AnlzTag::Vbr(t) => t.size(),
            AnlzTag::BeatGrid(t) => t.size(),
            AnlzTag::WaveformPreview(t) => t.size(),
            AnlzTag::TinyWaveform(t) => t.size(),
            AnlzTag::CueList(t) => t.size(),
            AnlzTag::ExtBeatGrid(t) => t.size(),
            AnlzTag::ExtCueList(t) => t.size(),
            AnlzTag::WaveformDetail(t) => t.size(),
            AnlzTag::WaveformColorPreview(t) => t.size(),
            AnlzTag::WaveformColorDetail(t) => t.size(),
            AnlzTag::SongStructure(t) => t.size(),
            AnlzTag::ExtWaveform1(t) => t.size(),
            AnlzTag::ExtWaveform2(t) => t.size(),
            AnlzTag::ExtWaveformColor(t) => t.size(),
        }
    }

    /// Write this tag to a writer
    pub fn write_to<W: Write + Seek>(&self, writer: &mut W) -> Result<(), binrw::Error> {
        match self {
            AnlzTag::Path(t) => t.write(writer),
            AnlzTag::Vbr(t) => t.write(writer),
            AnlzTag::BeatGrid(t) => t.write(writer),
            AnlzTag::WaveformPreview(t) => t.write(writer),
            AnlzTag::TinyWaveform(t) => t.write(writer),
            AnlzTag::CueList(t) => t.write(writer),
            AnlzTag::ExtBeatGrid(t) => t.write(writer),
            AnlzTag::ExtCueList(t) => t.write(writer),
            AnlzTag::WaveformDetail(t) => t.write(writer),
            AnlzTag::WaveformColorPreview(t) => t.write(writer),
            AnlzTag::WaveformColorDetail(t) => t.write(writer),
            AnlzTag::SongStructure(t) => t.write(writer),
            AnlzTag::ExtWaveform1(t) => t.write(writer),
            AnlzTag::ExtWaveform2(t) => t.write(writer),
            AnlzTag::ExtWaveformColor(t) => t.write(writer),
        }
    }
}

/// Read a tag from a reader based on magic bytes
///
/// Peeks at the magic bytes and dispatches to the appropriate parser.
pub fn read_tag<R: Read + Seek>(reader: &mut R) -> Result<AnlzTag, binrw::Error> {
    use binrw::BinReaderExt;

    // Read magic bytes
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;

    // Seek back to start of tag
    reader.seek(std::io::SeekFrom::Current(-4))?;

    // Dispatch based on magic
    let tag = match &magic {
        b"PPTH" => AnlzTag::Path(reader.read_be()?),
        b"PVBR" => AnlzTag::Vbr(reader.read_be()?),
        b"PQTZ" => AnlzTag::BeatGrid(reader.read_be()?),
        b"PWAV" => AnlzTag::WaveformPreview(reader.read_be()?),
        b"PWV2" => AnlzTag::TinyWaveform(reader.read_be()?),
        b"PCOB" => AnlzTag::CueList(reader.read_be()?),
        b"PQT2" => AnlzTag::ExtBeatGrid(reader.read_be()?),
        b"PCO2" => AnlzTag::ExtCueList(reader.read_be()?),
        b"PWV3" => AnlzTag::WaveformDetail(reader.read_be()?),
        b"PWV4" => AnlzTag::WaveformColorPreview(reader.read_be()?),
        b"PWV5" => AnlzTag::WaveformColorDetail(reader.read_be()?),
        b"PSSI" => AnlzTag::SongStructure(reader.read_be()?),
        b"PWV6" => AnlzTag::ExtWaveform1(reader.read_be()?),
        b"PWV7" => AnlzTag::ExtWaveform2(reader.read_be()?),
        b"PWVC" => AnlzTag::ExtWaveformColor(reader.read_be()?),
        _ => {
            return Err(binrw::Error::BadMagic {
                pos: reader.stream_position()?,
                found: Box::new(magic),
            });
        }
    };

    Ok(tag)
}
