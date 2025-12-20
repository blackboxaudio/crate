---
title: Library Management
description: Import, organize, and manage your track collection
---

Your library is the central hub for all your audio files. Crate automatically extracts metadata and keeps track of your collection.

## Importing Tracks

### Drag and Drop

The easiest way to add tracks is to drag audio files or folders directly onto the Crate window.

1. Open your file manager
2. Select files or folders containing audio
3. Drag them onto the Crate window
4. Crate will process and import the files

### Import Dialog

For more control, use the import dialog:

1. Click **Import** in the toolbar
2. Select files or folders in the dialog
3. Click **Open** to begin the import

### Supported Formats

Crate supports common audio formats:

| Format | Extensions |
|--------|------------|
| MP3 | `.mp3` |
| FLAC | `.flac` |
| WAV | `.wav` |
| AIFF | `.aiff`, `.aif` |
| AAC | `.m4a`, `.aac` |

## Automatic Metadata Extraction

When you import tracks, Crate automatically extracts:

- **Title** - Track name
- **Artist** - Performer
- **Album** - Album name
- **Year** - Release year
- **Genre** - Music genre
- **Label** - Record label
- **Catalog Number** - Label catalog ID
- **BPM** - Beats per minute
- **Key** - Musical key
- **Duration** - Track length
- **Bitrate** - Audio quality
- **Sample Rate** - Audio sample rate
- **Album Artwork** - Embedded cover art

Metadata is read from embedded ID3 tags (MP3) or equivalent metadata containers for other formats.

## Handling Duplicates

When importing a track that already exists in your library (detected by file hash), Crate offers options:

| Option | Effect |
|--------|--------|
| **Skip** | Keep the existing track, ignore the new file |
| **Update Path** | Keep existing metadata, update the file path |
| **Replace** | Replace the existing track with the new file |

This prevents accidental duplicates while giving you control over updates.

## Missing Tracks

If a file is moved or deleted outside of Crate, the track becomes "missing." Crate tracks this so you can:

- See which tracks are missing
- Relocate tracks to their new location
- Remove orphaned entries from your library

### Relocating a Missing Track

1. Right-click the missing track
2. Select **Relocate**
3. Navigate to the file's new location
4. Select the file and click **Open**

The track's metadata is preserved; only the file path is updated.

## File Integrity

Crate uses BLAKE3 hashing to verify file integrity. Each track's hash is stored when imported, allowing Crate to:

- Detect duplicate files accurately
- Identify when files have been modified
- Verify file integrity over time

## Track Properties

Each track in your library has properties you can view and edit:

### Viewable Properties

| Property | Description |
|----------|-------------|
| Title | Track name |
| Artist | Performer |
| Album | Album name |
| Duration | Length (MM:SS) |
| BPM | Tempo |
| Key | Musical key |
| Genre | Music style |
| Year | Release year |
| Label | Record label |
| Catalog # | Label catalog number |
| Bitrate | Audio quality (kbps) |
| Sample Rate | Audio sample rate (Hz) |
| Format | File format |
| Rating | Star rating |
| Play Count | Times played |
| Date Added | When imported |
| Last Played | Most recent play |

### Editable Properties

You can edit these properties in the Inspector panel:

- Title, Artist, Album
- Year, Genre, Label
- BPM, Key
- Rating
- Color
- Tags

## Track Colors

Assign colors to tracks for visual organization:

- **Pink**
- **Red**
- **Orange**
- **Yellow**
- **Green**
- **Aqua**
- **Blue**
- **Purple**

To set a color:
1. Select one or more tracks
2. Right-click and choose **Set Color**
3. Select a color

Colors appear as a dot in the track list for quick identification.

## Removing Tracks

To remove tracks from your library:

1. Select the tracks to remove
2. Right-click and choose **Remove from Library**
3. Confirm the action

This removes the tracks from Crate's database but does not delete the actual files from disk.
