import { writable, derived } from 'svelte/store'
import { getStoredBoolean, getStoredNumber, setStoredBoolean, setStoredNumber } from '$lib/utils/storage'

// =============================================================================
// State
// =============================================================================
//
// Desktop-only UI layout state: window chrome (sidebars, their widths), context menus, modals,
// and the playlist-tree multi-selection. Split out of the shared `ui` store so that mobile (which
// has none of this chrome) can reuse the cross-platform view/selection/filter state in `ui.ts`.

interface UILayoutState {
	// Sidebar
	sidebarWidth: number

	// Right Sidebar (Track Editor)
	rightSidebarVisible: boolean
	rightSidebarWidth: number

	// Modals
	activeModal: string | null

	// Context menu
	contextMenuOpen: boolean
	contextMenuPosition: { x: number; y: number }

	// Playlist tree multi-selection state
	selectedTreeIds: Set<string>

	// Context menu hover styling for playlist tree
	contextMenuPlaylistId: string | null

	// Context menu hover styling for discovery track sub-rows
	contextMenuDiscoveryTrackId: string | null

	// Per-playlist scroll offset cache
	playlistScrollOffsets: Map<string, number>
}

const initialState: UILayoutState = {
	sidebarWidth: getStoredNumber('sidebarWidth', 240),
	rightSidebarVisible: getStoredBoolean('rightSidebarVisible', false),
	rightSidebarWidth: getStoredNumber('rightSidebarWidth', 320),
	activeModal: null,
	contextMenuOpen: false,
	contextMenuPosition: { x: 0, y: 0 },
	selectedTreeIds: new Set(),
	contextMenuPlaylistId: null,
	contextMenuDiscoveryTrackId: null,
	playlistScrollOffsets: new Map(),
}

// =============================================================================
// Store
// =============================================================================

function createUILayoutStore() {
	const { subscribe, set, update } = writable<UILayoutState>(initialState)

	return {
		subscribe,

		// =========================================================================
		// Sidebar
		// =========================================================================

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
		// Playlist Tree Selection
		// =========================================================================

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
		 * Set context menu discovery track ID (for hover styling)
		 */
		setContextMenuDiscoveryTrackId(id: string | null) {
			update((state) => ({ ...state, contextMenuDiscoveryTrackId: id }))
		},

		/**
		 * Clear context menu discovery track ID
		 */
		clearContextMenuDiscoveryTrackId() {
			update((state) =>
				state.contextMenuDiscoveryTrackId !== null ? { ...state, contextMenuDiscoveryTrackId: null } : state
			)
		},

		// =========================================================================
		// Scroll
		// =========================================================================

		/**
		 * Update scroll offset for a specific playlist
		 */
		setPlaylistScrollOffset(playlistId: string, offset: number) {
			update((state) => {
				const newOffsets = new Map(state.playlistScrollOffsets)
				newOffsets.set(playlistId, offset)
				return { ...state, playlistScrollOffsets: newOffsets }
			})
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

export const uiLayoutStore = createUILayoutStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const rightSidebarVisible = derived(uiLayoutStore, ($ui) => $ui.rightSidebarVisible)

export const rightSidebarWidth = derived(uiLayoutStore, ($ui) => $ui.rightSidebarWidth)

export const selectedTreeIds = derived(uiLayoutStore, ($ui) => $ui.selectedTreeIds)

export const contextMenuPlaylistId = derived(uiLayoutStore, ($ui) => $ui.contextMenuPlaylistId)

export const contextMenuDiscoveryTrackId = derived(uiLayoutStore, ($ui) => $ui.contextMenuDiscoveryTrackId)

export const playlistScrollOffsets = derived(uiLayoutStore, ($ui) => $ui.playlistScrollOffsets)
