# Drag and Drop Issue - Context for Debugging

## Desired Outcome

We need **both** of these drag-and-drop capabilities to work simultaneously:

### 1. External File Drops
- Drag audio files from Finder/Explorer onto the track list
- Files should be imported into the library
- Visual feedback (drop zone highlight) should appear when dragging over the track list

### 2. Internal Drag-and-Drop
- **Tracks → Playlists**: Drag tracks from the track list onto playlists in the sidebar to add them
- **Playlists → Folders**: Drag playlists onto folders in the sidebar to organize them
- **Folders → Folders**: Drag folders onto other folders to nest them
- Visual feedback (ring highlight) should appear on valid drop targets

## Current Problem

HTML5 drag-and-drop events (`dragenter`, `dragover`, `dragleave`, `drop`) are **not firing at all** in this Tauri 2.0 application. Only `dragstart` and `dragend` events work.

### Symptoms
1. Dragging tracks from the track list over playlists in the sidebar - no visual feedback, drops don't work
2. Dragging playlists/folders over folders - no visual feedback, drops don't work
3. External file drops from Finder/Explorer don't work
4. `dragstart` fires correctly on both TrackRow and PlaylistItem components
5. `dragenter`/`dragover`/`dragleave`/`drop` never fire - not even on document-level event listeners

## Root Cause Identified

**Tauri's `dragDropEnabled` setting** intercepts ALL drag events at the native WebView level, preventing HTML5 drag events from reaching the web content.

### Research Findings
- [Tauri GitHub Issue #14373](https://github.com/tauri-apps/tauri/issues/14373): The flag means "Tauri's internal drag and drop system is enabled, and DOM drag and drop is disabled"
- [Tauri GitHub Issue #3277](https://github.com/tauri-apps/tauri/issues/3277): Known issue where only `dragstart` and `dragend` work
- [Ellie's blog post](https://ellie.wtf/notes/drag-event-issues-in-Tauri): Setting `dragDropEnabled: false` should fix it
- Platform-specific quirks exist - behavior differs between macOS, Windows, and Linux

## What We've Tried

### 1. Set `dragDropEnabled: false` in tauri.conf.json
```json
{
  "app": {
    "windows": [
      {
        "dragDropEnabled": false
      }
    ]
  }
}
```
**Result**: Still not working. Events don't fire.

### 2. Switched from Tauri's event system to browser-native drag events
Changed `useAppInitialization.ts` to use `document.addEventListener('dragenter', ...)` instead of Tauri's `onDragDropEvent`.
**Result**: Event listeners are registered (confirmed via console log), but events never fire.

### 3. Used capture phase for event listeners
```typescript
document.addEventListener('dragenter', handler, true) // capture phase
```
**Result**: Still no events firing.

### 4. Clean rebuild
```bash
cd src-tauri && cargo clean
```
**Result**: Still not working after full rebuild.

## Current State of Key Files

### `src-tauri/tauri.conf.json` (line 21)
```json
"dragDropEnabled": false
```

### `src/lib/hooks/useAppInitialization.ts`
- Uses browser-native drag events instead of Tauri's `onDragDropEvent`
- Event listeners registered on document with capture phase
- Has debug logging that confirms listeners are registered
- `hasExternalFiles()` check only handles OS file drags, ignores internal drags

### `src/lib/components/playlists/PlaylistItem.svelte`
- Has all drag handlers: `ondragstart`, `ondragover`, `ondragenter`, `ondragleave`, `ondrop`
- Uses `draggable="true"` on the div
- Has debug console.log statements in all handlers
- Uses counter pattern for drag enter/leave to handle nested elements
- MIME types: `application/x-crate-playlist` for playlists, `application/x-crate-tracks` for tracks

### `src/lib/components/library/TrackRow.svelte`
- Has `ondragstart` handler that sets `application/x-crate-tracks` MIME type
- Debug logging confirms dragstart fires correctly with proper MIME types

## Console Output When Dragging

When dragging a track:
```
[DragStart] "Track Name" {dataTransfer: true}
[DragStart] Set data: {trackIds: ["uuid"], types: ["application/x-crate-tracks", "text/plain"]}
```

When dragging a playlist:
```
[PlaylistItem] dragstart "Playlist Name" {dataTransfer: true}
[PlaylistItem] dragstart set data: {types: ["application/x-crate-playlist", "text/plain"]}
```

**Nothing else logs** - no `[Document] dragenter`, no `[PlaylistItem] dragenter`, nothing.

## Technical Details

- **Platform**: macOS (Darwin 24.5.0)
- **Tauri version**: 2.0 (uses `@tauri-apps/api` v2)
- **Frontend**: SvelteKit with Svelte 5
- **WebView**: WKWebView (macOS)

## Potential Solutions to Try

The challenge is getting **both** external file drops AND internal drag-and-drop working together.

1. **Hybrid approach (recommended)**: Keep `dragDropEnabled: true` for external file drops (using Tauri's `onDragDropEvent`) but implement internal drag-and-drop using a pointer/mouse-event based library like [dnd-kit](https://dndkit.com/) that bypasses native drag events entirely

2. **Fix the config**: Figure out why `dragDropEnabled: false` isn't enabling HTML5 drag events - maybe a Tauri bug, platform-specific issue, or missing configuration

3. **Tauri plugin**: Check if there's a Tauri plugin that provides better drag-drop control while preserving both capabilities

4. **WebView configuration**: Check if WKWebView has additional configuration options for drag-drop that could allow both to work

## File Locations

- Tauri config: `src-tauri/tauri.conf.json`
- App initialization hook: `src/lib/hooks/useAppInitialization.ts`
- Playlist tree item: `src/lib/components/playlists/PlaylistItem.svelte`
- Playlist tree: `src/lib/components/playlists/PlaylistTree.svelte`
- Track row: `src/lib/components/library/TrackRow.svelte`
- Track list: `src/lib/components/library/TrackList.svelte`
- Sidebar: `src/lib/components/layout/Sidebar.svelte`
- Main page: `src/routes/+page.svelte`
