---
title: Bulk Operations
description: Edit multiple tracks at once
---

Crate supports bulk operations for managing multiple tracks efficiently. Select multiple tracks and apply changes to all of them at once.

## Selecting Multiple Tracks

### Range Selection (Shift+Click)

Select a continuous range of tracks:
1. Click the first track
2. Hold **Shift**
3. Click the last track in the range
4. All tracks between are selected

### Individual Selection (Cmd/Ctrl+Click)

Toggle individual tracks in or out of the selection:
1. Click a track to start
2. Hold **Cmd** (macOS) or **Ctrl** (Windows/Linux)
3. Click additional tracks to add/remove them

### Select All (Cmd/Ctrl+A)

Select all visible tracks:
1. Press **Cmd+A** (macOS) or **Ctrl+A** (Windows/Linux)
2. All tracks in the current view are selected

### Clear Selection

Clear your selection:
- Press **Escape**
- Or press **Cmd+Shift+A** / **Ctrl+Shift+A**
- Or click an empty area

## Bulk Metadata Editing

Edit metadata for multiple tracks simultaneously.

### How It Works

1. Select multiple tracks
2. Open the Inspector (`Cmd+I` / `Ctrl+I`)
3. The Inspector shows fields for bulk editing
4. Change a field value
5. The change applies to all selected tracks

### Mixed Values

When selected tracks have different values for a field:
- The field shows a "mixed values" indicator
- Changing the field overwrites all selected tracks with the new value
- Leaving it unchanged preserves individual values

### Editable Fields

These fields can be bulk edited:

| Field | Description |
|-------|-------------|
| Title | Track name |
| Artist | Performer |
| Album | Album name |
| Year | Release year |
| Genre | Music genre |
| Label | Record label |
| BPM | Beats per minute |
| Key | Musical key |
| Rating | Star rating |
| Color | Track color |

## Bulk Tagging

Apply or remove tags from multiple tracks.

### Adding Tags

1. Select the tracks to tag
2. Right-click and choose **Tags**
3. Select a tag to apply
4. The tag is added to all selected tracks

### Removing Tags

1. Select tagged tracks
2. Right-click and choose **Tags**
3. Click a tag that's currently applied
4. The tag is removed from all selected tracks

### Tag Toggle Mode

When tracks are selected, the tag sidebar shows toggle state:
- **Solid** - All selected tracks have the tag
- **Partial** - Some selected tracks have the tag
- **Empty** - No selected tracks have the tag

Click a tag to toggle it for all selected tracks.

## Bulk Color Assignment

Assign colors to multiple tracks:

1. Select tracks
2. Right-click and choose **Set Color**
3. Select a color
4. All selected tracks receive that color

## Adding to Playlists

Add multiple tracks to a playlist at once:

### Drag and Drop

1. Select tracks in the library
2. Drag them onto a playlist in the sidebar
3. All selected tracks are added

### Context Menu

1. Select tracks
2. Right-click and choose **Add to Playlist**
3. Select the target playlist
4. All selected tracks are added

## Removing Tracks

### From a Playlist

1. View the playlist
2. Select tracks to remove
3. Right-click and choose **Remove from Playlist**
4. Selected tracks are removed (remain in library)

### From Library

1. Select tracks
2. Right-click and choose **Remove from Library**
3. Confirm the action
4. Tracks are removed from Crate's database

Files are not deleted from disk.

## Bulk Operations Summary

| Operation | How to Access |
|-----------|---------------|
| Edit metadata | Inspector panel |
| Apply tags | Right-click > Tags |
| Set color | Right-click > Set Color |
| Add to playlist | Drag to playlist, or Right-click > Add to Playlist |
| Remove from playlist | Right-click > Remove from Playlist |
| Remove from library | Right-click > Remove from Library |

## Tips

### Efficient Workflow

1. Use search/filters to find related tracks
2. **Cmd+A** to select all visible
3. Apply bulk operation
4. Clear filters and repeat for next batch

### Careful with Metadata

When bulk editing:
- Mixed values indicator helps prevent accidental overwrites
- Review before committing changes
- Some fields (like Title) usually should stay unique

### Tag-Based Organization

Use bulk tagging to quickly categorize imports:
1. Import new tracks
2. Sort by Date Added to see recent additions
3. Select a batch with similar characteristics
4. Apply relevant tags
5. Repeat for different attribute combinations
