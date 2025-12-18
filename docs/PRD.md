# Crate - Product Requirements Document

**Status:** Finalized
**Version:** 1.0
**Last Updated:** 2025-12-18

---

## 1. Vision & Problem Statement

### Vision
A fast, intuitive audio library management application designed for DJs who demand precise organization, flexible tagging, and seamless interoperability with professional DJ software.

### Problem Statement
Existing DJ software like Rekordbox prioritizes performance features over library management. DJs with carefully curated collections face:
- Slow, clunky interfaces that frustrate daily use
- Rigid tagging systems that don't match personal workflows
- Poor playlist management and organization tools
- Vendor lock-in with limited export options

### Target User
Techno DJ with a curated library of 1,000-5,000 tracks who:
- Invests significant effort in music organization
- Uses custom categorization systems (hybrid genre/energy/functional tags)
- Performs on Pioneer equipment (CDJs, XDJs)
- Wants library accessible on mobile via Apple Music
- Values speed, keyboard shortcuts, and efficient workflows

---

## 2. Core Features

### 2.1 Library Management
- **Supported formats**: MP3, WAV, AIFF, FLAC, M4A/AAC
- **Drag-and-drop import** onto track list (single files or folders)
- **Bulk import** with folder structure preservation
- **File watching** for automatic detection of new tracks
- **Duplicate detection** based on audio fingerprinting or metadata

### 2.2 Metadata & Tagging

**Standard Fields:**
- Artist, Title, Album, Year
- BPM, Key (Camelot and standard notation)
- Genre, Label, Catalog Number
- Duration, Bitrate, File Format

**Custom Tagging System (MyTags):**
- User-defined tag categories
- Multiple tags per category per track
- Hierarchical tags (e.g., Techno > Acid > 303-heavy)
- Color coding for visual identification
- Quick-tag keyboard shortcuts

**MyTag Categories (4 max, matching Rekordbox limitation):**

| Category | Description | Example Values |
|----------|-------------|----------------|
| **Style** | Genre keywords (subjective + objective) | Industrial, Organic, Acid, Dub, Hard, Minimal |
| **Mood** | Subjective feeling/vibe of track | Chaotic, Dreamy, Eerie, Party, Euphoric |
| **Characteristics** | Objective elements in the track | 808, Amen, Narrator, Vocal, Synth-heavy |
| **Situation** | Energy level + functional context | Blue/Green/Yellow/Orange/Red/Pink (energy), Intro, Closer, Curveball |

**Tag System Requirements:**
- Maximum 4 top-level categories (matching Rekordbox MyTag)
- Unlimited values per category
- Full CRUD: add, remove, rename categories and values
- Color coding for energy levels and visual scanning
- Drag-and-drop tag assignment

### 2.3 Playlist Management
- **Standard playlists** - manual track ordering
- **Smart playlists** - dynamic, rule-based filtering
  - Filter by: BPM range, key, tags, genre, date added, rating, play count
  - Combine multiple conditions with AND/OR logic
  - Auto-update when library changes
- **Playlist folders** for hierarchical organization
- **Drag-and-drop** reordering
- **Playlist history** - track changes over time

### 2.4 Audio Preview & Playback
- **Waveform display** (overview and zoomed)
- **Playback controls** - play, pause, seek, loop
- **Memory cues** - visual markers on waveform
  - Support for loop cues (auto-loop at track end)
  - Import cues from Rekordbox
- **Hot cue support** - quick jump points
- **Keyboard-driven navigation** - cue to cue, quick preview

### 2.5 Audio Analysis
- **Hybrid approach:**
  - Import existing analysis from Rekordbox (BPM, key, beatgrid, cues)
  - Independent analysis engine for new tracks
- **BPM detection** using aubio or bliss-rs
- **Key detection** (Camelot wheel display)
- **Waveform generation** from audio samples
- **Manual override** for all analysis values

### 2.6 Search & Filter
- **Global search** across all metadata fields
- **Faceted filtering** - narrow by multiple criteria simultaneously
- **Saved searches** - quick access to common queries
- **Column sorting** - click to sort by any field
- **Keyboard-first** - type to search, arrow keys to navigate

---

## 3. Interoperability Requirements

### 3.1 Rekordbox Integration (Priority: Critical)

**Import:**
- Read Rekordbox SQLite database (`master.db`)
- Parse Rekordbox XML export format
- Import: tracks, playlists, cue points, beatgrids, analysis data, ratings

**Export:**
- Generate Rekordbox-compatible XML for import
- Export to USB in Rekordbox format for CDJ/XDJ playback
- Preserve: playlist structure, cue points, beatgrids, hot cues

**USB Export (Pioneer Pro DJ Link):**
- Write `PIONEER` folder structure
- Generate device database
- Support for CDJ-2000NXS2, CDJ-3000, XDJ-XZ, etc.

### 3.2 Apple Music / iTunes Integration (Priority: High)

**Export:**
- Generate iTunes XML Library format
- Create playlists that appear in Apple Music
- Preserve metadata (limited by iTunes format)

**Workflow:**
- Export selected playlists to iTunes
- Sync to iPhone via Apple Music
- Use for casual listening / track discovery on-the-go

### 3.3 Future: Traktor Support (Priority: Low)
- Design data model to accommodate Traktor's NML format
- Plugin architecture for additional export formats

---

## 4. User Interface Concepts

### 4.1 Main Layout

```
┌──────────────────────────────────────────────────────────────────┐
│  [Toolbar: Search | Import | Export | Settings]                  │
├────────────────┬─────────────────────────────────────────────────┤
│                │                                                 │
│   Sidebar      │              Track List                         │
│                │  ┌─────┬────────┬────────┬─────┬─────┬───────┐  │
│   • Library    │  │ Art │ Title  │ Artist │ BPM │ Key │ Tags  │  │
│   • Playlists  │  ├─────┼────────┼────────┼─────┼─────┼───────┤  │
│     ├─ Set 1   │  │     │        │        │     │     │       │  │
│     ├─ Set 2   │  │     │        │        │     │     │       │  │
│     └─ ...     │  │     │        │        │     │     │       │  │
│   • Smart PL   │  └─────┴────────┴────────┴─────┴─────┴───────┘  │
│   • Tags       │                                                 │
│                │                                                 │
├────────────────┴─────────────────────────────────────────────────┤
│  [Player: Waveform] [◄◄] [▶/❚❚] [►►] [Cues: 1 2 3 4 5 6 7 8]    │
│  Track: Artist - Title                            00:00 / 06:30  │
└──────────────────────────────────────────────────────────────────┘
```

### 4.2 Key UI Principles
- **Keyboard-first**: All common actions have shortcuts
- **Fast search**: Instant filtering as you type
- **Minimal clicks**: Inline editing, context menus, drag-and-drop
- **Visual tags**: Color-coded chips for quick scanning
- **Information density**: Show more tracks, less chrome

---

## 5. MVP Scope

### Phase 1: Foundation (MVP)
1. Library import - add tracks from filesystem (drag-and-drop)
2. Basic metadata display - read existing ID3/Vorbis tags
3. Manual tagging - create tags, assign to tracks
4. Basic playlists - create, add tracks, reorder
5. Audio preview - simple playback, seek
6. Search/filter - find tracks by any field

### Phase 2: Rekordbox Integration
1. Import from Rekordbox - tracks, playlists, cues, analysis
2. Export to Rekordbox XML - playlists and metadata
3. USB export - Pioneer-compatible format (direct)

### Phase 3: Advanced Features
1. Smart playlists - rule-based dynamic playlists
2. Waveform display - visual cue management
3. Native audio analysis - BPM/key detection for new tracks
4. iTunes export - Apple Music integration

### Phase 4: Polish & Future
1. Performance optimization - handle large libraries
2. Cloud sync foundation - prepare for multi-device
3. Plugin architecture - extensible export formats
4. Mobile companion (future consideration)

---

## 6. Success Criteria

An MVP is successful when you can:
- [ ] Import your existing library (files + Rekordbox data)
- [ ] Apply your MyTag system to tracks
- [ ] Create and manage playlists
- [ ] Preview tracks with waveform
- [ ] Export playlists back to Rekordbox for USB export
- [ ] Search/filter to quickly find tracks for a set

---

## 7. Resolved Decisions

| Decision | Resolution |
|----------|------------|
| App Name | **Crate** |
| USB Export | Direct USB export (Pioneer format) - no Rekordbox middleman |
| Tag Categories | 4 max: Style, Mood, Characteristics, Situation |
| Analysis | Hybrid: Import Rekordbox + native engine for new tracks |
| Target Platforms | macOS + Windows |
