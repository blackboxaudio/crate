# Crate - Claude Implementation Guide

This document provides phased implementation prompts for Claude sessions to build the Crate application. Each phase is designed to be completed in one or more sessions, with clear goals and acceptance criteria.

**Before starting any phase**: Read `docs/PRD.md` and `docs/TRD.md` to understand the full context.

---

## Phase 1: Foundation (MVP Core)

### Goal
Build basic library management without external integrations. Users should be able to import audio files, view them in a list, tag them, and create playlists.

### Prerequisites
- Tauri 2.0 + Svelte 5 + Tailwind CSS project initialized
- Rust toolchain installed
- Node.js installed

### Prompt for Claude

```
I'm building a DJ audio library management app called "Crate" using Tauri 2.0 (Rust backend) + Svelte 5 (frontend) + Tailwind CSS.

Please read the docs/PRD.md and docs/TRD.md files first.

For Phase 1, implement the following:

1. **SQLite Database Setup**
   - Create the database schema from TRD.md section 4
   - Set up migrations in src-tauri/src/db/
   - Initialize DB on app startup

2. **Track Import**
   - Tauri command: `import_tracks(paths: Vec<PathBuf>)`
   - Read ID3/Vorbis tags using `lofty` crate
   - Support formats: MP3, WAV, AIFF, FLAC, M4A
   - Store in SQLite database
   - Frontend: drag-and-drop onto track list

3. **Track List UI**
   - Display tracks in a table (title, artist, BPM, key, duration)
   - Column sorting (click header to sort)
   - Basic search/filter input
   - Multi-select with Shift+Click, Cmd+Click

4. **Audio Preview**
   - Tauri commands: `play_track`, `pause`, `resume`, `seek`, `set_volume`
   - Use `rodio` for playback, `symphonia` for decoding
   - Player panel at bottom with play/pause, seek bar, volume
   - Display current track info

5. **Tag System**
   - Create tag categories (max 4): Style, Mood, Characteristics, Situation
   - Create tags within categories (with optional color)
   - Assign/remove tags from tracks
   - Display tags as colored chips in track list
   - Sidebar section showing all tags (click to filter)

6. **Basic Playlists**
   - Create/rename/delete playlists
   - Add tracks to playlist (drag-and-drop or right-click menu)
   - Playlist tree in sidebar
   - Reorder tracks within playlist

Focus on clean architecture with clear separation between:
- `src-tauri/src/commands/` - Tauri IPC handlers
- `src-tauri/src/services/` - Business logic
- `src-tauri/src/models/` - Data structures
- `src-tauri/src/db/` - Database operations

For frontend:
- `src/lib/stores/` - Svelte stores for state
- `src/lib/components/` - Reusable components
- `src/lib/api/` - Tauri command wrappers
```

### Acceptance Criteria
- [ ] Can drag-drop MP3/FLAC/WAV/AIFF files onto app to import
- [ ] Track list displays with sortable columns
- [ ] Can search tracks by artist/title
- [ ] Can play/pause/seek through tracks
- [ ] Can create tag categories and tags
- [ ] Can assign tags to tracks
- [ ] Can filter tracks by clicking a tag
- [ ] Can create playlists and add tracks
- [ ] Data persists after app restart

---

## Phase 2: Rekordbox Integration

### Goal
Import existing Rekordbox library and export playlists directly to USB for Pioneer CDJs.

### Prompt for Claude

```
I'm continuing work on Crate, a DJ library app. Phase 1 (basic library management) is complete.

Please read docs/PRD.md and docs/TRD.md for full context.

For Phase 2, implement Rekordbox integration:

1. **Rekordbox XML Import**
   - Parse Rekordbox XML export format
   - Import: tracks, playlists, cue points (memory, hot, loop)
   - Map Rekordbox MyTags to Crate tag system
   - Import BPM, key, ratings
   - Reference: https://cdn.rekordbox.com/files/20200410160904/xml_format_list.pdf

2. **Rekordbox Database Import (Optional/Advanced)**
   - Read master.db SQLite database directly
   - Location: ~/Library/Pioneer/rekordbox/master.db (macOS)
   - Import analysis data, beatgrids

3. **Rekordbox XML Export**
   - Generate rekordbox.xml from Crate playlists
   - Preserve track metadata, cue points
   - Allow user to select which playlists to export

4. **Pioneer USB Export (Critical)**
   - Write directly to USB in Pioneer format
   - Create PIONEER/rekordbox/ folder structure
   - Generate device database files
   - Reference: https://github.com/Deep-Symmetry/crate-digger
   - Test with: CDJ-2000NXS2, CDJ-3000, XDJ-XZ formats

5. **Import UI**
   - Dialog to select Rekordbox XML or database
   - Progress indicator during import
   - Conflict resolution (track already exists)

6. **Export UI**
   - Select playlists to export
   - Choose destination (USB drive)
   - Progress indicator
   - Success/error feedback

Key files to create/modify:
- src-tauri/src/importers/rekordbox.rs
- src-tauri/src/exporters/rekordbox.rs
- src-tauri/src/exporters/pioneer_usb.rs
- src-tauri/src/commands/export.rs
```

### Acceptance Criteria
- [ ] Can import Rekordbox XML file
- [ ] Playlists and tracks appear in Crate
- [ ] Cue points are imported and visible
- [ ] MyTags map to Crate tags
- [ ] Can export playlists to Rekordbox XML
- [ ] Can export directly to USB for CDJ use
- [ ] USB works on real Pioneer CDJ/XDJ hardware

---

## Phase 3: Advanced UI

### Goal
Full-featured DJ library experience with waveforms, smart playlists, and professional UX.

### Prompt for Claude

```
I'm continuing work on Crate. Phases 1-2 are complete (library management + Rekordbox integration).

Please read docs/PRD.md and docs/TRD.md for full context.

For Phase 3, implement advanced UI features:

1. **Waveform Display**
   - Generate waveform data from audio (on import or on-demand)
   - Store in database as BLOB
   - Render waveform using Canvas 2D
   - Show overview waveform in track list row
   - Show detailed waveform in player panel
   - Display cue points as markers on waveform
   - Click waveform to seek

2. **Cue Point Management**
   - View/edit memory cues, hot cues, loop cues
   - Add/delete cue points from waveform view
   - Color coding for cue types
   - Import cues from Rekordbox (done in Phase 2)

3. **Smart Playlists**
   - Rule-based dynamic playlists
   - Conditions: BPM range, key, tags, genre, date added, rating
   - AND/OR logic for combining conditions
   - Auto-update when library changes
   - UI for building rules (SmartPlaylistEditor component)

4. **Virtual Scrolling**
   - Handle libraries with 20,000+ tracks
   - Only render visible rows
   - Use TanStack Virtual or custom implementation
   - Maintain selection state during scroll

5. **Keyboard Shortcuts**
   - Space: play/pause
   - Arrow keys: navigate track list
   - Enter: play selected track
   - Cmd+F: focus search
   - Delete: remove from playlist
   - Customizable shortcuts (settings)

6. **Performance Optimizations**
   - Debounced search (300ms delay)
   - Lazy load waveforms (only visible)
   - Background thread for heavy operations
   - Loading states for async operations

Key components to build:
- src/lib/components/Waveform.svelte
- src/lib/components/CueMarker.svelte
- src/lib/components/SmartPlaylistEditor.svelte
- src/lib/components/VirtualTrackList.svelte
```

### Acceptance Criteria
- [ ] Waveforms render for imported tracks
- [ ] Can see cue points on waveform
- [ ] Can create smart playlists with multiple conditions
- [ ] Smart playlists auto-update
- [ ] Track list scrolls smoothly with 10,000+ tracks
- [ ] Keyboard shortcuts work throughout app
- [ ] No UI blocking during heavy operations

---

## Phase 4: Analysis & Polish

### Goal
Independent audio analysis, iTunes export, and production-ready polish.

### Prompt for Claude

```
I'm finishing Crate. Phases 1-3 are complete.

Please read docs/PRD.md and docs/TRD.md for full context.

For Phase 4, implement analysis and polish:

1. **BPM Detection**
   - Use aubio-rs for beat tracking
   - Analyze new tracks that don't have BPM
   - Background analysis queue
   - Progress indicator
   - Manual BPM override

2. **Key Detection**
   - Detect musical key
   - Display in Camelot notation (8A, 11B, etc.)
   - Show harmonic mixing suggestions
   - Manual key override

3. **Waveform Generation**
   - Generate waveform from audio samples
   - Downsample for efficient storage
   - Store in database
   - Generate on import or on-demand

4. **iTunes/Apple Music Export**
   - Generate iTunes XML Library format
   - Create playlists that appear in Apple Music
   - Preserve metadata
   - File path handling

5. **Error Handling**
   - User-friendly error messages
   - Graceful degradation
   - Logging for debugging
   - Crash recovery (preserve data)

6. **Settings & Preferences**
   - Audio device selection
   - Library location
   - Import preferences (auto-analyze, etc.)
   - UI preferences (column visibility, etc.)
   - Keyboard shortcut customization

7. **Polish**
   - App icon and branding
   - Loading states and skeletons
   - Empty states (no tracks, no playlists)
   - Tooltips and help text
   - Animations (subtle, performant)

Key files:
- src-tauri/src/services/analysis.rs
- src-tauri/src/exporters/itunes.rs
- src/lib/components/Settings.svelte
```

### Acceptance Criteria
- [ ] New tracks get BPM detected automatically
- [ ] Key detection works accurately
- [ ] Waveforms generate for all imported tracks
- [ ] Can export to iTunes/Apple Music
- [ ] Errors display helpful messages to user
- [ ] Settings persist and apply correctly
- [ ] App feels polished and professional

---

## General Guidelines for All Phases

### Code Quality
- Write idiomatic Rust (no unwrap in production code)
- Use TypeScript strictly (no any)
- Add comments for complex logic
- Follow existing code patterns

### Testing
- Unit tests for Rust services
- Integration tests for DB operations
- Frontend tests with Vitest
- Manual testing on real hardware (Phase 2)

### Git Workflow
- Commit frequently with clear messages
- One feature per commit when possible
- Test before committing

### When Stuck
1. Check docs/PRD.md and docs/TRD.md
2. Look at existing code patterns
3. Search for similar implementations in the codebase
4. Ask user for clarification

---

## Quick Reference

### Key Files
| Purpose | Rust Path | Frontend Path |
|---------|-----------|---------------|
| Track data | src-tauri/src/models/track.rs | src/lib/types/index.ts |
| Library commands | src-tauri/src/commands/library.rs | src/lib/api/library.ts |
| Audio playback | src-tauri/src/services/audio.rs | src/lib/stores/player.ts |
| Database | src-tauri/src/db/ | - |
| Track list | - | src/lib/components/TrackList.svelte |
| Player | - | src/lib/components/Player.svelte |

### Tauri Command Pattern
```rust
// Backend
#[tauri::command]
async fn my_command(arg: String) -> Result<ReturnType, CrateError> {
    // Implementation
}

// Register in main.rs
.invoke_handler(tauri::generate_handler![my_command])
```

```typescript
// Frontend
import { invoke } from '@tauri-apps/api/core';

const result = await invoke<ReturnType>('my_command', { arg: 'value' });
```

### Svelte Store Pattern
```typescript
import { writable } from 'svelte/store';

export const myStore = writable<MyType>(initialValue);

// Update
myStore.update(current => ({ ...current, newProp: value }));

// Subscribe in component
$: console.log($myStore);
```
