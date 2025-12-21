---
title: FAQ
description: Frequently asked questions and troubleshooting
---

Common questions and solutions for Crate users.

## General

### What audio formats does Crate support?

Crate supports:
- MP3 (`.mp3`)
- FLAC (`.flac`)
- WAV (`.wav`)
- AIFF (`.aiff`, `.aif`)
- AAC (`.m4a`, `.aac`)

### Where is my library data stored?

Your library is stored in a local SQLite database in Crate's data directory. You can find the exact path in **Settings > About**.

Typical locations:
- **macOS**: `~/Library/Application Support/com.crate.app/`
- **Windows**: `%APPDATA%\crate\`
- **Linux**: `~/.local/share/crate/`

### Does Crate modify my audio files?

No. Crate reads metadata from your files but never writes to them. All changes (tags, ratings, colors, playlists) are stored in Crate's database only.

### Is there cloud sync?

No. Crate is local-first by design. Your library exists only on your machine. This means:
- Complete privacy
- No internet required
- No subscription fees
- No risk of cloud service changes

### Can I use Crate with DJ software?

Yes. Crate is a library manager, not DJ software. Use it to organize your collection, then play tracks in your preferred DJ application.

You can:
- Preview tracks in Crate
- Create playlists and tag tracks
- Export or drag tracks to your DJ software

## Import & Library

### Why are some files not importing?

Common reasons:
- **Unsupported format** - Check the file extension
- **Corrupted file** - The file may be damaged
- **Permission issues** - Crate needs read access to the file

### How do I handle duplicate tracks?

When importing a duplicate, Crate offers:
- **Skip** - Keep existing, ignore new
- **Update Path** - Update file location
- **Replace** - Replace with new file

### What happens to missing tracks?

If you move or delete files outside Crate, tracks become "missing." You can:
- Relocate them to the new path
- Remove them from the library

Crate doesn't automatically delete database entries when files disappear.

### Can I import from external drives?

Yes. Import from any mounted drive. Note that if the drive is disconnected, tracks from it will show as missing.

## Playback

### No sound is playing

1. Check the volume slider in the player
2. Verify the output device in **Settings > Sound**
3. Check your system volume
4. Try selecting a different audio device
5. Restart Crate

### Audio is stuttering

1. Close other audio applications
2. Try a different audio output device
3. Restart Crate
4. Check your system audio settings

### Can I route audio to a specific device?

Yes. Go to **Settings > Sound** and select your preferred output device. This is useful for routing to headphones while another app uses speakers.

## Organization

### What's the difference between tags and playlists?

| Feature | Tags | Playlists |
|---------|------|-----------|
| Purpose | Describe attributes | Curate specific tracks |
| Filter | Dynamic filtering | Fixed track list |
| Order | No order | Ordered list |

Use tags for attributes (genre, energy, mood). Use playlists for curated collections.

### Can I nest folders?

Yes. Drag folders into other folders to create a hierarchy. There's no depth limit.

### How do I organize large collections?

Recommended approach:
1. Use **folders** for broad categories (genres, years, projects)
2. Use **playlists** for specific sets or selections
3. Use **tags** for cross-cutting attributes (energy, mood, venue)
4. Combine all three for maximum flexibility

## Performance

### Crate is slow with large libraries

Crate is optimized for large libraries, but if you experience slowness:
1. Ensure you're running the latest version
2. Check if search or filters are active
3. Close other resource-intensive applications

### Import is taking a long time

Large imports (thousands of files) take time for metadata extraction. This is normal. You can continue using Crate while import progresses.

## Troubleshooting

### Crate won't start

1. Check for error messages
2. Try deleting the settings file (see "Where is my library data stored?")
3. Reinstall Crate
4. Report the issue if it persists

### Settings aren't saving

Settings are saved automatically. If they're not persisting:
1. Check write permissions to the data directory
2. Ensure Crate closes normally (not force-quit)
3. Try resetting settings by deleting the settings file

### I found a bug

Please report issues on the [GitHub repository](https://github.com/blackboxaudio/crate/issues). Include:
- What you expected to happen
- What actually happened
- Steps to reproduce
- Your OS and Crate version

## Feature Requests

Have an idea for Crate? Submit feature requests on [GitHub](https://github.com/blackboxaudio/crate/issues). Please search existing issues first to avoid duplicates.
