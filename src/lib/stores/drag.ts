import { writable, derived } from 'svelte/store'

// =============================================================================
// Types
// =============================================================================

export type DragData =
	| { type: 'tracks'; trackIds: string[] }
	| { type: 'playlist'; playlistId: string; isFolder: boolean }

interface DragState {
	data: DragData | null
	position: { x: number; y: number } | null
	hoveredDropTarget: string | null
}

// =============================================================================
// Store
// =============================================================================

const initialState: DragState = {
	data: null,
	position: null,
	hoveredDropTarget: null,
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
			})
		},

		/**
		 * Start dragging a playlist or folder
		 */
		startPlaylistDrag(playlistId: string, isFolder: boolean, x: number, y: number) {
			set({
				data: { type: 'playlist', playlistId, isFolder },
				position: { x, y },
				hoveredDropTarget: null,
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

export const isDraggingPlaylist = derived(dragStore, ($drag) => $drag.data?.type === 'playlist')
