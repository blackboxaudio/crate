# Crate - Technical Requirements Document

**Status:** Finalized
**Version:** 1.0
**Last Updated:** 2025-12-18

---

## 1. Tech Stack

| Component | Technology | Rationale |
|-----------|------------|-----------|
| Framework | Tauri 2.0 | Cross-platform, Rust backend, small binary size |
| Backend | Rust | Performance, safety, excellent audio libraries |
| Frontend | Svelte 5 | Reactive (runes), fast, small bundle, great DX |
| Database | SQLite | Local-first, portable, proven reliability |
| Audio Playback | rodio | High-level API, easy device handling |
| Audio Decoding | symphonia | Pure Rust, all DJ formats (MP3, WAV, AIFF, FLAC, M4A) |
| Metadata | lofty | Unified API for ID3v2, Vorbis, MP4 atoms |
| BPM Analysis | aubio-rs | Industry-standard beat tracking |
| XML Handling | quick-xml | Read/write Rekordbox XML format |
| File Watching | notify | Cross-platform, async-friendly |
| Styling | Tailwind CSS | Utility-first, consistent design system |
| UI Components | Fully custom | Full control, DJ-specific needs |

**Target Platforms:** macOS, Windows

---

## 2. Project Structure

```
crate/
├── src-tauri/                    # Rust backend (Tauri)
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   ├── main.rs               # Tauri entry point
│   │   ├── lib.rs                # Module exports
│   │   ├── commands/             # Tauri IPC commands
│   │   │   ├── mod.rs
│   │   │   ├── library.rs        # Track import, metadata
│   │   │   ├── playlist.rs       # Playlist CRUD
│   │   │   ├── tag.rs            # Tag management
│   │   │   ├── playback.rs       # Audio playback controls
│   │   │   ├── analysis.rs       # BPM/key analysis
│   │   │   └── export.rs         # Rekordbox/iTunes export
│   │   ├── services/             # Business logic
│   │   │   ├── mod.rs
│   │   │   ├── library.rs
│   │   │   ├── playlist.rs
│   │   │   ├── tag.rs
│   │   │   ├── audio.rs          # Playback engine
│   │   │   ├── analysis.rs       # Audio analysis
│   │   │   ├── waveform.rs       # Waveform generation
│   │   │   └── file_watcher.rs   # Watch for new files
│   │   ├── importers/            # External format importers
│   │   │   ├── mod.rs
│   │   │   ├── rekordbox.rs      # Rekordbox DB/XML import
│   │   │   └── id3.rs            # ID3/Vorbis tag reading
│   │   ├── exporters/            # External format exporters
│   │   │   ├── mod.rs
│   │   │   ├── rekordbox.rs      # Rekordbox XML export
│   │   │   ├── pioneer_usb.rs    # Pioneer USB format
│   │   │   └── itunes.rs         # iTunes XML export
│   │   ├── models/               # Data structures
│   │   │   ├── mod.rs
│   │   │   ├── track.rs
│   │   │   ├── playlist.rs
│   │   │   ├── tag.rs
│   │   │   ├── cue.rs
│   │   │   └── smart_playlist.rs
│   │   ├── db/                   # Database layer
│   │   │   ├── mod.rs
│   │   │   ├── schema.rs         # SQLite schema
│   │   │   ├── migrations/       # DB migrations
│   │   │   └── queries.rs        # SQL queries
│   │   └── error.rs              # Error types
│   └── icons/                    # App icons
│
├── src/                          # Svelte frontend
│   ├── app.html
│   ├── app.css                   # Global styles + Tailwind
│   ├── lib/
│   │   ├── components/           # Reusable UI components
│   │   │   ├── TrackList.svelte
│   │   │   ├── TrackRow.svelte
│   │   │   ├── Waveform.svelte
│   │   │   ├── Player.svelte
│   │   │   ├── Sidebar.svelte
│   │   │   ├── TagChip.svelte
│   │   │   ├── TagEditor.svelte
│   │   │   ├── SearchBar.svelte
│   │   │   ├── PlaylistTree.svelte
│   │   │   └── SmartPlaylistEditor.svelte
│   │   ├── stores/               # Svelte stores (state)
│   │   │   ├── library.ts        # Track list state
│   │   │   ├── player.ts         # Playback state
│   │   │   ├── playlist.ts       # Playlist state
│   │   │   ├── tags.ts           # Tag state
│   │   │   └── ui.ts             # UI state (selection, etc.)
│   │   ├── api/                  # Tauri command wrappers
│   │   │   ├── library.ts
│   │   │   ├── playlist.ts
│   │   │   ├── tag.ts
│   │   │   ├── player.ts
│   │   │   └── export.ts
│   │   ├── utils/                # Helper functions
│   │   │   ├── format.ts         # Time, BPM formatting
│   │   │   ├── key.ts            # Key/Camelot conversion
│   │   │   └── search.ts         # Search logic
│   │   └── types/                # TypeScript types
│   │       └── index.ts
│   └── routes/                   # SvelteKit routes
│       ├── +layout.svelte        # Main layout
│       └── +page.svelte          # Main page
│
├── docs/                         # Documentation
│   ├── PRD.md
│   ├── TRD.md
│   └── CLAUDE_IMPLEMENTATION_GUIDE.md
│
├── static/                       # Static assets
├── package.json
├── svelte.config.js
├── tailwind.config.js
├── tsconfig.json
└── vite.config.ts
```

---

## 3. Rust Dependencies (Cargo.toml)

```toml
[package]
name = "crate-app"
version = "0.1.0"
edition = "2021"

[dependencies]
# Tauri
tauri = { version = "2", features = ["devtools"] }
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"
tauri-plugin-shell = "2"

# Database
rusqlite = { version = "0.31", features = ["bundled"] }

# Audio
rodio = "0.19"
symphonia = { version = "0.5", features = ["all"] }
lofty = "0.21"

# Analysis
aubio = "0.2"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# XML handling
quick-xml = { version = "0.36", features = ["serialize"] }

# Async
tokio = { version = "1", features = ["full"] }

# File watching
notify = "6"

# Utilities
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1"
log = "0.4"
env_logger = "0.11"
walkdir = "2"
```

---

## 4. SQLite Schema

```sql
-- Core tables
CREATE TABLE tracks (
    id TEXT PRIMARY KEY,          -- UUID
    file_path TEXT NOT NULL UNIQUE,
    file_hash TEXT,               -- For duplicate detection

    -- Metadata (from ID3/Vorbis)
    title TEXT,
    artist TEXT,
    album TEXT,
    year INTEGER,
    genre TEXT,
    label TEXT,
    catalog_number TEXT,

    -- Audio properties
    duration_ms INTEGER NOT NULL,
    bpm REAL,
    key TEXT,                     -- e.g., "8A" (Camelot) or "Am"
    bitrate INTEGER,
    sample_rate INTEGER,
    format TEXT,                  -- mp3, flac, wav, etc.

    -- Analysis metadata
    analysis_source TEXT,         -- 'rekordbox', 'native', 'manual'
    waveform_data BLOB,           -- Compressed waveform

    -- User data
    rating INTEGER DEFAULT 0,     -- 0-5 stars
    play_count INTEGER DEFAULT 0,

    -- Timestamps
    date_added TEXT NOT NULL,     -- ISO 8601
    date_modified TEXT NOT NULL,
    last_played TEXT,

    -- Rekordbox sync
    rekordbox_id TEXT,

    CONSTRAINT valid_rating CHECK (rating >= 0 AND rating <= 5)
);

CREATE INDEX idx_tracks_artist ON tracks(artist);
CREATE INDEX idx_tracks_bpm ON tracks(bpm);
CREATE INDEX idx_tracks_key ON tracks(key);
CREATE INDEX idx_tracks_date_added ON tracks(date_added);

-- Tag system
CREATE TABLE tag_categories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE tags (
    id TEXT PRIMARY KEY,
    category_id TEXT NOT NULL REFERENCES tag_categories(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    color TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    UNIQUE(category_id, name)
);

CREATE TABLE track_tags (
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (track_id, tag_id)
);

-- Playlists
CREATE TABLE playlists (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    parent_id TEXT REFERENCES playlists(id) ON DELETE CASCADE,
    is_folder BOOLEAN NOT NULL DEFAULT FALSE,
    is_smart BOOLEAN NOT NULL DEFAULT FALSE,
    smart_rules TEXT,             -- JSON
    sort_order INTEGER NOT NULL DEFAULT 0,
    date_created TEXT NOT NULL,
    date_modified TEXT NOT NULL
);

CREATE TABLE playlist_tracks (
    playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    date_added TEXT NOT NULL,
    PRIMARY KEY (playlist_id, track_id)
);

-- Cue points
CREATE TABLE cues (
    id TEXT PRIMARY KEY,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position_ms INTEGER NOT NULL,
    type TEXT NOT NULL,           -- 'memory', 'hot', 'loop'
    loop_end_ms INTEGER,
    hot_cue_index INTEGER,        -- 0-7 for hot cues
    name TEXT,
    color TEXT,
    CONSTRAINT valid_type CHECK (type IN ('memory', 'hot', 'loop')),
    CONSTRAINT loop_has_end CHECK (type != 'loop' OR loop_end_ms IS NOT NULL)
);

CREATE INDEX idx_cues_track ON cues(track_id);

-- App settings
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

---

## 5. Tauri Commands (IPC API)

### Library Commands
```rust
#[tauri::command]
async fn import_tracks(paths: Vec<PathBuf>) -> Result<Vec<Track>, Error>;

#[tauri::command]
async fn import_rekordbox(db_path: PathBuf) -> Result<ImportResult, Error>;

#[tauri::command]
async fn get_tracks(offset: u32, limit: u32, filter: Option<TrackFilter>) -> Result<Vec<Track>, Error>;

#[tauri::command]
async fn get_track(id: Uuid) -> Result<Track, Error>;

#[tauri::command]
async fn update_track(id: Uuid, updates: TrackUpdate) -> Result<Track, Error>;

#[tauri::command]
async fn delete_tracks(ids: Vec<Uuid>) -> Result<(), Error>;

#[tauri::command]
async fn search_tracks(query: String) -> Result<Vec<Track>, Error>;
```

### Playlist Commands
```rust
#[tauri::command]
async fn create_playlist(name: String, parent_id: Option<Uuid>) -> Result<Playlist, Error>;

#[tauri::command]
async fn create_smart_playlist(name: String, rules: SmartPlaylistRules) -> Result<Playlist, Error>;

#[tauri::command]
async fn get_playlists() -> Result<Vec<Playlist>, Error>;

#[tauri::command]
async fn get_playlist_tracks(id: Uuid) -> Result<Vec<Track>, Error>;

#[tauri::command]
async fn add_to_playlist(playlist_id: Uuid, track_ids: Vec<Uuid>) -> Result<(), Error>;

#[tauri::command]
async fn remove_from_playlist(playlist_id: Uuid, track_ids: Vec<Uuid>) -> Result<(), Error>;

#[tauri::command]
async fn reorder_playlist(playlist_id: Uuid, track_ids: Vec<Uuid>) -> Result<(), Error>;

#[tauri::command]
async fn delete_playlist(id: Uuid) -> Result<(), Error>;
```

### Tag Commands
```rust
#[tauri::command]
async fn get_tag_categories() -> Result<Vec<TagCategory>, Error>;

#[tauri::command]
async fn create_tag_category(name: String) -> Result<TagCategory, Error>;

#[tauri::command]
async fn create_tag(category_id: Uuid, name: String, color: Option<String>) -> Result<Tag, Error>;

#[tauri::command]
async fn assign_tags(track_ids: Vec<Uuid>, tag_ids: Vec<Uuid>) -> Result<(), Error>;

#[tauri::command]
async fn remove_tags(track_ids: Vec<Uuid>, tag_ids: Vec<Uuid>) -> Result<(), Error>;
```

### Playback Commands
```rust
#[tauri::command]
async fn play_track(id: Uuid) -> Result<(), Error>;

#[tauri::command]
async fn pause() -> Result<(), Error>;

#[tauri::command]
async fn resume() -> Result<(), Error>;

#[tauri::command]
async fn seek(position_ms: u64) -> Result<(), Error>;

#[tauri::command]
async fn get_playback_state() -> Result<PlaybackState, Error>;

#[tauri::command]
async fn set_volume(volume: f32) -> Result<(), Error>;
```

### Export Commands
```rust
#[tauri::command]
async fn export_rekordbox_xml(playlist_ids: Vec<Uuid>, path: PathBuf) -> Result<(), Error>;

#[tauri::command]
async fn export_pioneer_usb(playlist_ids: Vec<Uuid>, usb_path: PathBuf) -> Result<ExportResult, Error>;

#[tauri::command]
async fn export_itunes_xml(playlist_ids: Vec<Uuid>, path: PathBuf) -> Result<(), Error>;
```

---

## 6. Frontend State Management (Svelte Stores)

```typescript
// stores/library.ts
import { writable, derived } from 'svelte/store';

interface LibraryState {
  tracks: Track[];
  loading: boolean;
  filter: TrackFilter | null;
  sortBy: keyof Track;
  sortOrder: 'asc' | 'desc';
}

export const library = writable<LibraryState>({
  tracks: [],
  loading: false,
  filter: null,
  sortBy: 'date_added',
  sortOrder: 'desc'
});

export const filteredTracks = derived(library, $lib => {
  // Apply filters and sorting
});

// stores/player.ts
interface PlayerState {
  currentTrack: Track | null;
  isPlaying: boolean;
  position: number;
  duration: number;
  volume: number;
  waveformData: number[] | null;
}

export const player = writable<PlayerState>({...});

// stores/ui.ts
interface UIState {
  selectedTrackIds: Set<string>;
  selectedPlaylistId: string | null;
  sidebarWidth: number;
  searchQuery: string;
}

export const ui = writable<UIState>({...});
```

---

## 7. Error Handling

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrateError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Audio error: {0}")]
    Audio(String),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Import error: {0}")]
    Import(String),

    #[error("Export error: {0}")]
    Export(String),

    #[error("Analysis error: {0}")]
    Analysis(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

impl serde::Serialize for CrateError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(&self.to_string())
    }
}
```

---

## 8. Key Technical Decisions

| Area | Decision | Rationale |
|------|----------|-----------|
| ID Generation | UUID v4 | Globally unique, no DB coordination |
| Timestamps | ISO 8601 strings | Human readable, SQLite friendly |
| Waveform Storage | BLOB in DB | Fast access, ~50KB per track |
| Smart Playlist Rules | JSON column | Flexible schema, easy to evolve |
| Waveform Rendering | Canvas 2D | Simple, performant for static waveforms |
| USB Export | Direct Pioneer format | No Rekordbox middleman |

---

## 9. Performance Considerations

1. **Virtual scrolling** for track list (handle 20K+ tracks)
2. **Lazy waveform loading** - only load visible tracks
3. **Background analysis** - don't block UI during import
4. **Debounced search** - avoid excessive queries
5. **Indexed columns** - BPM, key, artist for fast filtering
6. **Connection pooling** - reuse SQLite connections

---

## 10. Testing Strategy

| Layer | Approach | Tools |
|-------|----------|-------|
| Rust unit tests | Test services, parsers | `cargo test` |
| Rust integration | Test DB operations | SQLite in-memory |
| Frontend unit | Test stores, utils | Vitest |
| Frontend component | Test UI components | Svelte Testing Library |
| E2E | Critical user flows | Playwright |

---

## 11. External Resources

- [crate-digger](https://github.com/Deep-Symmetry/crate-digger) - Pioneer database format documentation
- [Rekordbox XML format](https://cdn.rekordbox.com/files/20200410160904/xml_format_list.pdf) - Official XML spec
- [Tauri 2.0 Docs](https://v2.tauri.app/) - Framework documentation
- [Svelte 5 Docs](https://svelte.dev/docs) - Frontend framework
