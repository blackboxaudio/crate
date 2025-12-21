# Rekordbox export.pdb Binary Format Specification

This document provides a complete technical specification for Pioneer Rekordbox's DeviceSQL database format (`.pdb` files). This format is used on USB drives exported from Rekordbox for use with Pioneer DJ equipment (CDJs, XDJs, etc.).

**Sources:**
- [Deep Symmetry Rekordbox Export Analysis](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html)
- [Crate Digger Kaitai Struct Definition](https://github.com/Deep-Symmetry/crate-digger/blob/main/src/main/kaitai/rekordbox_pdb.ksy)

---

## Table of Contents

1. [Overview](#overview)
2. [Byte Ordering](#byte-ordering)
3. [File Structure](#file-structure)
4. [File Header](#file-header)
5. [Table Pointers](#table-pointers)
6. [Table Types](#table-types)
7. [Page Structure](#page-structure)
8. [Row Index System](#row-index-system)
9. [DeviceSQL String Format](#devicesql-string-format)
10. [Track Rows](#track-rows)
11. [Artist Rows](#artist-rows)
12. [Album Rows](#album-rows)
13. [Genre Rows](#genre-rows)
14. [Label Rows](#label-rows)
15. [Key Rows](#key-rows)
16. [Color Rows](#color-rows)
17. [Artwork Rows](#artwork-rows)
18. [Playlist Tree Rows](#playlist-tree-rows)
19. [Playlist Entry Rows](#playlist-entry-rows)
20. [History Playlist Rows](#history-playlist-rows)
21. [History Entry Rows](#history-entry-rows)
22. [Extended Format (exportExt.pdb)](#extended-format-exportextpdb)
23. [Relationship to Analysis Files](#relationship-to-analysis-files)
24. [Implementation Notes](#implementation-notes)

---

## Overview

The `export.pdb` file is a relational database optimized for low-power DJ hardware. Key characteristics:

- **Fixed-size pages**: All pages share the same size (typically 4096 bytes)
- **Linked-list tables**: Each table is a chain of pages
- **Heap-based rows**: Variable-length rows stored in page heaps
- **Backward indexing**: Row offsets built from page end toward start
- **Reference IDs**: Tables cross-reference each other via numeric IDs

The database contains track metadata, playlists, organizational data, and references to external analysis files.

---

## Byte Ordering

**All multi-byte numeric values in export.pdb use little-endian byte order.**

Example: The 32-bit value `0x00001000` (4096) is stored as bytes `00 10 00 00`.

> **Important:** This is opposite to the analysis files (.DAT/.EXT) which use big-endian ordering.

---

## File Structure

```
┌─────────────────────────────────┐
│         File Header             │  ← First page (page index 0)
│   (includes table pointers)     │
├─────────────────────────────────┤
│         Table Page 1            │  ← len_page bytes each
├─────────────────────────────────┤
│         Table Page 2            │
├─────────────────────────────────┤
│            ...                  │
├─────────────────────────────────┤
│         Table Page N            │
└─────────────────────────────────┘
```

All pages are exactly `len_page` bytes. Pages are identified by their index (0-based position in file).

---

## File Header

The file header occupies the first page and defines the database structure.

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| `0x00` | 4 | `unknown1` | Always zero (4 null bytes) |
| `0x04` | 4 | `len_page` | Page size in bytes (all pages) |
| `0x08` | 4 | `num_tables` | Number of tables in database |
| `0x0C` | 4 | `next_unused_page` | Index of next unused page (often beyond file end) |
| `0x10` | 4 | `unknown2` | Unknown purpose |
| `0x14` | 4 | `sequence` | Database version, incremented on sync |
| `0x18` | 4 | `gap` | Padding/unknown |
| `0x1C` | var | `table_pointers[]` | Array of table pointer structures |

### Example Header (hex dump)

```
00000000: 00 00 00 00 00 10 00 00  0E 00 00 00 2A 00 00 00  |............*...|
          ├──────────┤ ├──────────┤ ├──────────┤ ├──────────┤
          unknown1    len_page     num_tables   next_unused
                      (4096)       (14 tables)
```

---

## Table Pointers

Each table pointer is 16 bytes, describing one table's location and type.

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| `0x00` | 4 | `type` | Table type identifier (see Table Types) |
| `0x04` | 4 | `empty_candidate` | First page in garbage collection chain |
| `0x08` | 4 | `first_page` | Index of table's first data page |
| `0x0C` | 4 | `last_page` | Index of table's last data page |

The `empty_candidate` field points to pages with deleted rows available for reuse.

---

## Table Types

| Type | Hex | Name | Description |
|------|-----|------|-------------|
| 0 | `0x00` | `tracks` | Track metadata (title, artist, BPM, etc.) |
| 1 | `0x01` | `genres` | Musical genre names |
| 2 | `0x02` | `artists` | Artist/performer names |
| 3 | `0x03` | `albums` | Album titles with artist references |
| 4 | `0x04` | `labels` | Record label names |
| 5 | `0x05` | `keys` | Musical key names (e.g., "Am", "C#m") |
| 6 | `0x06` | `colors` | Color label definitions |
| 7 | `0x07` | `playlist_tree` | Playlist/folder hierarchy |
| 8 | `0x08` | `playlist_entries` | Track-to-playlist mappings |
| 13 | `0x0D` | `artwork` | Artwork file path references |
| 16 | `0x10` | `columns` | Column layout (details unknown) |
| 17 | `0x11` | `history_playlists` | History playlist definitions |
| 18 | `0x12` | `history_entries` | History playlist track entries |
| 19 | `0x13` | `history` | Sync history metadata |

---

## Page Structure

Each table page follows this structure:

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| `0x00` | 4 | `unknown1` | Always zero |
| `0x04` | 4 | `page_index` | This page's index in file |
| `0x08` | 4 | `type` | Table type (matches table pointer) |
| `0x0C` | 4 | `next_page` | Next page index (or 0 if last) |
| `0x10` | 4 | `unknown2` | Unknown |
| `0x14` | 4 | `unknown3` | Unknown |
| `0x18` | 1 | `num_rows_small` | Low 8 bits of row group count |
| `0x19` | 1 | `unknown4` | Unknown |
| `0x1A` | 1 | `num_rows_large` | High bits + actual row count |
| `0x1B` | 1 | `page_flags` | Page type indicator |
| `0x1C` | 2 | `free_size` | Unused heap space (bytes) |
| `0x1E` | 2 | `used_size` | Used heap space (bytes) |
| `0x20` | 2 | `unknown5` | Unknown (often `0x0000`) |
| `0x22` | 2 | `num_rows_large_again` | Copy of row count |
| `0x24` | 2 | `unknown6` | Unknown |
| `0x26` | 2 | `unknown7` | Unknown |
| `0x28` | var | `heap[]` | Row data heap |

### Page Flags

| Value | Meaning |
|-------|---------|
| `0x24` | Normal data page |
| `0x34` | Normal data page (alternate) |
| `0x44` | Strange/non-data page |
| `0x64` | Strange/non-data page (alternate) |

**Rule:** A page contains valid row data if `(page_flags & 0x40) == 0`.

### Row Count Calculation

The actual row count is encoded across multiple bytes:

```
num_row_groups = (num_rows_large >> 4) | (num_rows_small << 4)  // 13 bits
num_rows = num_rows_large & 0x1F                                 // 5 bits, but see below
```

For precise row counting, iterate through row index groups.

---

## Row Index System

Rows are located via a backward-built index at the end of each page.

### Structure

```
Page Layout:
┌───────────────────────────────────────────────────────────┐
│ Page Header (0x00 - 0x27)                                 │
├───────────────────────────────────────────────────────────┤
│ Heap (0x28 - varies)                                      │
│   Row N data...                                           │
│   Row 1 data...                                           │
│   Row 0 data...                                           │
├───────────────────────────────────────────────────────────┤
│ (unused space)                                            │
├───────────────────────────────────────────────────────────┤
│ Row Index (built backwards from page end)                 │
│   Group 0: presence_flags (2 bytes) + offsets (up to 16)  │
│   Group 1: presence_flags (2 bytes) + offsets             │
│   ...                                                     │
└───────────────────────────────────────────────────────────┘
```

### Row Group Structure

Each group can hold up to 16 rows:

| Offset from group start | Size | Field | Description |
|------------------------|------|-------|-------------|
| `0x00` | 2 | `presence_flags` | Bitmask of present rows (bit N = row N exists) |
| `0x02` | 2 each | `row_offsets[]` | Offset for each present row |

### Finding Row Offsets

1. Calculate group index: `group = row_number / 16`
2. Calculate bit position: `bit = row_number % 16`
3. Navigate to group from page end
4. Check if `presence_flags & (1 << bit)` is set
5. Count set bits before this position to find offset index
6. Row data is at: `heap_start (0x28) + row_offset`

### Presence Flags

The 16-bit presence flag indicates which rows in the group exist:
- Bit 0 = Row 0 (or row 16, 32, etc. for subsequent groups)
- Bit 1 = Row 1
- ...
- Bit 15 = Row 15

Deleted rows have their bit cleared but may still occupy heap space.

---

## DeviceSQL String Format

Strings use a custom encoding with a format indicator byte.

### Format Byte Flags

```
Bit 7 (0x80): E - Endianness (1 = little-endian for UTF-16)
Bit 6 (0x40): A - ASCII encoding
Bit 5 (0x20): N - Narrow (UTF-8) encoding
Bit 4 (0x10): W - Wide (UTF-16) encoding
Bit 0 (0x01): S - Short string (length in same byte)
```

### Short ASCII String (most common for short text)

When bit 0 is set, the string is "short ASCII":

```
┌──────────────────┬────────────────────────┐
│ length_and_kind  │ ASCII data             │
│ (1 byte)         │ (variable)             │
└──────────────────┴────────────────────────┘
```

- `length = length_and_kind >> 1`
- Data immediately follows (no null terminator)
- Maximum length: 127 bytes

**Example:** `0x0B 48 65 6C 6C 6F` = length 5 (`0x0B >> 1 = 5`), "Hello"

### Long ASCII String

Format byte `0x40`:

```
┌────────┬──────────┬─────────┬────────────────────┐
│ 0x40   │ length   │ padding │ ASCII data         │
│ 1 byte │ 2 bytes  │ 1 byte  │ variable           │
└────────┴──────────┴─────────┴────────────────────┘
```

- Length is total bytes including 4-byte header
- Actual string length = `length - 4`
- No null terminator

### Long UTF-16LE String

Format byte `0x90`:

```
┌────────┬──────────┬─────────┬────────────────────┐
│ 0x90   │ length   │ padding │ UTF-16LE data      │
│ 1 byte │ 2 bytes  │ 1 byte  │ variable           │
└────────┴──────────┴─────────┴────────────────────┘
```

- Length is total bytes including 4-byte header
- Actual string bytes = `length - 4`
- Character count = `(length - 4) / 2`
- No null terminator

### ISRC String (Special Case)

ISRC codes use a slightly different format:

```
┌────────┬──────────┬─────────┬────────────────────┐
│ 0x90   │ length   │ 0x03    │ ASCII + null       │
│ 1 byte │ 2 bytes  │ 1 byte  │ variable           │
└────────┴──────────┴─────────┴────────────────────┘
```

- Byte 3 is `0x03` instead of `0x00`
- Data is ASCII with null terminator
- The "mangled" format is specific to ISRC fields

---

## Track Rows

Track rows are the most complex, containing extensive metadata.

### Header (fixed portion)

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 2 | u16 | `subtype` | Always `0x0024` |
| `0x02` | 2 | u16 | `index_shift` | Unknown purpose |
| `0x04` | 4 | u32 | `bitmask` | Unknown bit flags |
| `0x08` | 4 | u32 | `sample_rate` | Samples per second (e.g., 44100) |
| `0x0C` | 4 | u32 | `composer_id` | Composer artist row ID |
| `0x10` | 4 | u32 | `file_size` | Audio file size in bytes |
| `0x14` | 4 | u32 | `unknown1` | Unknown |
| `0x18` | 4 | u32 | `unknown2` | Unknown |
| `0x1C` | 4 | u32 | `artwork_id` | Artwork row ID (0 = none) |
| `0x20` | 4 | u32 | `key_id` | Musical key row ID (0 = none) |
| `0x24` | 4 | u32 | `original_artist_id` | Original artist row ID |
| `0x28` | 4 | u32 | `label_id` | Label row ID (0 = none) |
| `0x2C` | 4 | u32 | `remixer_id` | Remixer artist row ID |
| `0x30` | 4 | u32 | `bitrate` | Bits per second |
| `0x34` | 4 | u32 | `track_number` | Track number on album |
| `0x38` | 4 | u32 | `tempo` | BPM × 100 (e.g., 12800 = 128.00 BPM) |
| `0x3C` | 4 | u32 | `genre_id` | Genre row ID (0 = none) |
| `0x40` | 4 | u32 | `album_id` | Album row ID (0 = none) |
| `0x44` | 4 | u32 | `artist_id` | Primary artist row ID |
| `0x48` | 4 | u32 | `id` | This track's unique ID |
| `0x4C` | 2 | u16 | `disc_number` | Disc number for multi-disc albums |
| `0x4E` | 2 | u16 | `play_count` | Number of times played |
| `0x50` | 2 | u16 | `year` | Release year |
| `0x52` | 2 | u16 | `sample_depth` | Bits per sample (e.g., 16, 24) |
| `0x54` | 2 | u16 | `duration` | Duration in seconds |
| `0x56` | 2 | u16 | `unknown3` | Unknown |
| `0x58` | 1 | u8 | `color_id` | Color label ID (0 = none) |
| `0x59` | 1 | u8 | `rating` | Rating 0-5 (0 = unrated) |
| `0x5A` | 2 | u16 | `unknown4` | Unknown |
| `0x5C` | 2 | u16 | `unknown5` | Unknown |

### String Offsets (at 0x5E)

Following the fixed header are 21 two-byte offsets pointing to DeviceSQL strings:

| Index | Offset | Field | Description |
|-------|--------|-------|-------------|
| 0 | `0x5E` | `ofs_isrc` | ISRC code (mangled format) |
| 1 | `0x60` | `ofs_texter` | Lyricist/text writer |
| 2 | `0x62` | `ofs_unknown1` | Unknown string |
| 3 | `0x64` | `ofs_unknown2` | Unknown string |
| 4 | `0x66` | `ofs_unknown3` | Unknown string |
| 5 | `0x68` | `ofs_message` | DJ message/notes |
| 6 | `0x6A` | `ofs_kuvo_public` | Kuvo public info |
| 7 | `0x6C` | `ofs_autoload_hotcues` | Autoload hot cues setting |
| 8 | `0x6E` | `ofs_unknown4` | Unknown string |
| 9 | `0x70` | `ofs_unknown5` | Unknown string |
| 10 | `0x72` | `ofs_date_added` | Date added (YYYY-MM-DD) |
| 11 | `0x74` | `ofs_release_date` | Release date (YYYY-MM-DD) |
| 12 | `0x76` | `ofs_mix_name` | Mix/version name |
| 13 | `0x78` | `ofs_unknown6` | Unknown string |
| 14 | `0x7A` | `ofs_analyze_path` | Path to ANLZ file |
| 15 | `0x7C` | `ofs_analyze_date` | Analysis date (YYYY-MM-DD) |
| 16 | `0x7E` | `ofs_comment` | User comment |
| 17 | `0x80` | `ofs_title` | Track title |
| 18 | `0x82` | `ofs_unknown7` | Unknown string |
| 19 | `0x84` | `ofs_filename` | Audio filename |
| 20 | `0x86` | `ofs_file_path` | Full file path |

**String Offset Resolution:** Each offset is relative to the start of the row. Add the offset to the row's heap address to find the string.

### File Type Codes

| Value | Format |
|-------|--------|
| `0x00` | Unknown |
| `0x01` | MP3 |
| `0x04` | M4A (AAC) |
| `0x05` | FLAC |
| `0x0B` | WAV |
| `0x0C` | AIFF |

---

## Artist Rows

Artists use a variable-length format with short/long variants.

### Short Variant (subtype `0x0060`)

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 2 | u16 | `subtype` | `0x0060` |
| `0x02` | 2 | u16 | `index_shift` | Unknown |
| `0x04` | 4 | u32 | `id` | Artist unique ID |
| `0x08` | 1 | u8 | `constant` | Always `0x03` |
| `0x09` | 1 | u8 | `ofs_name` | Single-byte name offset |

### Long Variant (subtype `0x0064`)

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 2 | u16 | `subtype` | `0x0064` |
| `0x02` | 2 | u16 | `index_shift` | Unknown |
| `0x04` | 4 | u32 | `id` | Artist unique ID |
| `0x08` | 2 | u16 | `constant` | Always `0x0003` |
| `0x0A` | 2 | u16 | `ofs_name` | Two-byte name offset |

**Detection:** Check subtype to determine variant. Use `subtype & 0x04` to detect long variant.

---

## Album Rows

Albums also have short/long variants and include an artist reference.

### Short Variant (subtype `0x0080`)

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 2 | u16 | `subtype` | `0x0080` |
| `0x02` | 2 | u16 | `index_shift` | Unknown |
| `0x04` | 4 | u32 | `unknown1` | Unknown |
| `0x08` | 4 | u32 | `artist_id` | Album artist row ID |
| `0x0C` | 4 | u32 | `id` | Album unique ID |
| `0x10` | 4 | u32 | `unknown2` | Unknown |
| `0x14` | 1 | u8 | `unknown3` | Unknown |
| `0x15` | 1 | u8 | `ofs_name` | Single-byte name offset |

### Long Variant (subtype `0x0084`)

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 2 | u16 | `subtype` | `0x0084` |
| `0x02` | 2 | u16 | `index_shift` | Unknown |
| `0x04` | 4 | u32 | `unknown1` | Unknown |
| `0x08` | 4 | u32 | `artist_id` | Album artist row ID |
| `0x0C` | 4 | u32 | `id` | Album unique ID |
| `0x10` | 6 | | `unknown2` | Unknown bytes |
| `0x16` | 2 | u16 | `ofs_name` | Two-byte name offset |

---

## Genre Rows

Simple structure with ID and name string.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 4 | u32 | `id` | Genre unique ID |
| `0x04` | var | string | `name` | DeviceSQL string with genre name |

---

## Label Rows

Identical structure to genres.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 4 | u32 | `id` | Label unique ID |
| `0x04` | var | string | `name` | DeviceSQL string with label name |

---

## Key Rows

Musical key references.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 4 | u32 | `id` | Key unique ID |
| `0x04` | 4 | u32 | `id2` | Duplicate of ID |
| `0x08` | var | string | `name` | DeviceSQL string (e.g., "Am", "C#m") |

---

## Color Rows

Color label definitions for track organization.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 4 | u32 | `unknown1` | Unknown |
| `0x04` | 1 | u8 | `unknown2` | Unknown |
| `0x05` | 2 | u16 | `id` | Color unique ID |
| `0x07` | 1 | u8 | `unknown3` | Unknown |
| `0x08` | var | string | `name` | DeviceSQL string with color name |

### Standard Color IDs

| ID | Color |
|----|-------|
| 0 | No color |
| 1 | Pink |
| 2 | Red |
| 3 | Orange |
| 4 | Yellow |
| 5 | Green |
| 6 | Aqua |
| 7 | Blue |
| 8 | Purple |

---

## Artwork Rows

References to artwork image files.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 4 | u32 | `id` | Artwork unique ID |
| `0x04` | var | string | `path` | DeviceSQL string with file path |

**Path format:** `/PIONEER/USBANLZ/P0nn/xxxxxxxx/ARTWORK/filename.jpg`

**High-resolution variant:** Insert `_m` before extension: `filename_m.jpg`

---

## Playlist Tree Rows

Hierarchical playlist and folder structure.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 4 | u32 | `parent_id` | Parent folder ID (0 = root) |
| `0x04` | 4 | u32 | `unknown1` | Unknown |
| `0x08` | 4 | u32 | `sort_order` | Display sort order |
| `0x0C` | 4 | u32 | `id` | Playlist/folder unique ID |
| `0x10` | 4 | u32 | `raw_is_folder` | Non-zero if this is a folder |
| `0x14` | var | string | `name` | DeviceSQL string with name |

**Hierarchy:** Build tree by following `parent_id` references. Root items have `parent_id = 0`.

---

## Playlist Entry Rows

Maps tracks to playlist positions.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 4 | u32 | `entry_index` | Position in playlist (1-based) |
| `0x04` | 4 | u32 | `track_id` | Track row ID |
| `0x08` | 4 | u32 | `playlist_id` | Playlist row ID |

---

## History Playlist Rows

History/session playlist definitions.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 4 | u32 | `id` | History playlist unique ID |
| `0x04` | var | string | `name` | DeviceSQL string with name |

---

## History Entry Rows

Tracks in history playlists.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 4 | u32 | `track_id` | Track row ID |
| `0x04` | 4 | u32 | `playlist_id` | History playlist row ID |
| `0x08` | 4 | u32 | `entry_index` | Position in history playlist |

---

## Extended Format (exportExt.pdb)

The `exportExt.pdb` file adds tag/category support with additional table types.

### Tag Rows (type `0x0E`)

#### Short Variant (subtype `0x0680`)

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 2 | u16 | `subtype` | `0x0680` |
| `0x02` | 2 | u16 | `tag_index` | Increments by `0x20` |
| `0x04` | 8 | | `unknown1` | Always zero |
| `0x0C` | 4 | u32 | `category_id` | Category ID (0 if this is a category) |
| `0x10` | 4 | u32 | `category_pos` | Display position in category |
| `0x14` | 4 | u32 | `id` | Tag unique ID |
| `0x18` | 4 | u32 | `raw_is_category` | Non-zero if category row |
| `0x1C` | 1 | u8 | `unknown2` | Unknown |
| `0x1D` | 1 | u8 | `flags` | Unknown flags |
| `0x1E` | 1 | u8 | `constant` | Always `0x03` |
| `0x1F` | 1 | u8 | `ofs_name` | Name string offset |
| `0x20` | 1 | u8 | `ofs_unknown` | Unknown string offset |

#### Long Variant (subtype `0x0684`)

Similar structure with 2-byte offsets at positions `0x21-0x22` (name) and `0x23-0x24` (unknown).

### Tag-Track Rows (type `0x0F`)

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| `0x00` | 4 | u32 | `unknown` | Always zero |
| `0x04` | 4 | u32 | `track_id` | Track row ID |
| `0x08` | 4 | u32 | `tag_id` | Tag row ID |
| `0x0C` | 4 | u32 | `unknown2` | Constant values |

---

## Relationship to Analysis Files

Track rows contain a path (string offset 14: `ofs_analyze_path`) pointing to ANLZ files:

- **`.DAT` files**: Basic analysis (waveform, cue points, beat grid)
- **`.EXT` files**: Extended analysis (colored waveforms)
- **`.2EX` files**: CDJ-3000 three-band waveforms

**Important:** Analysis files use **big-endian** byte order (opposite of export.pdb).

Typical path format: `/PIONEER/USBANLZ/P0nn/xxxxxxxx/ANLZ0000.DAT`

---

## Implementation Notes

### ID Management

- All IDs are positive integers starting from 1
- ID 0 typically means "none" or "not set"
- IDs must be unique within each table
- Cross-references use IDs (not page/row positions)

### Page Allocation

1. New pages are allocated from `next_unused_page`
2. Deleted row pages go to `empty_candidate` chain
3. Each table maintains its own `first_page` → `last_page` chain

### Row Insertion

1. Find table's last page
2. Check if `free_size` can accommodate new row
3. If not, allocate new page and link it
4. Write row data to heap
5. Update row index at page end
6. Update `used_size`, `free_size`, row counts

### Row Deletion

1. Clear presence bit in row index
2. Row data remains in heap (not compacted)
3. Page becomes garbage collection candidate when mostly empty

### String Writing

1. For short ASCII (≤127 chars): `(length << 1) | 0x01` + data
2. For long ASCII: `0x40` + length (2 bytes, includes header) + `0x00` + data
3. For UTF-16LE: `0x90` + length (2 bytes, includes header) + `0x00` + data

### Calculating Page Positions

```
page_position_in_file = page_index * len_page
```

### Heap and Index Layout

```
Available heap space: len_page - 0x28 - (row_index_size)
Row index grows backward from page end
Heap grows forward from offset 0x28
```

### Common Pitfalls

1. **Byte order**: Always little-endian for all numeric fields
2. **String lengths**: Include header bytes in long string lengths
3. **Row presence**: Always check presence flags before reading rows
4. **Offset base**: String offsets are relative to row start, not page start
5. **Page flags**: Only process pages where `(page_flags & 0x40) == 0`

---

## Quick Reference: Writing a Valid export.pdb

### Minimum File Structure

1. File header page with:
   - `len_page = 4096` (or other power of 2)
   - Table pointers for required tables
   - `sequence = 1`

2. At least these tables (empty is OK):
   - tracks (type 0)
   - genres (type 1)
   - artists (type 2)
   - albums (type 3)
   - keys (type 5)
   - colors (type 6)
   - playlist_tree (type 7)
   - playlist_entries (type 8)
   - artwork (type 13)

3. Each table needs at least one page (even if empty)

### Validation Checklist

- [ ] All pages are exactly `len_page` bytes
- [ ] Page indices are consecutive starting from 0
- [ ] Table chains are properly linked (`first_page` → `next_page` → ... → `last_page`)
- [ ] Last page in chain has `next_page = 0`
- [ ] Row offsets point to valid heap positions
- [ ] All referenced IDs exist in their respective tables
- [ ] Strings are properly encoded with correct length bytes
- [ ] `free_size + used_size` accounts for heap space correctly
