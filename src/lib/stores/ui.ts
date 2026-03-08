import { writable, derived } from 'svelte/store'
import type { ActiveView, SidebarView, TagFilterMode } from '$lib/types'
import {
	getStoredBoolean,
	getStoredNumber,
	getStoredString,
	setStoredBoolean,
	setStoredNumber,
	setStoredString,
} from '$lib/utils/storage'

// =============================================================================
// State
// =============================================================================

interface ViewNavigationState {
	selectedPlaylistId: string | null
	selectedFolderId: string | null
	sidebarView: SidebarView
}

type ViewNavigationCache = Record<ActiveView, ViewNavigationState>

interface UIState {
	// Active view
	activeView: ActiveView

	// Selection
	selectedTrackIds: Set<string>
	lastSelectedTrackId: string | null
	selectedReleaseIds: Set<string>
	lastSelectedReleaseId: string | null

	// Sidebar
	sidebarView: SidebarView
	selectedPlaylistId: string | null
	selectedFolderId: string | null
	selectedTagIds: string[]
	tagFilterMode: TagFilterMode
	sidebarWidth: number

	// Right Sidebar (Track Editor)
	rightSidebarVisible: boolean
	rightSidebarWidth: number

	// Search
	searchQuery: string
	searchFocused: boolean

	// Modals
	activeModal: string | null

	// Context menu
	contextMenuOpen: boolean
	contextMenuPosition: { x: number; y: number }

	// Tag toggle tracking (for "mixed removes first" behavior)
	recentlyToggledMixedTags: Set<string>

	// Playlist tree multi-selection state
	selectedTreeIds: Set<string>

	// Context menu hover styling for playlist tree
	contextMenuPlaylistId: string | null

	// Navigation cache per view context
	viewNavigationCache: ViewNavigationCache
}

const initialState: UIState = {
	activeView: getStoredString<ActiveView>('activeView', 'library', ['library', 'discovery']),
	selectedTrackIds: new Set(),
	lastSelectedTrackId: null,
	selectedReleaseIds: new Set(),
	lastSelectedReleaseId: null,
	sidebarView: 'library',
	selectedPlaylistId: null,
	selectedFolderId: null,
	selectedTagIds: [],
	tagFilterMode: 'or',
	sidebarWidth: getStoredNumber('sidebarWidth', 240),
	rightSidebarVisible: getStoredBoolean('rightSidebarVisible', false),
	rightSidebarWidth: getStoredNumber('rightSidebarWidth', 320),
	searchQuery: '',
	searchFocused: false,
	activeModal: null,
	contextMenuOpen: false,
	contextMenuPosition: { x: 0, y: 0 },
	recentlyToggledMixedTags: new Set(),
	selectedTreeIds: new Set(),
	contextMenuPlaylistId: null,
	viewNavigationCache: {
		library: { selectedPlaylistId: null, selectedFolderId: null, sidebarView: 'library' },
		discovery: { selectedPlaylistId: null, selectedFolderId: null, sidebarView: 'library' },
	},
}

// =============================================================================
// Store
// =============================================================================

function createUIStore() {
	const { subscribe, set, update } = writable<UIState>(initialState)

	return {
		subscribe,

		// =========================================================================
		// Active View
		// =========================================================================

		/**
		 * Switch between library and discovery views
		 */
		setActiveView(view: ActiveView) {
			setStoredString('activeView', view)
			update((state) => {
				// Save current navigation into cache
				const updatedCache = {
					...state.viewNavigationCache,
					[state.activeView]: {
						selectedPlaylistId: state.selectedPlaylistId,
						selectedFolderId: state.selectedFolderId,
						sidebarView: state.sidebarView,
					},
				}

				// Restore cached navigation for the target view
				const restored = updatedCache[view]

				return {
					...state,
					activeView: view,
					selectedTrackIds: new Set(),
					lastSelectedTrackId: null,
					selectedReleaseIds: new Set(),
					lastSelectedReleaseId: null,
					selectedPlaylistId: restored.selectedPlaylistId,
					selectedFolderId: restored.selectedFolderId,
					sidebarView: restored.sidebarView,
					viewNavigationCache: updatedCache,
				}
			})
		},

		// =========================================================================
		// Selection
		// =========================================================================

		/**
		 * Set selected track IDs
		 */
		setSelectedTracks(ids: Set<string>, lastId?: string) {
			update((state) => ({
				...state,
				selectedTrackIds: ids,
				lastSelectedTrackId: lastId ?? state.lastSelectedTrackId,
			}))
		},

		/**
		 * Clear track selection
		 */
		clearSelection() {
			update((state) => ({
				...state,
				selectedTrackIds: new Set(),
				lastSelectedTrackId: null,
			}))
		},

		/**
		 * Select a single track
		 */
		selectTrack(id: string) {
			update((state) => ({
				...state,
				selectedTrackIds: new Set([id]),
				lastSelectedTrackId: id,
			}))
		},

		/**
		 * Toggle track selection
		 */
		toggleTrackSelection(id: string) {
			update((state) => {
				const newSelection = new Set(state.selectedTrackIds)
				if (newSelection.has(id)) {
					newSelection.delete(id)
				} else {
					newSelection.add(id)
				}
				return {
					...state,
					selectedTrackIds: newSelection,
					lastSelectedTrackId: id,
				}
			})
		},

		// =========================================================================
		// Release Selection
		// =========================================================================

		/**
		 * Set selected release IDs
		 */
		setSelectedReleases(ids: Set<string>, lastId?: string) {
			update((state) => ({
				...state,
				selectedReleaseIds: ids,
				lastSelectedReleaseId: lastId ?? state.lastSelectedReleaseId,
			}))
		},

		/**
		 * Clear release selection
		 */
		clearReleaseSelection() {
			update((state) => ({
				...state,
				selectedReleaseIds: new Set(),
				lastSelectedReleaseId: null,
			}))
		},

		/**
		 * Select a single release
		 */
		selectRelease(id: string) {
			update((state) => ({
				...state,
				selectedReleaseIds: new Set([id]),
				lastSelectedReleaseId: id,
			}))
		},

		// =========================================================================
		// Sidebar
		// =========================================================================

		/**
		 * Set sidebar view
		 */
		setSidebarView(view: SidebarView) {
			update((state) => ({
				...state,
				sidebarView: view,
				selectedPlaylistId: view === 'playlist' ? state.selectedPlaylistId : null,
				selectedFolderId: view === 'folder' ? state.selectedFolderId : null,
				selectedTagIds: view === 'tag' ? state.selectedTagIds : [],
			}))
		},

		/**
		 * Select a playlist
		 */
		selectPlaylist(id: string | null) {
			update((state) => ({
				...state,
				sidebarView: id ? 'playlist' : 'library',
				selectedPlaylistId: id,
				selectedFolderId: null,
				selectedTagIds: [],
			}))
		},

		/**
		 * Toggle a tag filter (add if not present, remove if present)
		 */
		toggleTagFilter(id: string) {
			update((state) => {
				const exists = state.selectedTagIds.includes(id)
				const newIds = exists ? state.selectedTagIds.filter((tid) => tid !== id) : [...state.selectedTagIds, id]
				return {
					...state,
					sidebarView: newIds.length > 0 ? 'tag' : 'library',
					selectedTagIds: newIds,
					selectedPlaylistId: null,
					selectedFolderId: null,
				}
			})
		},

		/**
		 * Add a tag to filters
		 */
		addTagFilter(id: string) {
			update((state) => ({
				...state,
				sidebarView: 'tag',
				selectedTagIds: state.selectedTagIds.includes(id) ? state.selectedTagIds : [...state.selectedTagIds, id],
				selectedPlaylistId: null,
				selectedFolderId: null,
			}))
		},

		/**
		 * Remove a tag from filters
		 */
		removeTagFilter(id: string) {
			update((state) => {
				const newIds = state.selectedTagIds.filter((tid) => tid !== id)
				return {
					...state,
					sidebarView: newIds.length > 0 ? 'tag' : 'library',
					selectedTagIds: newIds,
				}
			})
		},

		/**
		 * Clear all tag filters
		 */
		clearTagFilters() {
			update((state) => ({
				...state,
				sidebarView: 'library',
				selectedTagIds: [],
			}))
		},

		/**
		 * Set tag filter mode (AND/OR)
		 */
		setTagFilterMode(mode: TagFilterMode) {
			update((state) => ({
				...state,
				tagFilterMode: mode,
			}))
		},

		/**
		 * Toggle tag filter mode between AND and OR
		 */
		toggleTagFilterMode() {
			update((state) => ({
				...state,
				tagFilterMode: state.tagFilterMode === 'or' ? 'and' : 'or',
			}))
		},

		/**
		 * Select a folder
		 */
		selectFolder(id: string | null) {
			update((state) => ({
				...state,
				sidebarView: id ? 'folder' : 'library',
				selectedFolderId: id,
				selectedPlaylistId: null,
				selectedTagIds: [],
			}))
		},

		/**
		 * Set sidebar width
		 */
		setSidebarWidth(width: number) {
			const clampedWidth = Math.max(200, Math.min(400, width))
			setStoredNumber('sidebarWidth', clampedWidth)
			update((state) => ({
				...state,
				sidebarWidth: clampedWidth,
			}))
		},

		// =========================================================================
		// Right Sidebar (Track Editor)
		// =========================================================================

		/**
		 * Toggle right sidebar visibility
		 */
		toggleRightSidebar() {
			update((state) => {
				const newVisible = !state.rightSidebarVisible
				setStoredBoolean('rightSidebarVisible', newVisible)
				return { ...state, rightSidebarVisible: newVisible }
			})
		},

		/**
		 * Set right sidebar visibility
		 */
		setRightSidebarVisible(visible: boolean) {
			setStoredBoolean('rightSidebarVisible', visible)
			update((state) => ({ ...state, rightSidebarVisible: visible }))
		},

		/**
		 * Set right sidebar width
		 */
		setRightSidebarWidth(width: number) {
			const clampedWidth = Math.max(280, Math.min(500, width))
			setStoredNumber('rightSidebarWidth', clampedWidth)
			update((state) => ({ ...state, rightSidebarWidth: clampedWidth }))
		},

		// =========================================================================
		// Search
		// =========================================================================

		/**
		 * Set search query
		 */
		setSearchQuery(query: string) {
			update((state) => ({ ...state, searchQuery: query }))
		},

		/**
		 * Set search focus state
		 */
		setSearchFocused(focused: boolean) {
			update((state) => ({ ...state, searchFocused: focused }))
		},

		/**
		 * Clear search
		 */
		clearSearch() {
			update((state) => ({ ...state, searchQuery: '' }))
		},

		// =========================================================================
		// Modals
		// =========================================================================

		/**
		 * Open a modal
		 */
		openModal(modalId: string) {
			update((state) => ({ ...state, activeModal: modalId }))
		},

		/**
		 * Close the active modal
		 */
		closeModal() {
			update((state) => ({ ...state, activeModal: null }))
		},

		// =========================================================================
		// Context Menu
		// =========================================================================

		/**
		 * Open context menu at position
		 */
		openContextMenu(x: number, y: number) {
			update((state) => ({
				...state,
				contextMenuOpen: true,
				contextMenuPosition: { x, y },
			}))
		},

		/**
		 * Close context menu
		 */
		closeContextMenu() {
			update((state) => ({
				...state,
				contextMenuOpen: false,
			}))
		},

		// =========================================================================
		// Tag Toggle Tracking
		// =========================================================================

		/**
		 * Mark a tag as recently toggled (for mixed state "remove first" behavior)
		 */
		markTagAsRecentlyToggled(tagId: string) {
			update((state) => ({
				...state,
				recentlyToggledMixedTags: new Set([...state.recentlyToggledMixedTags, tagId]),
			}))
		},

		/**
		 * Clear a tag from the recently toggled set
		 */
		clearRecentlyToggledTag(tagId: string) {
			update((state) => {
				const newSet = new Set(state.recentlyToggledMixedTags)
				newSet.delete(tagId)
				return { ...state, recentlyToggledMixedTags: newSet }
			})
		},

		/**
		 * Set playlist tree multi-selection IDs
		 */
		setSelectedTreeIds(ids: Set<string>) {
			update((state) => ({ ...state, selectedTreeIds: ids }))
		},

		/**
		 * Clear playlist tree multi-selection
		 */
		clearSelectedTreeIds() {
			update((state) => (state.selectedTreeIds.size > 0 ? { ...state, selectedTreeIds: new Set() } : state))
		},

		/**
		 * Set context menu playlist ID (for hover styling)
		 */
		setContextMenuPlaylistId(id: string | null) {
			update((state) => ({ ...state, contextMenuPlaylistId: id }))
		},

		/**
		 * Clear context menu playlist ID
		 */
		clearContextMenuPlaylistId() {
			update((state) => (state.contextMenuPlaylistId !== null ? { ...state, contextMenuPlaylistId: null } : state))
		},

		/**
		 * Clear all recently toggled tags (call when selection changes)
		 */
		clearAllRecentlyToggledTags() {
			update((state) => ({
				...state,
				recentlyToggledMixedTags: new Set(),
			}))
		},

		// =========================================================================
		// Reset
		// =========================================================================

		/**
		 * Reset store to initial state
		 */
		reset() {
			set(initialState)
		},
	}
}

export const uiStore = createUIStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const selectedTrackIds = derived(uiStore, ($ui) => $ui.selectedTrackIds)

export const selectedTrackCount = derived(uiStore, ($ui) => $ui.selectedTrackIds.size)

export const hasSelection = derived(uiStore, ($ui) => $ui.selectedTrackIds.size > 0)

export const searchQuery = derived(uiStore, ($ui) => $ui.searchQuery)

export const isSearchActive = derived(uiStore, ($ui) => $ui.searchQuery.length > 0)

export const recentlyToggledMixedTags = derived(uiStore, ($ui) => $ui.recentlyToggledMixedTags)

export const selectedTagIds = derived(uiStore, ($ui) => $ui.selectedTagIds)

export const tagFilterMode = derived(uiStore, ($ui) => $ui.tagFilterMode)

export const activeView = derived(uiStore, ($ui) => $ui.activeView)

export const selectedReleaseIds = derived(uiStore, ($ui) => $ui.selectedReleaseIds)

export const selectedReleaseCount = derived(uiStore, ($ui) => $ui.selectedReleaseIds.size)

export const rightSidebarVisible = derived(uiStore, ($ui) => $ui.rightSidebarVisible)

export const rightSidebarWidth = derived(uiStore, ($ui) => $ui.rightSidebarWidth)

export const selectedTreeIds = derived(uiStore, ($ui) => $ui.selectedTreeIds)

export const contextMenuPlaylistId = derived(uiStore, ($ui) => $ui.contextMenuPlaylistId)
