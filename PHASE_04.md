# Milestone 4: PDB Merging & Sync Implementation Plan

## Overview

Implement PDB file reading, merging with existing device data, sync diff calculation, checkpoint-based resume, and parallel track copying.

## Key Decisions
- **Removal Strategy**: Conservative (only remove tracks deleted from library)
- **Sync Preview**: No preview modal - show summary after sync
- **Format Scope**: PDB only (Device Library Plus uses SQLite - easier to handle)

---

## Phase 1: PDB Reader

### File: `src-tauri/src/services/export/pdb/reader.rs`

**Goal**: Parse existing export.pdb files to extract tracks, playlists, and lookup tables.

#### Core Structures
```rust
pub struct ParsedTrack {
    pub id: u32,
    pub title: String,
    pub artist_id: u32,
    pub album_id: u32,
    pub genre_id: u32,
    pub key_id: u32,
    pub color_id: u8,
    pub file_path: String,      // USB path (e.g., /Contents/Artist/Album/file.mp3)
    pub anlz_path: String,
    pub duration_seconds: u16,
    pub tempo: u32,             // BPM * 100
}

pub struct ParsedPlaylist {
    pub id: u32,
    pub parent_id: u32,
    pub name: String,
    pub is_folder: bool,
    pub track_ids: Vec<u32>,
}

pub struct ParsedPdb {
    pub tracks: Vec<ParsedTrack>,
    pub playlists: Vec<ParsedPlaylist>,
    pub artists: HashMap<u32, String>,
    pub albums: HashMap<u32, (String, u32)>,  // (name, artist_id)
    pub genres: HashMap<u32, String>,
    pub keys: HashMap<u32, String>,
    pub next_track_id: u32,
    pub next_playlist_id: u32,
}
```

#### Implementation Steps
1. Add `BinRead` implementation to `DeviceSQLString` in `strings.rs`
2. Read `FileHeader` from page 0 to get table descriptors
3. For each table, traverse pages via `next_page` chain
4. Parse row groups (backwards from page end, up to 16 rows per group)
5. Extract row data based on table type

#### Key Files to Modify
- `src-tauri/src/services/export/pdb/strings.rs` - Add `read_from()` method
- `src-tauri/src/services/export/pdb/mod.rs` - Export reader

---

## Phase 2: PDB Merger

### File: `src-tauri/src/services/export/pdb/merger.rs`

**Goal**: Combine existing PDB data with new export data, preserving IDs where possible.

#### Core Structure
```rust
pub struct PdbMerger {
    existing_tracks: HashMap<String, ParsedTrack>,  // USB path -> track
    existing_playlists: HashMap<String, ParsedPlaylist>,  // name -> playlist
    existing_artists: HashMap<String, u32>,
    existing_albums: HashMap<(String, u32), u32>,
    existing_genres: HashMap<String, u32>,
    existing_keys: HashMap<String, u32>,
    next_track_id: u32,
    next_playlist_id: u32,
    // ... other ID counters
}

impl PdbMerger {
    pub fn from_parsed(parsed: ParsedPdb) -> Self;
    pub fn get_existing_track(&self, usb_path: &str) -> Option<&ParsedTrack>;
    pub fn get_or_allocate_track_id(&mut self, usb_path: &str) -> u32;
    pub fn get_or_create_artist(&mut self, name: &str) -> u32;
    // ... other lookup methods
}
```

#### Merge Strategy
1. Match tracks by normalized USB path (case-insensitive)
2. Reuse existing track IDs for matching paths
3. Allocate new IDs starting from max(existing) + 1
4. Preserve artist/album/genre/key IDs by name matching

---

## Phase 3: Sync Diff Calculator

### File: `src-tauri/src/services/export/sync/mod.rs`

**Goal**: Calculate what needs to change between library and device.

#### Core Structures
```rust
pub enum TrackRemovalStrategy {
    RemoveOrphaned,   // Remove tracks not in any synced playlist
    Conservative,     // Only remove if deleted from library (DEFAULT)
    NeverRemove,      // Keep all tracks on device
}

pub enum TrackChangeType { Add, Update, Remove, Unchanged }

pub struct SyncDiff {
    pub tracks_to_add: Vec<(Track, String)>,      // (track, usb_path)
    pub tracks_to_update: Vec<(Track, String)>,   // (track, usb_path)
    pub tracks_to_remove: Vec<String>,            // usb_paths
    pub bytes_to_copy: u64,
    pub bytes_to_remove: u64,
}
```

#### Calculation Logic
1. Build set of tracks needed for current export
2. Compare with `device_tracks` table to find:
   - New tracks (in library, not on device)
   - Updated tracks (hash changed)
   - Removed tracks (based on removal strategy)

---

## Phase 4: Checkpoint System

### Database Schema (Migration 8)

Add to `src-tauri/src/db/schema.rs`:

```sql
CREATE TABLE export_checkpoints (
    id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,
    device_name TEXT NOT NULL,
    started_at TEXT NOT NULL,
    state TEXT NOT NULL,              -- 'copying' | 'generating_pdb'
    playlist_ids TEXT NOT NULL,       -- JSON array
    tracks_completed TEXT NOT NULL,   -- JSON array of track IDs
    tracks_failed TEXT NOT NULL,      -- JSON array of (track_id, error)
    last_updated_at TEXT NOT NULL
);

ALTER TABLE device_tracks ADD COLUMN metadata_hash TEXT;
```

### File: `src-tauri/src/services/export/checkpoint.rs`

```rust
pub struct ExportCheckpoint {
    pub id: String,
    pub device_id: String,
    pub state: CheckpointState,
    pub playlist_ids: Vec<String>,
    pub tracks_completed: Vec<String>,
    pub tracks_failed: Vec<(String, String)>,
}

pub enum CheckpointState {
    Copying { current_track_id: Option<String>, bytes_copied: u64 },
    GeneratingPdb,
}

pub struct CheckpointService {
    conn: Arc<Mutex<Connection>>,
}

impl CheckpointService {
    pub fn save_checkpoint(&self, checkpoint: &ExportCheckpoint) -> Result<()>;
    pub fn get_pending_checkpoint(&self, device_id: &str) -> Result<Option<ExportCheckpoint>>;
    pub fn complete_checkpoint(&self, checkpoint_id: &str) -> Result<()>;
    pub fn mark_track_completed(&self, checkpoint_id: &str, track_id: &str) -> Result<()>;
}
```

---

## Phase 5: Parallel Track Copying

### Implementation in `src-tauri/src/services/export/mod.rs`

**Goal**: Copy tracks in parallel with configurable concurrency.

```rust
async fn copy_tracks_parallel(
    &self,
    app_handle: &AppHandle,
    mount_point: &str,
    tracks: &[Track],
    device_id: &str,
    checkpoint_id: &str,
    max_concurrency: usize,  // Default: 4
) -> Result<Vec<DeviceTrack>> {
    let semaphore = Arc::new(Semaphore::new(max_concurrency));
    let mut join_set = JoinSet::new();

    for track in tracks {
        // Spawn copy task with semaphore permit
        // Update checkpoint after each completion
        // Emit progress events
    }

    // Collect results, handle failures
}
```

### Configuration
Add to settings:
```rust
pub parallel_copy_threads: usize,  // Default: 4
pub removal_strategy: TrackRemovalStrategy,  // Default: Conservative
```

---

## Phase 6: Resume Command

### New Commands in `src-tauri/src/commands/export.rs`

```rust
#[tauri::command]
pub async fn resume_export(
    device_id: String,
    mount_point: String,
    export_service: State<'_, Arc<ExportService>>,
    app_handle: AppHandle,
) -> Result<ExportResult>;

#[tauri::command]
pub async fn get_pending_checkpoint(
    device_id: String,
    checkpoint_service: State<'_, Arc<CheckpointService>>,
) -> Result<Option<ExportCheckpoint>>;

#[tauri::command]
pub async fn delete_checkpoint(
    checkpoint_id: String,
    checkpoint_service: State<'_, Arc<CheckpointService>>,
) -> Result<()>;
```

### Frontend Integration

Add to `src/lib/api/export.ts`:
```typescript
export async function resumeExport(deviceId: string, mountPoint: string): Promise<ExportResult>;
export async function getPendingCheckpoint(deviceId: string): Promise<ExportCheckpoint | null>;
```

Update `ExportFailureModal.svelte` to show resume option.

---

## Files Summary

### Create
| File | Purpose |
|------|---------|
| `src-tauri/src/services/export/pdb/reader.rs` | PDB file parsing |
| `src-tauri/src/services/export/pdb/merger.rs` | Data merging logic |
| `src-tauri/src/services/export/sync/mod.rs` | Sync diff calculator |
| `src-tauri/src/services/export/checkpoint.rs` | Checkpoint management |

### Modify
| File | Changes |
|------|---------|
| `src-tauri/src/services/export/pdb/strings.rs` | Add `BinRead` / `read_from()` |
| `src-tauri/src/services/export/pdb/mod.rs` | Export new modules |
| `src-tauri/src/services/export/mod.rs` | Integrate sync, parallel copy, checkpoints |
| `src-tauri/src/db/schema.rs` | Add migration 8 (checkpoints table) |
| `src-tauri/src/commands/export.rs` | Add resume/checkpoint commands |
| `src-tauri/src/lib.rs` | Register new commands |
| `src-tauri/src/models/export.rs` | Add checkpoint types |
| `src/lib/api/export.ts` | Add resume/checkpoint API |
| `src/lib/types/index.ts` | Add checkpoint types |
| `src/lib/components/export/ExportFailureModal.svelte` | Add resume option |
| `src/lib/i18n/locales/*.json` | Add new strings |

---

## Implementation Order

1. **PDB Reader** (largest piece)
   - Add string reading to `DeviceSQLString`
   - Implement page traversal
   - Parse all table types

2. **PDB Merger**
   - Implement ID preservation logic
   - Integrate with `PdbWriter.from_existing()`

3. **Sync Diff Calculator**
   - Implement diff logic
   - Connect to export flow

4. **Checkpoint System**
   - Add database migration
   - Implement checkpoint service
   - Integrate into copy flow

5. **Parallel Copying**
   - Convert to async with tokio
   - Add semaphore-based concurrency

6. **Frontend Updates**
   - Add resume API
   - Update failure modal
   - Add locale strings

---

## Testing Strategy

1. **Unit Tests**: DeviceSQLString round-trip, sync diff edge cases
2. **Integration**: Export → read back → verify data integrity
3. **Device Testing**: Test on actual CDJ/XDJ hardware
