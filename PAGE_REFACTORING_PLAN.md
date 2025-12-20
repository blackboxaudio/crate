# +page.svelte Refactoring Plan

A multi-phase plan to refactor the 1,720-line `src/routes/+page.svelte` into smaller, manageable components. Each phase is designed to be executed by a fresh Claude instance.

**Current state:** 1,720 lines, 65+ state variables, 8 context menus, 27+ modals
**Target state:** ~250-350 lines

---

## Phase Summary

| Phase | Focus | New Files | Lines Reduced | Risk |
|-------|-------|-----------|---------------|------|
| 1 | Context Menus | 1 component | ~300 | Low |
| 2 | Modals | 1 component | ~400 | Low |
| 3 | Playlist Utils | 1 util file | ~150 | Low-Med |
| 4 | Tag Controller | 1 controller | ~180 | Low |
| 5 | Track Controller | 1 controller | ~200 | Medium |
| 6 | App Hooks | 3 hook files | ~180 | Low |

---

## Phase 1: Context Menu Orchestration Layer

**Goal:** Extract all 8 context menu state groups and handlers into a single orchestrator component.

### State to Extract (~30 variables)
- Track context menu: `contextMenuOpen`, `contextMenuPosition`, `contextMenuTracks`
- Playlist context menu: `playlistContextMenuOpen`, `playlistContextMenuPosition`, `playlistContextMenuPlaylist`, `playlistContextMenuSource`
- Folder view context menu: `folderViewContextMenuOpen`, `folderViewContextMenuPosition`, `folderViewContextMenuFolderId`
- Playlist tree context menu: `playlistTreeContextMenuOpen`, `playlistTreeContextMenuPosition`
- Library view context menu: `libraryViewContextMenuOpen`, `libraryViewContextMenuPosition`
- Playlist view context menu: `playlistViewContextMenuOpen`, `playlistViewContextMenuPosition`, `playlistViewContextMenuPlaylist`
- Tag context menu: `tagContextMenuOpen`, `tagContextMenuPosition`, `tagContextMenuTarget`
- Tags sidebar context menu: `tagsSidebarContextMenuOpen`, `tagsSidebarContextMenuPosition`
- Device context menu: `deviceContextMenuOpen`, `deviceContextMenuPosition`, `deviceContextMenuDevice`

### Functions to Move
- `closeAllContextMenus()`
- `handleTrackContextMenu()`
- `handlePlaylistContextMenu()`
- `handleFolderViewCardContextMenu()`
- `handlePlaylistTreeContextMenu()`
- `handleFolderViewContextMenu()`
- `handleLibraryViewContextMenu()`
- `handlePlaylistViewContextMenu()`
- `handleTagContextMenu()`
- `handleCategoryContextMenu()`
- `handleTagsWhitespaceContextMenu()`
- `handleDeviceContextMenu()`

### New File
**`src/lib/components/common/ContextMenuOrchestrator.svelte`**

Uses a discriminated union for menu type:
```typescript
type ActiveContextMenu =
  | { type: 'none' }
  | { type: 'track'; x: number; y: number; tracks: Track[] }
  | { type: 'playlist'; x: number; y: number; playlist: Playlist; source: 'tree' | 'folder' }
  | { type: 'folderView'; x: number; y: number; folderId: string }
  | { type: 'playlistTree'; x: number; y: number }
  | { type: 'libraryView'; x: number; y: number }
  | { type: 'playlistView'; x: number; y: number; playlist: Playlist }
  | { type: 'tag'; x: number; y: number; target: TagContextTarget }
  | { type: 'tagsSidebar'; x: number; y: number }
  | { type: 'device'; x: number; y: number; device: UsbDevice }
```

### Integration
Parent passes callback props for all actions (onAddToPlaylist, onRevealInExplorer, onRemove, etc.)

### Testing
- Right-click on tracks, playlists, folders, tags, categories, devices, whitespace areas
- Verify all menu items appear correctly
- Test all menu actions trigger correct callbacks

---

## Phase 2: Modal Orchestration Layer

**Goal:** Extract all modal state and template instances into a single modal manager component.

### State to Extract (~35 variables)

**Creation modals:**
- `showPlaylistModal`, `playlistModalParentId`
- `showFolderModal`, `folderModalParentId`
- `showCategoryModal`
- `showTagModal`, `tagModalCategoryId`

**Rename modals:**
- `showRenamePlaylistModal`, `renamePlaylistId`, `renamePlaylistValue`
- `showRenameTagModal`, `renameTagId`, `renameTagValue`
- `showRenameCategoryModal`, `renameCategoryId`, `renameCategoryValue`

**Delete/Confirmation modals:**
- `showDeletePlaylistConfirm`, `deletePlaylistId`, `deletePlaylistIsFolder`, `deletePlaylistHasChildren`, `deleteTracksFromCollection`
- `showRemoveFromPlaylistConfirm`, `removeTrackIds`, `removePlaylistId`
- `showRemoveFromLibraryConfirm`
- `showDeleteTagConfirm`, `deleteTagId`
- `showDeleteCategoryConfirm`, `deleteCategoryId`

**Feature-specific modals:**
- `showSettings`
- `showRelocateModal`, `relocateTrack`
- `showMoveConflictModal`, `moveConflictMovingItem`, `moveConflictExistingItem`, `moveConflictTargetParentId`, `pendingMergeConflicts`
- `showDeviceInfoModal`, `deviceInfoDevice`
- `showTagInputModal`

### Functions to Move
- `handlePlaylistModalSubmit()`
- `handleFolderModalSubmit()`
- `handleCategoryModalSubmit()`
- `handleTagModalSubmit()`
- `handleRenamePlaylistSubmit()`
- `handleDeletePlaylistConfirm()`
- `handleRemoveFromPlaylistConfirm()`
- `handleRemoveFromLibraryConfirm()`
- `handleRenameTagSubmit()`
- `handleDeleteTagConfirm()`
- `handleRenameCategorySubmit()`
- `handleDeleteCategoryConfirm()`
- `handleTagInputModalSubmit()`
- Move conflict handlers (cancel, overwrite, merge, processNextMergeConflict)
- `handleRelocateComplete()`

### New File
**`src/lib/components/common/ModalOrchestrator.svelte`**

Uses a discriminated union for modal type similar to context menus.

### Testing
- Create playlist/folder/category/tag
- Rename operations
- Delete confirmations
- Settings modal
- Move conflict resolution

---

## Phase 3: Playlist Operations Utilities

**Goal:** Extract playlist business logic including CRUD, move operations, and conflict resolution into utility functions.

### Functions to Move
- `findConflictingItem()`
- `getPlaylistById()`
- Move conflict detection logic from `handlePlaylistMove()` and `handlePlaylistDragMove()`

### New File
**`src/lib/utils/playlistConflicts.ts`**
```typescript
export function findConflictingItem(
  playlists: Playlist[],
  movingItem: Playlist,
  targetParentId: string | null
): Playlist | null

export function getPlaylistById(
  playlists: Playlist[],
  id: string
): Playlist | null
```

### Testing
- Move playlist between folders
- Handle name conflicts
- Merge folders

---

## Phase 4: Tag Operations Controller

**Goal:** Extract tag-related business logic including filtering, toggling, and CRUD operations.

### Functions to Move
- `handleTagSelect()`
- `handleTagToggle()`
- `handleClearTagFilter()`
- `handleRemoveTagFilter()`
- `handleToggleTagFilterMode()`
- `handleRenameTag()`
- `handleDeleteTag()`
- `handleRenameCategory()`
- `handleDeleteCategory()`
- `handleChangeCategoryColor()`

### New File
**`src/lib/controllers/tagController.ts`**
```typescript
export function createTagController(tagsStore, libraryStore, uiStore) {
  return {
    handleTagSelect(...),
    handleTagToggle(...),
    handleClearTagFilter(...),
    handleRemoveTagFilter(...),
    handleToggleTagFilterMode(...),
    // ... CRUD operations
  }
}
```

### Testing
- Toggle tag filters (AND/OR mode)
- Tag tracks (single and multi-select)
- Mixed state toggle behavior

---

## Phase 5: Track Operations Controller

**Goal:** Extract track-related operations including playback, selection, color changes, removal, and file operations.

### Functions to Move
- `handleTrackPlay()`
- `handleSelectionChange()`
- `handleAddToPlaylist()`
- `handleRevealInExplorer()`
- `handleRemoveFromPlaylistClick()`
- `handleRemoveFromLibraryClick()`
- `handleTracksDropOnPlaylist()`
- `handleTrackColorChange()`
- `handleContextMenuSetColor()`
- `handleExternalFileDrop()`
- `handleImport()`

### New File
**`src/lib/controllers/trackController.ts`**
```typescript
export function createTrackController(playerStore, libraryStore, playlistsStore, missingTracksStore, uiStore) {
  return {
    handlePlay(...),
    handleSelectionChange(...),
    handleAddToPlaylist(...),
    handleRemoveFromPlaylist(...),
    handleRemoveFromLibrary(...),
    handleColorChange(...),
    handleReveal(...),
    handleExternalFileDrop(...),
    handleImport(...),
  }
}
```

### Testing
- Play tracks
- Selection (click, shift-click, cmd-click)
- Remove from playlist/library
- Color changes
- File import via drag-drop and menu

---

## Phase 6: App Initialization Hooks

**Goal:** Extract application lifecycle, keyboard shortcuts, and menu action handlers.

### Functions to Move
- `handleKeydown()`
- `isInputFocused()`
- `handleMenuAction()`
- Most `onMount` logic (store initialization, event listeners)

### New Files

**`src/lib/hooks/useAppInitialization.ts`**
```typescript
export function useAppInitialization(config) {
  // Store initialization, drag-drop setup, device listener, menu listener
  // Returns cleanup function
}
```

**`src/lib/hooks/useKeyboardShortcuts.ts`**
```typescript
export function useKeyboardShortcuts(handlers) {
  // Space, Cmd+F, Escape, Cmd+A, Cmd+,
  // Returns cleanup function
}
```

**`src/lib/hooks/useMenuActions.ts`**
```typescript
export function useMenuActions(handlers) {
  // import_tracks, new_playlist, play_pause, settings, etc.
  // Returns cleanup function
}
```

### Testing
- App loads correctly
- Keyboard shortcuts work
- Menu bar actions work
- Device connect/disconnect notifications

---

## Critical Files

- `src/routes/+page.svelte` - The file being refactored
- `src/lib/components/common/ContextMenu.svelte` - Base context menu pattern
- `src/lib/components/common/Modal.svelte` - Base modal pattern
- `src/lib/stores/ui.ts` - UI state management patterns
- `src/lib/types/index.ts` - Type definitions to extend

---

## Final +page.svelte Structure

```svelte
<script lang="ts">
  // Minimal imports
  import { onMount } from 'svelte'
  import { stores } from '$lib/stores'
  import { useAppInitialization, useKeyboardShortcuts, useMenuActions } from '$lib/hooks'
  import { createTagController, createTrackController } from '$lib/controllers'
  import { ContextMenuOrchestrator, ModalOrchestrator } from '$lib/components/common'
  import { Sidebar, Toolbar } from '$lib/components/layout'
  import { LibraryView } from '$lib/components/library'
  import { PlaylistView, FolderView } from '$lib/components/playlists'
  import { Player } from '$lib/components/player'

  // Core local state only
  let sortConfig = $state<SortConfig>({ field: 'date_added', direction: 'desc' })
  let isDragOver = $state(false)

  // Store subscriptions
  let playlists = $state<Playlist[]>([])
  let tagCategories = $state<TagCategory[]>([])
  // ...

  // Controllers
  const tagController = createTagController(...)
  const trackController = createTrackController(...)

  // Lifecycle
  onMount(() => {
    const cleanupApp = useAppInitialization(...)
    const cleanupKeyboard = useKeyboardShortcuts(...)
    const cleanupMenu = useMenuActions(...)
    return () => { cleanupApp(); cleanupKeyboard(); cleanupMenu() }
  })

  // Remaining handlers (navigation, sort, sidebar resize)
</script>

<div class="flex h-full flex-col">
  <Toolbar ... />
  <div class="flex flex-1 overflow-hidden">
    <Sidebar ... />
    <ResizeHandle ... />
    <ContentView ... />
  </div>
  <Player />
</div>

<ContextMenuOrchestrator ... />
<ModalOrchestrator ... />
```
