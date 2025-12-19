import { writable, derived } from 'svelte/store'
import type { SidebarView, TagFilterMode } from '$lib/types'
import { getStoredNumber, setStoredNumber } from '$lib/utils/storage'

// =============================================================================
// State
// =============================================================================

interface UIState {
	// Selection
	selectedTrackIds: Set<string>
	lastSelectedTrackId: string | null

	// Sidebar
	sidebarView: SidebarView
	selectedPlaylistId: string | null
	selectedFolderId: string | null
	selectedTagIds: string[]
	tagFilterMode: TagFilterMode
	sidebarWidth: number

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
}

const initialState: UIState = {
	selectedTrackIds: new Set(),
	lastSelectedTrackId: null,
	sidebarView: 'library',
	selectedPlaylistId: null,
	selectedFolderId: null,
	selectedTagIds: [],
	tagFilterMode: 'or',
	sidebarWidth: getStoredNumber('sidebarWidth', 240),
	searchQuery: '',
	searchFocused: false,
	activeModal: null,
	contextMenuOpen: false,
	contextMenuPosition: { x: 0, y: 0 },
	recentlyToggledMixedTags: new Set(),
}

// =============================================================================
// Store
// =============================================================================

function createUIStore() {
	const { subscribe, set, update } = writable<UIState>(initialState)

	return {
		subscribe,

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
