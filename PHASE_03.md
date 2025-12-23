# Milestone 3: Device Library Plus Implementation Plan

## Overview
Add support for exporting to Device Library Plus format (SQLCipher-encrypted SQLite database) used by newer Pioneer DJ hardware (OPUS-QUAD, OMNIS-DUO, XDJ-AZ).

## Key Decisions
- **Format Selection**: Manual setting in LibraryTab.svelte, OFF by default (PDB remains default)
- **Schema Scope**: Full 22-table schema implementation
- **Read Support**: Write-only for Milestone 3 (read support in Milestone 4)

---

## Implementation Tasks

### 1. Add Dependencies to Cargo.toml
**File**: `src-tauri/Cargo.toml`

```toml
# Change existing rusqlite line:
rusqlite = { version = "0.38.0", features = ["bundled-sqlcipher"] }

# Add for key deobfuscation:
flate2 = "1.0"
base85 = "2.0"
```

Note: `bundled-sqlcipher` replaces `bundled` and includes SQLCipher support.

---

### 2. Create Device Library Plus Module Structure
**New directory**: `src-tauri/src/services/export/device_library_plus/`

```
device_library_plus/
  mod.rs           # Public API, DeviceLibraryPlusWriter
  encryption.rs    # SQLCipher key deobfuscation
  schema.rs        # CREATE TABLE SQL statements
  models.rs        # Rust structs for all 22 tables
  database.rs      # Database operations (insert, helpers)
```

---

### 3. Implement SQLCipher Key Deobfuscation
**File**: `device_library_plus/encryption.rs`

```rust
// Obfuscated blob from pyrekordbox
const BLOB: &[u8] = b"PN_1dH8$oLJY)16j_RvM6qphWw`476>;C1cWmI#se(PG`j}~xAjlufj?`#0i{;=glh(SkW)y0>n?YEiD`l%t(";
const BLOB_KEY: &[u8] = b"657f48f84c437cc1";

pub fn get_sqlcipher_key() -> Result<String> {
    // 1. Base85 decode
    // 2. XOR with BLOB_KEY (cycling)
    // 3. Zlib decompress
    // 4. UTF-8 decode
    // Result starts with "r8gd"
}
```

---

### 4. Implement Schema (All 22 Tables)
**File**: `device_library_plus/schema.rs`

Tables to implement:
1. `content` - Main track table (40+ columns)
2. `artist` - Artist entries
3. `album` - Album entries with artist/image FK
4. `genre` - Genre entries
5. `label` - Record label entries
6. `key` - Musical key entries
7. `color` - Color tag entries
8. `image` - Artwork image paths
9. `playlist` - Playlists/folders (hierarchical)
10. `playlist_content` - Playlist track entries
11. `cue` - Cue points and loops
12. `hotCueBankList` - Hot cue bank collections
13. `hotCueBankList_cue` - Cues in hot cue banks
14. `history` - DJ mix history
15. `history_content` - Tracks in history
16. `myTag` - Custom tag categories
17. `myTag_content` - Tracks in custom tags
18. `property` - Database metadata
19. `recommendedLike` - Track recommendations
20. `menuItem` - Menu items for sorting/display
21. `category` - Category groupings
22. `sort` - Sort order configuration

Each table needs:
- `CREATE TABLE` SQL statement
- Proper data types (INTEGER, VARCHAR(255), TEXT)
- Foreign key constraints
- UNIQUE constraints where needed

---

### 5. Implement Rust Models
**File**: `device_library_plus/models.rs`

```rust
pub struct Content {
    pub content_id: i64,
    pub title: Option<String>,
    pub bpmx100: Option<i32>,
    pub length: Option<i32>,
    pub artist_id_artist: Option<i64>,
    pub album_id: Option<i64>,
    // ... 40+ fields total
    pub path: String,
    pub fileName: String,
    pub fileSize: i64,
    pub fileType: i32,
    // ...
}

pub struct Playlist {
    pub playlist_id: i64,
    pub sequenceNo: i32,
    pub name: String,
    pub attribute: i32,  // 0=playlist, 1=folder, 4=smart
    pub playlist_id_parent: Option<i64>,
}

// ... models for all 22 tables
```

---

### 6. Implement Database Operations
**File**: `device_library_plus/database.rs`

```rust
pub struct DeviceLibraryPlusWriter {
    conn: Connection,
}

impl DeviceLibraryPlusWriter {
    /// Create new encrypted database at path
    pub fn create(path: &Path) -> Result<Self>;

    /// Insert content (track)
    pub fn add_content(&mut self, content: &Content) -> Result<i64>;

    /// Get or create artist by name
    pub fn get_or_create_artist(&mut self, name: &str) -> Result<i64>;

    /// Get or create album by name
    pub fn get_or_create_album(&mut self, name: &str, artist_id: Option<i64>) -> Result<i64>;

    /// Similar helpers for genre, key, color, label, image

    /// Add playlist
    pub fn add_playlist(&mut self, playlist: &Playlist) -> Result<i64>;

    /// Add track to playlist
    pub fn add_playlist_content(&mut self, playlist_id: i64, content_id: i64, seq: i32) -> Result<()>;

    /// Add cue point
    pub fn add_cue(&mut self, cue: &Cue) -> Result<i64>;

    /// Set property (database metadata)
    pub fn set_property(&mut self, prop: &Property) -> Result<()>;

    /// Update content count in property
    pub fn update_content_count(&mut self) -> Result<()>;

    /// Commit all changes
    pub fn commit(&mut self) -> Result<()>;
}
```

Key implementation details:
- Open with SQLCipher pragma: `PRAGMA key = 'derived_key';`
- Use deduplication for artist/album/genre/key/color (same as PDB writer)
- DateTime format: `YYYY-MM-DD HH:MM:SS.SSS +00:00`

---

### 7. Add Export Format Setting
**Files to modify**:
- `src/lib/types/index.ts` - Add `ExportFormat` type
- `src/lib/stores/settings.ts` - Add `exportFormat` setting
- `src/lib/components/settings/LibraryTab.svelte` - Add toggle/selector
- `src/lib/i18n/locales/*.json` - Add translations
- `src-tauri/src/models/settings.rs` - Add `export_format` field

```typescript
// types/index.ts
export type ExportFormat = 'pdb' | 'device_library_plus'

// settings store default
exportFormat: 'pdb' as ExportFormat
```

UI in LibraryTab.svelte:
- Toggle/select for "Device Library Plus Export"
- Description: "Enable for newer Pioneer DJ hardware (OPUS-QUAD, OMNIS-DUO, XDJ-AZ)"
- Default: OFF (uses PDB format)

---

### 8. Integrate with ExportService
**File**: `src-tauri/src/services/export/mod.rs`

Modify `generate_rekordbox_pdb()` to check format setting:

```rust
fn generate_database(
    &self,
    mount_point: &str,
    playlists_with_tracks: &[(Playlist, Vec<Track>)],
    device_tracks: &[DeviceTrack],
    use_device_library_plus: bool,
) -> Result<()> {
    if use_device_library_plus {
        self.generate_device_library_plus(mount_point, playlists_with_tracks, device_tracks)
    } else {
        self.generate_rekordbox_pdb(mount_point, playlists_with_tracks, device_tracks)
    }
}

fn generate_device_library_plus(
    &self,
    mount_point: &str,
    playlists_with_tracks: &[(Playlist, Vec<Track>)],
    device_tracks: &[DeviceTrack],
) -> Result<()> {
    let db_path = Path::new(mount_point)
        .join("PIONEER")
        .join("rekordbox")
        .join("exportLibrary.db");

    let mut writer = DeviceLibraryPlusWriter::create(&db_path)?;

    // Add tracks, playlists, cues similar to PDB flow
    // ...

    writer.commit()?;
    Ok(())
}
```

---

### 9. Update ExportRequest Model
**File**: `src-tauri/src/models/export.rs`

Add format field to ExportRequest:

```rust
pub struct ExportRequest {
    pub device_id: String,
    pub device_name: String,
    pub mount_point: String,
    pub playlist_ids: Vec<String>,
    pub enable_sync: bool,
    pub use_device_library_plus: bool,  // NEW
}
```

---

### 10. Update Frontend API
**File**: `src/lib/api/export.ts`

Include format setting in export request:

```typescript
export async function exportToDevice(request: ExportRequest): Promise<ExportResult> {
    const settings = get(settingsStore)
    return invoke('export_playlists', {
        request: {
            ...request,
            use_device_library_plus: settings.exportFormat === 'device_library_plus'
        }
    })
}
```

---

## Files Summary

### New Files
| File | Purpose |
|------|---------|
| `src-tauri/src/services/export/device_library_plus/mod.rs` | Module exports |
| `src-tauri/src/services/export/device_library_plus/encryption.rs` | Key deobfuscation |
| `src-tauri/src/services/export/device_library_plus/schema.rs` | CREATE TABLE statements |
| `src-tauri/src/services/export/device_library_plus/models.rs` | Rust structs |
| `src-tauri/src/services/export/device_library_plus/database.rs` | Database operations |

### Modified Files
| File | Change |
|------|--------|
| `src-tauri/Cargo.toml` | Add `bundled-sqlcipher`, `flate2`, `base85` |
| `src-tauri/src/services/export/mod.rs` | Add format detection, Device Library Plus generation |
| `src-tauri/src/models/export.rs` | Add `use_device_library_plus` field |
| `src-tauri/src/models/settings.rs` | Add `export_format` field |
| `src/lib/types/index.ts` | Add `ExportFormat` type |
| `src/lib/stores/settings.ts` | Add `exportFormat` setting |
| `src/lib/api/export.ts` | Include format in export request |
| `src/lib/components/settings/LibraryTab.svelte` | Add format toggle |
| `src/lib/i18n/locales/*.json` | Add translations |

---

## Implementation Order

1. **Dependencies** - Update Cargo.toml
2. **Encryption** - Implement key deobfuscation (can test independently)
3. **Schema** - Create all 22 table definitions
4. **Models** - Create Rust structs
5. **Database** - Implement DeviceLibraryPlusWriter
6. **Settings** - Add export format setting (backend + frontend)
7. **Integration** - Wire into ExportService
8. **Testing** - Test export to USB, verify with Pioneer hardware

---

## Testing Strategy

1. **Key derivation test**: Verify deobfuscated key starts with "r8gd"
2. **Database creation test**: Create encrypted DB, verify it can be opened with correct key
3. **Schema test**: Insert sample data into all tables, verify constraints
4. **Export test**: Full export flow to USB
5. **Device test**: Test on actual Pioneer DJ hardware if available
