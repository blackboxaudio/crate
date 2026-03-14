import { writable, derived } from 'svelte/store'

// =============================================================================
// Types
// =============================================================================

export type DragData =
	| { type: 'tracks'; trackIds: string[] }
	| { type: 'releases'; releaseIds: string[] }
	| { type: 'playlist'; playlistId: string; playlistIds: string[]; isFolder: boolean }
	| { type: 'tag'; tagId: string; sourceCategoryId: string }

interface DragState {
	data: DragData | null
	position: { x: number; y: number } | null
	hoveredDropTarget: string | null
	needsDropTargetRefresh: boolean
}

// =============================================================================
// Store
// =============================================================================

const initialState: DragState = {
	data: null,
	position: null,
	hoveredDropTarget: null,
	needsDropTargetRefresh: false,
}

function createDragStore() {
	const { subscribe, set, update } = writable<DragState>(initialState)

	return {
		subscribe,

		/**
		 * Start dragging tracks
		 */
		startTrackDrag(trackIds: string[], x: number, y: number) {
			set({
				data: { type: 'tracks', trackIds },
				position: { x, y },
				hoveredDropTarget: null,
				needsDropTargetRefresh: false,
			})
		},

		/**
		 * Start dragging releases
		 */
		startReleaseDrag(releaseIds: string[], x: number, y: number) {
			set({
				data: { type: 'releases', releaseIds },
				position: { x, y },
				hoveredDropTarget: null,
				needsDropTargetRefresh: false,
			})
		},

		/**
		 * Start dragging a playlist or folder
		 */
		startPlaylistDrag(playlistId: string, isFolder: boolean, x: number, y: number, playlistIds?: string[]) {
			set({
				data: { type: 'playlist', playlistId, playlistIds: playlistIds ?? [playlistId], isFolder },
				position: { x, y },
				hoveredDropTarget: null,
				needsDropTargetRefresh: false,
			})
		},

		/**
		 * Start dragging a tag
		 */
		startTagDrag(tagId: string, sourceCategoryId: string, x: number, y: number) {
			set({
				data: { type: 'tag', tagId, sourceCategoryId },
				position: { x, y },
				hoveredDropTarget: null,
				needsDropTargetRefresh: false,
			})
		},

		/**
		 * Update drag position (called on pointermove)
		 */
		updatePosition(x: number, y: number) {
			update((state) => ({
				...state,
				position: { x, y },
			}))
		},

		/**
		 * Set the currently hovered drop target
		 */
		setHoveredDropTarget(targetId: string | null) {
			update((state) => ({
				...state,
				hoveredDropTarget: targetId,
			}))
		},

		/**
		 * End the drag operation
		 */
		endDrag() {
			set(initialState)
		},

		/**
		 * Request a refresh of drop targets (e.g., after folder expand)
		 */
		requestDropTargetRefresh() {
			update((state) => ({
				...state,
				needsDropTargetRefresh: true,
			}))
		},

		/**
		 * Clear the refresh request flag
		 */
		clearDropTargetRefresh() {
			update((state) => ({
				...state,
				needsDropTargetRefresh: false,
			}))
		},

		/**
		 * Check if currently dragging
		 */
		isDragging(): boolean {
			let dragging = false
			subscribe((state) => {
				dragging = state.data !== null
			})()
			return dragging
		},
	}
}

export const dragStore = createDragStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const isDragging = derived(dragStore, ($drag) => $drag.data !== null)

export const dragData = derived(dragStore, ($drag) => $drag.data)

export const dragPosition = derived(dragStore, ($drag) => $drag.position)

export const hoveredDropTarget = derived(dragStore, ($drag) => $drag.hoveredDropTarget)

export const isDraggingTracks = derived(dragStore, ($drag) => $drag.data?.type === 'tracks')

export const isDraggingReleases = derived(dragStore, ($drag) => $drag.data?.type === 'releases')

export const isDraggingPlaylist = derived(dragStore, ($drag) => $drag.data?.type === 'playlist')

export const isDraggingTag = derived(dragStore, ($drag) => $drag.data?.type === 'tag')

export const needsDropTargetRefresh = derived(dragStore, ($drag) => $drag.needsDropTargetRefresh)
