# PDB & ANLZ Export Rewrite Plan

Complete rewrite of the export module using `pyrekordbox` as reference, implementing with `binrw` for binary formats.

## Scope Summary

| Feature | Current State | Target State |
|---------|--------------|--------------|
| ANLZ Tags | 6 basic tags (PPTH, PVBR, PQTZ, PWAV, PCOB×2) | All 14 tags including color waveforms, PSSI |
| PDB Format | Manual byte manipulation (2234 lines) | binrw derive macros, cleaner code |
| Device Library Plus | Not supported | Full SQLCipher SQLite support |
| PDB Merging | Not implemented | Read existing → merge → write |
| Track Copying | Sequential | Parallel with configurable concurrency |
| Resume/Checkpoint | Not supported | Checkpoint-based resume |

## Reference Materials

- **pyrekordbox**: `../pyrekordbox/`
  - `anlz/structs.py` - ANLZ binary format definitions
  - `anlz/file.py` - AnlzFile class, XOR decryption
  - `devicelib_plus/models.py` - SQLite schema for Device Library Plus

---

## Milestone 1: ANLZ Module Rewrite

### New Module Structure
```
src-tauri/src/services/export/
  anlz/
    mod.rs           # Public API, AnlzFile struct, AnlzFileBuilder
    header.rs        # PMAI file header (28 bytes)
    error.rs         # ANLZ-specific errors
    crypto.rs        # XOR encryption/decryption for PSSI
    utils.rs         # UTF-16-BE encoding, path generation
    tags/
      mod.rs         # AnlzTag enum with binrw dispatch
      ppth.rs        # Path tag (UTF-16-BE)
      pvbr.rs        # VBR seek index (400 entries)
      pqtz.rs        # Beat grid (.DAT)
      pqt2.rs        # Extended beat grid (.EXT)
      pcob.rs        # Cue list + PCPT entries
      pco2.rs        # Extended cue list + PCP2 entries
      pwav.rs        # Waveform preview (400 bytes)
      pwv2.rs        # Tiny waveform preview
      pwv3.rs        # Waveform detail (.EXT)
      pwv4.rs        # Waveform color preview (.EXT)
      pwv5.rs        # Waveform color detail (.EXT)
      pwv6.rs        # Extended waveform (.2EX)
      pwv7.rs        # Extended waveform (.2EX)
      pwvc.rs        # Extended waveform color (.2EX)
      pssi.rs        # Song structure with XOR encryption
```

### Key Structs (binrw)

```rust
// All ANLZ is big-endian
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

#[derive(BinRead, BinWrite)]
#[brw(big, magic = b"PQTZ")]
pub struct BeatGridTag {
    pub len_header: u32,  // 24
    pub len_tag: u32,
    pub unknown1: u32,    // 0
    pub unknown2: u32,    // 0x80000
    pub entry_count: u32,
    #[br(count = entry_count)]
    pub entries: Vec<BeatGridEntry>,
}

#[derive(BinRead, BinWrite)]
#[brw(big)]
pub struct BeatGridEntry {
    pub beat: u16,        // 1-4
    pub tempo: u16,       // BPM × 100
    pub time_ms: u32,
}
```

### XOR Encryption (PSSI tags in RB6+)
```rust
const XOR_MASK: [u8; 19] = [
    0xCB, 0xE1, 0xEE, 0xFA, 0xE5, 0xEE, 0xAD, 0xEE, 0xE9,
    0xD2, 0xE9, 0xEB, 0xE1, 0xE9, 0xF3, 0xE8, 0xE9, 0xF4, 0xE1,
];

pub fn xor_decrypt(data: &mut [u8], len_entries: u16) {
    for (i, byte) in data.iter_mut().enumerate() {
        let mask = XOR_MASK[i % 19].wrapping_add(len_entries as u8);
        *byte ^= mask;
    }
}
```

### Tasks
1. Create `anlz/` directory structure
2. Implement `AnlzFileHeader` with binrw
3. Implement tag enum with magic-based dispatch
4. Implement essential .DAT tags: PPTH, PVBR, PQTZ, PWAV, PWV2, PCOB
5. Implement .EXT tags: PQT2, PCO2, PWV3, PWV4, PWV5, PSSI (with XOR)
6. Implement .2EX tags: PWV6, PWV7, PWVC
7. Implement `AnlzFileBuilder` for creating new files
8. Implement `AnlzFile::parse()` for reading existing files
9. Update `ExportService` to use new module
10. Add cue point export from library data

### Files to Modify
- `src-tauri/src/services/export/anlz.rs` → Replace with `anlz/` module
- `src-tauri/src/services/export/mod.rs` → Update imports, integration

---

## Milestone 2: PDB Module Rewrite with binrw

### New Module Structure
```
src-tauri/src/services/export/
  pdb/
    mod.rs           # Public API, PdbWriter trait
    types.rs         # Page structures with binrw
    strings.rs       # DeviceSQLString (3 encodings)
    tables/
      mod.rs         # Table type enum
      track.rs       # Track row (92-byte header + 22 strings)
      artist.rs      # Artist row
      album.rs       # Album row
      genre.rs       # Genre/Key/Label/Color rows
      playlist.rs    # PlaylistTree + PlaylistEntry rows
      menu.rs        # Menu + Column rows
    page.rs          # PageBuilder with binrw
    writer.rs        # PdbWriter implementation
    reader.rs        # PdbReader for merging (Milestone 4)
```

### Key Structs (binrw)

```rust
// PDB is little-endian
pub const PAGE_SIZE: u32 = 4096;

#[derive(BinRead, BinWrite)]
#[brw(little)]
pub struct PageHeader {
    pub magic: u32,           // 0
    pub page_index: u32,
    pub page_type: u32,       // TableType
    pub next_page: u32,
    pub sequence: u32,
    pub unknown2: u32,        // 0
    pub row_counts: [u8; 3],  // Packed
    pub page_flags: u8,       // 0x64=index, 0x24=data
    pub free_size: u16,
    pub used_size: u16,
}

#[derive(BinRead, BinWrite)]
#[brw(little)]
pub struct TrackRowHeader {
    pub subtype: u16,         // 0x0024
    pub index_shift: u16,
    pub bitmask: u32,         // 0x000C0700
    pub sample_rate: u32,
    // ... 92 bytes total
}
```

### DeviceSQLString
```rust
pub enum DeviceSQLString {
    ShortAscii(Vec<u8>),   // ≤126 bytes: header = ((len+1)<<1)|1
    LongAscii(Vec<u8>),    // >126 bytes: 0x40 + u16 len
    LongUtf16(Vec<u16>),   // Non-ASCII: 0x90 + u16 len + UTF-16LE
}
```

### Tasks
1. Create `pdb/` directory structure
2. Implement page types with binrw
3. Implement `DeviceSQLString` with custom BinRead/BinWrite
4. Implement all row types (Track, Artist, Album, Genre, Key, Color, Playlist, Menu, Column)
5. Implement `PageBuilder` for constructing pages
6. Implement `PdbWriter` using binrw for serialization
7. Migrate from manual byte manipulation to binrw structs
8. Implement `exportExt.pdb` writer for tags
9. Update `ExportService` integration

### Files to Modify
- `src-tauri/src/services/export/rekordbox.rs` → Replace with `pdb/` module
- `src-tauri/src/services/export/mod.rs` → Update integration

---

## Milestone 3: Device Library Plus (SQLite + SQLCipher)

### New Module Structure
```
src-tauri/src/services/export/
  device_library_plus/
    mod.rs           # Public API
    schema.rs        # CREATE TABLE statements
    models.rs        # Rust structs for tables
    encryption.rs    # SQLCipher key derivation
    database.rs      # Database operations
```

### Schema (Key Tables)
```sql
CREATE TABLE content (
    content_id INTEGER PRIMARY KEY,
    title VARCHAR(255),
    bpmx100 INTEGER,
    length INTEGER,
    artist_id_artist INTEGER REFERENCES artist(artist_id),
    album_id INTEGER REFERENCES album(album_id),
    genre_id INTEGER REFERENCES genre(genre_id),
    key_id INTEGER REFERENCES key(key_id),
    color_id INTEGER REFERENCES color(color_id),
    path VARCHAR(255) UNIQUE NOT NULL,
    analysisDataFilePath VARCHAR(255),
    -- ... 40+ columns total
);

CREATE TABLE cue (
    cue_id INTEGER PRIMARY KEY,
    content_id INTEGER REFERENCES content(content_id),
    kind INTEGER,
    inUsec INTEGER,
    outUsec INTEGER,
    -- ... position in multiple formats
);

CREATE TABLE playlist (
    playlist_id INTEGER PRIMARY KEY,
    name VARCHAR(255),
    playlist_id_parent INTEGER REFERENCES playlist(playlist_id),
);
```

### SQLCipher Key Derivation
The key is obfuscated in pyrekordbox. Need to either:
1. Port the deobfuscation logic (base85 decode → XOR → zlib decompress)
2. Hard-code the derived key (it's a fixed, known value)

### Dependencies
```toml
# Cargo.toml additions
rusqlite = { version = "0.32", features = ["bundled-sqlcipher"] }
```

### Tasks
1. Add `rusqlite` with `bundled-sqlcipher` feature to Cargo.toml
2. Implement SQLCipher key handling
3. Create schema.rs with all CREATE TABLE statements
4. Implement models.rs with Rust structs
5. Implement database.rs with CRUD operations
6. Add format detection (legacy PDB vs Device Library Plus)
7. Create `ExportDatabase` trait for shared interface
8. Update `ExportService` to support both formats

### Files to Create
- `src-tauri/src/services/export/device_library_plus/` (new module)

### Files to Modify
- `src-tauri/Cargo.toml` → Add SQLCipher dependency
- `src-tauri/src/services/export/mod.rs` → Format detection, trait usage

---

## Milestone 4: PDB Merging & Sync

### PDB Reader
```rust
pub struct PdbReader {
    file_header: FileHeader,
    existing_tracks: Vec<ParsedTrack>,
    existing_playlists: Vec<ParsedPlaylist>,
    next_track_id: u32,
    next_playlist_id: u32,
}

impl PdbReader {
    pub fn from_bytes(data: &[u8]) -> Result<Self>;
    pub fn find_track_by_path(&self, path: &str) -> Option<&ParsedTrack>;
    pub fn allocate_track_id(&mut self) -> u32;
}
```

### Sync Diff Calculator
```rust
pub struct SyncDiff {
    pub tracks_to_add: Vec<Track>,
    pub tracks_to_update: Vec<(Track, DeviceTrack)>,
    pub tracks_to_remove: Vec<DeviceTrack>,
    pub playlists_changed: Vec<Playlist>,
}
```

### Checkpoint System
```sql
-- New table
CREATE TABLE export_checkpoints (
    id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,
    state_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Add to device_tracks
ALTER TABLE device_tracks ADD COLUMN anlz_hash TEXT;
ALTER TABLE device_tracks ADD COLUMN last_verified_at TEXT;
```

### Tasks
1. Implement `PdbReader` using binrw for parsing
2. Implement `PdbMerger` for combining existing + new data
3. Implement `SyncDiffCalculator`
4. Add track removal strategies (remove orphaned, conservative, never remove)
5. Implement checkpoint save/restore
6. Add parallel track copying with configurable concurrency
7. Add resume command for failed exports
8. Update frontend for sync preview UI

### Files to Create
- `src-tauri/src/services/export/pdb/reader.rs`
- `src-tauri/src/services/export/pdb/merger.rs`
- `src-tauri/src/services/export/sync/` (new module)
- `src-tauri/src/services/export/checkpoint.rs`

### Files to Modify
- `src-tauri/src/db/schema.rs` → New tables/columns
- `src-tauri/src/commands/export.rs` → New commands (resume, sync preview)
- `src-tauri/src/services/export/mod.rs` → Integrate sync logic

---

## Implementation Order

```
Milestone 1: ANLZ Rewrite
├── Week 1: Core infrastructure + essential .DAT tags
├── Week 2: .EXT tags + PSSI encryption
└── Week 3: .2EX tags + integration + testing

Milestone 2: PDB with binrw
├── Week 4: Types, strings, page structures
├── Week 5: Row types + PageBuilder
└── Week 6: Writer + migration from manual code

Milestone 3: Device Library Plus
├── Week 7: SQLCipher setup + schema
└── Week 8: Database operations + format detection

Milestone 4: Merging & Sync
├── Week 9: PDB reader + merger
├── Week 10: Sync diff + removal strategies
└── Week 11: Checkpoints + parallel copying + resume
```

---

## Critical Files Summary

| File | Action | Milestone |
|------|--------|-----------|
| `src-tauri/src/services/export/anlz.rs` | Replace with `anlz/` module | 1 |
| `src-tauri/src/services/export/rekordbox.rs` | Replace with `pdb/` module | 2 |
| `src-tauri/src/services/export/mod.rs` | Refactor orchestration | 1-4 |
| `src-tauri/Cargo.toml` | Add bundled-sqlcipher | 3 |
| `src-tauri/src/db/schema.rs` | Add checkpoint tables | 4 |
| `src-tauri/src/commands/export.rs` | Add resume/sync commands | 4 |
| `src-tauri/src/models/export.rs` | Extend with new types | 1-4 |

## Testing Strategy

1. **Round-trip tests**: Parse pyrekordbox output → write with our code → compare bytes
2. **Device testing**: Export to USB, test on CDJ-3000/XDJ-XZ
3. **Merge tests**: Create PDB → add tracks → verify merge preserves existing
4. **Sync tests**: Add/update/remove scenarios
