# Milestone 2: PDB Module Rewrite with binrw

Rewrite `src-tauri/src/services/export/rekordbox.rs` (2,234 lines) as a modular `pdb/` directory using binrw derive macros.

## Reference Implementation
- **Primary reference**: `../pyrekordbox/` - Python module that successfully writes PDB files
- The current `rekordbox.rs` does NOT work correctly; use pyrekordbox as source of truth
- Skip unit tests for now; focus on implementation matching pyrekordbox output

## Module Structure

```
src-tauri/src/services/export/pdb/
  mod.rs              # Public API, re-exports
  error.rs            # PdbError enum
  constants.rs        # All PDB constants (PAGE_SIZE, flags, markers)
  types.rs            # Core binrw types (PageHeader, IndexHeader)
  strings.rs          # DeviceSQLString (3 encodings)
  page.rs             # PageBuilder, RowGroup
  header.rs           # File header (page 0) with TableDescriptor
  index.rs            # Index page structures and builders
  writer.rs           # PdbWriter implementation (export.pdb)
  ext_writer.rs       # ExtPdbWriter (exportExt.pdb)
  tables/
    mod.rs            # TableType enum (20 types), ExtTableType
    track.rs          # TrackRow (92-byte header + 22 strings)
    artist.rs         # ArtistRow
    album.rs          # AlbumRow
    genre.rs          # GenreRow
    key.rs            # KeyRow
    color.rs          # ColorRow + STANDARD_COLORS
    playlist.rs       # PlaylistTreeRow, PlaylistEntryRow
    menu.rs           # MenuRow, ColumnRow + definitions
    tag.rs            # TagRow, TrackTagRow (for exportExt.pdb)
```

## Implementation Order

### Phase 1: Foundation
1. `pdb/error.rs` - PdbError enum with thiserror, From<binrw::Error>, From<PdbError> for CrateError
2. `pdb/constants.rs` - PAGE_SIZE (4096), flags, markers, offsets
3. `pdb/strings.rs` - DeviceSQLString with custom BinWrite (ShortAscii, LongAscii, LongUtf16)

### Phase 2: Types
4. `pdb/types.rs` - PageHeader, DataPageHeader, IndexPageHeader, TableDescriptor with binrw derives
5. `pdb/tables/mod.rs` - TableType enum (0-19), ExtTableType enum

### Phase 3: Row Types
6. `pdb/tables/track.rs` - TrackRowHeader (92 bytes), PdbTrack, build_track_row()
7. `pdb/tables/artist.rs` - build_artist_row()
8. `pdb/tables/album.rs` - build_album_row()
9. `pdb/tables/genre.rs` - build_genre_row()
10. `pdb/tables/key.rs` - build_key_row()
11. `pdb/tables/color.rs` - build_color_row(), STANDARD_COLORS
12. `pdb/tables/playlist.rs` - PdbPlaylistNode, PdbPlaylistEntry, build functions
13. `pdb/tables/menu.rs` - COLUMN_DEFS (27), STANDARD_MENUS (17), build functions
14. `pdb/tables/tag.rs` - PdbTag, PdbTrackTag, build functions

### Phase 4: Page Building
15. `pdb/page.rs` - RowGroup (16 offsets + presence flags), PageBuilder
16. `pdb/index.rs` - build_index_page(), build_empty_index_page()
17. `pdb/header.rs` - TableLayout, build_file_header()

### Phase 5: Writers
18. `pdb/writer.rs` - PdbWriter with add_track(), add_playlist(), write()
19. `pdb/ext_writer.rs` - ExtPdbWriter for tags

### Phase 6: Integration
20. `pdb/mod.rs` - Re-exports (RekordboxPdbWriter = PdbWriter for compatibility)
21. Update `export/mod.rs` imports
22. Delete `rekordbox.rs`

## Key Implementation Details

### DeviceSQLString Encoding
```rust
ShortAscii: header = ((len+1) << 1) | 1, then content
LongAscii:  0x40 + u16 len (includes 4-byte header) + 0x00 + content
LongUtf16:  0x90 + u16 len (char_count*2 + 4) + 0x00 + UTF-16LE content
```

### Page Structure (4096 bytes, little-endian)
```
0x00-0x1F: PageHeader (magic, page_index, page_type, next_page, sequence, packed_row_counts, flags, sizes)
0x20-0x27: DataPageHeader (unknown5, unknown_not, reserved)
0x28-...:  Heap data (rows stored sequentially)
...-0xFFF: Row groups at page end (written backwards)
```

### Row Groups
- Up to 16 row offsets per group
- presence_flags bitmask indicates which rows exist
- Written from page end backwards

### Packed Row Counts (3 bytes)
```rust
bits 0-12: num_row_groups (13 bits)
bits 13-23: num_rows (11 bits)
```

### Table-Specific Behavior
- Menu/Columns: unknown5 = num_rows, unknown_not = 0
- Other tables: unknown5 = num_rows, unknown_not = num_rows - 1

## Files Summary

### Create (20 files)
- `pdb/mod.rs`, `error.rs`, `constants.rs`, `types.rs`, `strings.rs`
- `pdb/page.rs`, `header.rs`, `index.rs`, `writer.rs`, `ext_writer.rs`
- `pdb/tables/mod.rs`, `track.rs`, `artist.rs`, `album.rs`, `genre.rs`
- `pdb/tables/key.rs`, `color.rs`, `playlist.rs`, `menu.rs`, `tag.rs`

### Modify (1 file)
- `src-tauri/src/services/export/mod.rs` - Update imports

### Delete (1 file)
- `src-tauri/src/services/export/rekordbox.rs`

## Public API (unchanged)
```rust
impl PdbWriter {
    pub fn new() -> Self;
    pub fn from_existing(data: &[u8]) -> Result<Self>;  // For Milestone 4
    pub fn add_track(&mut self, track: &Track, usb_path: &str, anlz_path: &str) -> u32;
    pub fn add_playlist(&mut self, playlist: &Playlist, track_ids: &[u32]);
    pub fn write(&self, path: &Path) -> Result<()>;
}
```

## Critical Files to Reference
- `src-tauri/src/services/export/rekordbox.rs` - Current implementation (2,234 lines)
- `src-tauri/src/services/export/anlz/` - Pattern for binrw module structure
- `src-tauri/src/services/export/mod.rs` - Integration point
- `../pyrekordbox/` - Reference Python implementation for correct PDB output
