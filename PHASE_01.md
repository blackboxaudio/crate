# Milestone 1: ANLZ Module Rewrite Implementation Plan

## Overview
Rewrite the ANLZ module from manual byte manipulation to binrw-based binary serialization, adding support for all file variants (.DAT, .EXT, .2EX), real cue export, and proper module structure.

## Current State
- Single file `src-tauri/src/services/export/anlz.rs` (~240 lines)
- Manual byte manipulation, 6 basic tags (PMAI, PPTH, PVBR, PQTZ, PWAV, PCOB empty)
- `binrw = "0.14.1"` already in Cargo.toml but unused
- Only generates `.DAT` files
- Cues exist in database but not exported

## Target State
- Modular `anlz/` directory with binrw-based structs
- Support for 14 tags across .DAT, .EXT, .2EX variants
- Real cue export from database (memory, hot, loop)
- Placeholder waveforms (actual analysis in later milestone)

---

## Implementation Tasks

### Task 1: Create Module Structure
Create the `anlz/` directory structure:

```
src-tauri/src/services/export/anlz/
  mod.rs           # Public API, AnlzFile, AnlzFileBuilder
  error.rs         # AnlzError enum with thiserror
  header.rs        # PMAI file header (28 bytes)
  crypto.rs        # XOR encryption for PSSI
  utils.rs         # UTF-16-BE encoding, path generation, AnlzVariant enum
  tags/
    mod.rs         # AnlzTag enum with binrw dispatch
    ppth.rs        # Path tag
    pvbr.rs        # VBR seek index (400 entries)
    pqtz.rs        # Beat grid (.DAT)
    pqt2.rs        # Extended beat grid (.EXT)
    pcob.rs        # Cue list + PCPT entries
    pco2.rs        # Extended cue list + PCP2 entries
    pwav.rs        # Waveform preview
    pwv2.rs        # Tiny waveform preview
    pwv3.rs        # Waveform detail (.EXT)
    pwv4.rs        # Waveform color preview (.EXT)
    pwv5.rs        # Waveform color detail (.EXT)
    pwv6.rs        # Extended waveform (.2EX)
    pwv7.rs        # Extended waveform (.2EX)
    pwvc.rs        # Extended waveform color (.2EX)
    pssi.rs        # Song structure with XOR encryption
```

### Task 2: Core Infrastructure Files

**error.rs** - Custom error type:
- `AnlzError::Io`, `AnlzError::Parse`, `AnlzError::InvalidMagic`, etc.
- Implement `From<AnlzError> for CrateError`

**utils.rs** - Utilities:
- `to_utf16_be(s: &str) -> Vec<u8>` - UTF-16-BE encoding
- `generate_anlz_dir(track_id: u32) -> String` - `/PIONEER/USBANLZ/Pxxx/xxxxxxxx/`
- `generate_anlz_path(track_id: u32, variant: AnlzVariant) -> String`
- `enum AnlzVariant { Dat, Ext, Ext2 }`

**crypto.rs** - XOR encryption for PSSI:
```rust
const XOR_MASK: [u8; 19] = [0xCB, 0xE1, 0xEE, 0xFA, 0xE5, 0xEE, 0xAD, 0xEE, 0xE9,
                            0xD2, 0xE9, 0xEB, 0xE1, 0xE9, 0xF3, 0xE8, 0xE9, 0xF4, 0xE1];
// mask_byte = (XOR_MASK[i % 19] + len_entries) & 0xFF
```

**header.rs** - PMAI file header (28 bytes):
```rust
#[derive(BinRead, BinWrite)]
#[brw(big, magic = b"PMAI")]
pub struct AnlzFileHeader {
    pub len_header: u32,  // Always 28
    pub len_file: u32,
    pub unknown1: u32,    // 0x00000001
    pub unknown2: u32,    // 0x00010000
    pub unknown3: u32,    // 0x00010000
    pub unknown4: u32,    // 0x00000000
}
```

### Task 3: Essential .DAT Tags

**tags/mod.rs** - Tag enum with magic dispatch:
```rust
#[derive(BinRead, BinWrite)]
#[brw(big)]
pub enum AnlzTag {
    #[brw(magic = b"PPTH")] Path(PathTag),
    #[brw(magic = b"PVBR")] Vbr(VbrTag),
    // ... all 14 tags
}
```

**ppth.rs** - Path tag (UTF-16-BE encoded device path)
**pvbr.rs** - VBR seek index (400 u32 entries, zeroed for placeholder)
**pqtz.rs** - Beat grid with `BeatGridEntry { beat: u16, tempo: u16, time_ms: u32 }`
**pwav.rs** - Waveform preview (400 bytes, placeholder 0x44)
**pwv2.rs** - Tiny waveform preview (100 bytes)
**pcob.rs** - Cue list with PCPT entries:
- `CueListTag::memory_cues(cues: &[Cue])` - Filter memory/loop cues
- `CueListTag::hot_cues(cues: &[Cue])` - Filter hot cues
- `CuePointEntry` (56 bytes) with time_ms, loop_time_ms, hot_cue index

### Task 4: .EXT Tags

**pqt2.rs** - Extended beat grid (same format as PQTZ)
**pco2.rs** - Extended cue list with PCP2 entries:
- Adds `color_id`, `loop_numerator/denominator`, `comment` (UTF-16-BE)
- Map color names to Pioneer color IDs (1-8)
**pwv3.rs** - Waveform detail (high-resolution)
**pwv4.rs** - Waveform color preview (6 bytes/entry: RGB + luminance)
**pwv5.rs** - Waveform color detail (2 bytes/entry: packed RGB + height)
**pssi.rs** - Song structure with XOR encryption (empty placeholder)

### Task 5: .2EX Tags

**pwv6.rs** - Extended waveform (3 bytes/entry)
**pwv7.rs** - Extended waveform with unknown constant
**pwvc.rs** - Extended waveform color (minimal, 14-byte header)

### Task 6: Public API (mod.rs)

**AnlzFile struct**:
```rust
pub struct AnlzFile {
    pub header: AnlzFileHeader,
    pub tags: Vec<AnlzTag>,
}

impl AnlzFile {
    pub fn parse(data: &[u8]) -> Result<Self>;
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self>;
    pub fn to_bytes(&self) -> Result<Vec<u8>>;
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()>;
}
```

**AnlzFileBuilder**:
```rust
pub struct AnlzFileBuilder {
    variant: AnlzVariant,
    path: Option<String>,
    bpm: Option<f32>,
    duration_ms: Option<u32>,
    cues: Vec<Cue>,
}

impl AnlzFileBuilder {
    pub fn new(variant: AnlzVariant) -> Self;
    pub fn path(self, path: impl Into<String>) -> Self;
    pub fn bpm(self, bpm: f32) -> Self;
    pub fn duration_ms(self, duration: u32) -> Self;
    pub fn cues(self, cues: Vec<Cue>) -> Self;
    pub fn build(self) -> Result<AnlzFile>;
}
```

### Task 7: ExportService Integration

Update `src-tauri/src/services/export/mod.rs`:

1. **Add cue fetching method** (new):
```rust
fn get_track_cues(&self, track_id: &str) -> Result<Vec<Cue>> {
    // Query cues table for track_id, ordered by position_ms
}
```

2. **Update generate_anlz_file signature** (line 518):
```rust
fn generate_anlz_file(
    &self,
    mount_point: &str,
    pdb_track_id: u32,
    usb_audio_path: &str,
    duration_ms: u32,
    bpm: Option<f32>,
    track_id: &str,  // NEW: for cue lookup
) -> Result<String>
```

3. **Update generate_anlz_file body** to:
   - Fetch cues: `let cues = self.get_track_cues(track_id)?;`
   - Generate all 3 variants (.DAT, .EXT, .2EX)
   - Use `AnlzFileBuilder` for each variant

4. **Update call site** (line 483):
```rust
let anlz_path = self.generate_anlz_file(
    mount_point,
    next_pdb_id,
    usb_path,
    track.duration_ms as u32,
    track.bpm.map(|b| b as f32),
    &track.id,  // NEW: pass track_id
)?;
```

### Task 8: Delete Old Implementation

Remove `src-tauri/src/services/export/anlz.rs` after the new module is complete and tested.

---

## Files to Create
| File | Description |
|------|-------------|
| `anlz/mod.rs` | Public API, AnlzFile, AnlzFileBuilder |
| `anlz/error.rs` | AnlzError enum |
| `anlz/header.rs` | PMAI file header |
| `anlz/crypto.rs` | XOR encryption |
| `anlz/utils.rs` | UTF-16-BE, path generation |
| `anlz/tags/mod.rs` | AnlzTag enum |
| `anlz/tags/ppth.rs` | Path tag |
| `anlz/tags/pvbr.rs` | VBR index |
| `anlz/tags/pqtz.rs` | Beat grid |
| `anlz/tags/pqt2.rs` | Extended beat grid |
| `anlz/tags/pcob.rs` | Cue list |
| `anlz/tags/pco2.rs` | Extended cue list |
| `anlz/tags/pwav.rs` | Waveform preview |
| `anlz/tags/pwv2.rs` | Tiny waveform |
| `anlz/tags/pwv3.rs` | Waveform detail |
| `anlz/tags/pwv4.rs` | Waveform color preview |
| `anlz/tags/pwv5.rs` | Waveform color detail |
| `anlz/tags/pwv6.rs` | Extended waveform 1 |
| `anlz/tags/pwv7.rs` | Extended waveform 2 |
| `anlz/tags/pwvc.rs` | Extended waveform color |
| `anlz/tags/pssi.rs` | Song structure |

## Files to Modify
| File | Changes |
|------|---------|
| `src-tauri/src/services/export/mod.rs` | Add cue fetching, update generate_anlz_file, generate all variants |

## Files to Delete
| File | Reason |
|------|--------|
| `src-tauri/src/services/export/anlz.rs` | Replaced by anlz/ module |

---

## Tag Distribution by File Type

| Tag | .DAT | .EXT | .2EX |
|-----|------|------|------|
| PPTH | Yes | Yes | Yes |
| PVBR | Yes | Yes | Yes |
| PQTZ | Yes | Yes | Yes |
| PWAV | Yes | Yes | Yes |
| PWV2 | Yes | Yes | Yes |
| PCOB (x2) | Yes | Yes | Yes |
| PQT2 | No | Yes | Yes |
| PCO2 (x2) | No | Yes | Yes |
| PWV3 | No | Yes | Yes |
| PWV4 | No | Yes | Yes |
| PWV5 | No | Yes | Yes |
| PSSI | No | Yes | Yes |
| PWV6 | No | No | Yes |
| PWV7 | No | No | Yes |
| PWVC | No | No | Yes |

---

## Implementation Order

1. Create directory structure and `error.rs`, `utils.rs`, `crypto.rs`
2. Implement `header.rs` (PMAI)
3. Implement essential .DAT tags in order: PPTH, PVBR, PQTZ, PWAV, PWV2, PCOB
4. Implement `tags/mod.rs` with enum dispatch
5. Implement .EXT tags: PQT2, PCO2, PWV3, PWV4, PWV5, PSSI
6. Implement .2EX tags: PWV6, PWV7, PWVC
7. Implement `AnlzFile` and `AnlzFileBuilder` in `mod.rs`
8. Update `ExportService` integration (add cue fetching, update signature)
9. Test with existing export workflow
10. Delete old `anlz.rs`

---

## Key Technical Notes

- **All ANLZ is big-endian** (`#[brw(big)]`)
- **PSSI encryption**: Only bytes after offset 18 are XOR'd
- **Beat grid calculation**: `num_beats = duration_ms / (60000 / bpm)`, capped at 10,000
- **Placeholder waveforms**: Use 0x44 (height=8, whiteness=4 encoded)
- **Cue colors**: Map to Pioneer IDs 1-8 (pink, red, orange, yellow, green, aqua, blue, purple)
