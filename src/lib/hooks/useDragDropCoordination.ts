import { get } from 'svelte/store'
import type { Playlist, ExportRequest, UsbDevice } from '$lib/types'
import { dragStore, isDragging, dragData, needsDropTargetRefresh } from '$lib/stores'
import { findDropTargets, findDropTargetAtPoint, type DropTarget } from '$lib/utils/drag'
import { toastStore } from '$lib/stores/toast'

// =============================================================================
// Types
// =============================================================================

export interface DragDropCoordinationConfig {
	getPlaylists: () => Playlist[]
	getDevices: () => UsbDevice[]
	onTracksDropOnPlaylist: (playlistId: string, trackIds: string[]) => Promise<void>
	onPlaylistMove: (playlistId: string, targetFolderId: string | null) => Promise<void>
	onPlaylistExportToDevice: (playlistId: string, isFolder: boolean, deviceId: string) => Promise<void>
}

// =============================================================================
// Hook
// =============================================================================

/**
 * Set up global drag-drop coordination for the application.
 *
 * Handles:
 * - Global pointer events during drag operations
 * - Drop target hit-testing
 * - Dispatching drops to appropriate handlers
 *
 * @returns Cleanup function to remove event listeners
 */
export function useDragDropCoordination(config: DragDropCoordinationConfig): () => void {
	const { getPlaylists, onTracksDropOnPlaylist, onPlaylistMove, onPlaylistExportToDevice } = config

	let dropTargets: DropTarget[] = []
	let rafId: number | null = null

	/**
	 * Check if targetId is a descendant of potentialAncestorId (prevents circular drops)
	 */
	function isDescendantOf(potentialAncestorId: string, targetId: string): boolean {
		const playlists = getPlaylists()
		let currentId: string | null = targetId
		while (currentId) {
			if (currentId === potentialAncestorId) return true
			const current = playlists.find((p) => p.id === currentId)
			currentId = current?.parent_id ?? null
		}
		return false
	}

	/**
	 * Handle global pointer move during drag
	 */
	function handleGlobalPointerMove(e: PointerEvent): void {
		if (!get(isDragging)) return

		// Use requestAnimationFrame to throttle updates
		if (rafId !== null) return

		rafId = requestAnimationFrame(() => {
			rafId = null
			dragStore.updatePosition(e.clientX, e.clientY)

			// Refresh drop targets if requested (e.g., after folder expand)
			if (get(needsDropTargetRefresh)) {
				dropTargets = findDropTargets()
				dragStore.clearDropTargetRefresh()
			}

			// Hit-test to find drop target under pointer
			const target = findDropTargetAtPoint(e.clientX, e.clientY, dropTargets)
			const targetId = target ? `${target.type}-${target.id}` : null
			dragStore.setHoveredDropTarget(targetId)
		})
	}

	/**
	 * Handle global pointer up (drop) during drag
	 */
	function handleGlobalPointerUp(e: PointerEvent): void {
		if (!get(isDragging)) return

		const data = get(dragData)
		if (!data) {
			dragStore.endDrag()
			return
		}

		// Find drop target under pointer
		const target = findDropTargetAtPoint(e.clientX, e.clientY, dropTargets)

		if (target) {
			// Handle the drop based on what we're dragging and where
			if (data.type === 'tracks' && target.type === 'playlist') {
				// Dropping tracks on a playlist
				onTracksDropOnPlaylist(target.id, data.trackIds)
			} else if (data.type === 'playlist' && target.type === 'folder') {
				// Validate: prevent dropping on self
				if (data.playlistId === target.id) {
					toastStore.error('Cannot drop a folder into itself')
					dragStore.endDrag()
					return
				}

				// Validate: prevent dropping folder into its own descendants
				if (data.isFolder && isDescendantOf(data.playlistId, target.id)) {
					toastStore.error('Cannot drop a folder into its own subfolder')
					dragStore.endDrag()
					return
				}

				// Dropping a playlist/folder on a folder
				onPlaylistMove(data.playlistId, target.id)
			} else if (data.type === 'playlist' && target.type === 'device') {
				// Dropping a playlist/folder on a device - export immediately
				onPlaylistExportToDevice(data.playlistId, data.isFolder, target.id)
			}
		}

		dragStore.endDrag()
	}

	/**
	 * Set up listeners when drag starts
	 */
	function setupListeners(): void {
		// Cache drop targets when drag starts
		dropTargets = findDropTargets()

		// Set grabbing cursor globally by adding class to html element
		document.documentElement.classList.add('is-dragging')

		// Add global listeners
		document.addEventListener('pointermove', handleGlobalPointerMove)
		document.addEventListener('pointerup', handleGlobalPointerUp)
	}

	/**
	 * Tear down listeners when drag ends
	 */
	function teardownListeners(): void {
		document.documentElement.classList.remove('is-dragging')
		document.removeEventListener('pointermove', handleGlobalPointerMove)
		document.removeEventListener('pointerup', handleGlobalPointerUp)
		if (rafId !== null) {
			cancelAnimationFrame(rafId)
			rafId = null
		}
	}

	// Subscribe to isDragging store to set up/tear down listeners
	const unsubscribe = isDragging.subscribe((dragging) => {
		if (dragging) {
			setupListeners()
		} else {
			teardownListeners()
		}
	})

	// Return cleanup function
	return () => {
		unsubscribe()
		teardownListeners()
	}
}
