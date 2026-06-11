import { get } from 'svelte/store'
import { translate } from '$shared/i18n'
import type { ExportRequest, Playlist, UsbDevice } from '$shared/types'
import type { exportStore as ExportStoreType } from '$lib/stores/export'
import type { toastStore as ToastStoreType } from '$shared/stores/toast'
import { isExporting } from '$lib/stores/export'
import { exportFormat } from '$shared/stores/settings'
import * as exportApi from '$shared/api/export'

// =============================================================================
// Types
// =============================================================================

export interface ExportControllerDeps {
	exportStore: typeof ExportStoreType
	toastStore: typeof ToastStoreType
	getDevices: () => UsbDevice[]
	getPlaylists: () => Playlist[]
}

export interface ExportControllerModalActions {
	openExportToDeviceModal: (device: UsbDevice) => void
	openExportPlaylistModal: (playlist: Playlist) => void
	openQuickExportModal: () => void
	openExportFailureModal: (error: string, deviceId: string, mountPoint: string, filesCopied: number) => void
}

export interface ExportController {
	handleDeviceExport: (device: UsbDevice) => void
	handlePlaylistExport: (playlist: Playlist) => void
	handleExportSubmit: (request: ExportRequest) => Promise<void>
	handleQuickExportSubmit: (requests: ExportRequest[]) => Promise<void>
	handleExportCancel: () => Promise<void>
	handleExportFailureKeep: () => void
	handleExportFailureCleanup: (deviceId: string, mountPoint: string) => Promise<void>
	handlePlaylistDropOnDevice: (playlistId: string, isFolder: boolean, deviceId: string) => Promise<void>
	getExportPlaylistIds: (playlistId: string, isFolder: boolean) => string[]
}

// =============================================================================
// Controller Factory
// =============================================================================

export function createExportController(
	deps: ExportControllerDeps,
	modalActions: ExportControllerModalActions
): ExportController {
	const { exportStore, toastStore, getDevices, getPlaylists } = deps

	/**
	 * Get all playlist IDs to export from a playlist/folder (single or recursive)
	 */
	function getExportPlaylistIds(playlistId: string, isFolder: boolean): string[] {
		if (!isFolder) {
			return [playlistId]
		}

		const playlists = getPlaylists()

		// For folders, recursively collect all non-folder playlist IDs
		const playlistIds: string[] = []
		function collectDescendants(parentId: string) {
			const children = playlists.filter((p) => p.parent_id === parentId)
			for (const child of children) {
				if (!child.is_folder) {
					playlistIds.push(child.id)
				} else {
					collectDescendants(child.id)
				}
			}
		}
		collectDescendants(playlistId)
		return playlistIds
	}

	/**
	 * Open export to device modal
	 */
	function handleDeviceExport(device: UsbDevice): void {
		modalActions.openExportToDeviceModal(device)
	}

	/**
	 * Open export playlist modal
	 */
	function handlePlaylistExport(playlist: Playlist): void {
		modalActions.openExportPlaylistModal(playlist)
	}

	/**
	 * Submit an export request
	 */
	async function handleExportSubmit(request: ExportRequest): Promise<void> {
		exportStore.startExport(request.device_id, request.device_name, request.playlist_ids.length)

		try {
			const result = await exportApi.exportToDevice(request)
			exportStore.completeExport(result)

			if (result.success) {
				toastStore.success(
					get(translate)('toast.tracksExported', {
						values: {
							exported: result.tracks_copied,
							skipped: result.tracks_skipped,
							deviceName: request.device_name,
						},
					})
				)
			} else {
				// Export completed but with errors
				const errorMsg = result.errors.length > 0 ? result.errors[0] : 'Unknown error'
				modalActions.openExportFailureModal(errorMsg, request.device_id, request.mount_point, result.tracks_copied)
			}
		} catch (error) {
			const errorMsg = error instanceof Error ? error.message : 'Export failed'
			exportStore.failExport(errorMsg)
			modalActions.openExportFailureModal(errorMsg, request.device_id, request.mount_point, 0)
		}
	}

	/**
	 * Submit multiple export requests (quick export)
	 */
	async function handleQuickExportSubmit(requests: ExportRequest[]): Promise<void> {
		// Export to each device sequentially
		for (const request of requests) {
			await handleExportSubmit(request)
		}
	}

	/**
	 * Cancel an in-progress export
	 */
	async function handleExportCancel(): Promise<void> {
		try {
			await exportApi.cancelExport()
			exportStore.reset()
		} catch (error) {
			console.error('Failed to cancel export:', error)
		}
	}

	/**
	 * Keep partial export files after failure
	 */
	function handleExportFailureKeep(): void {
		exportStore.reset()
	}

	/**
	 * Clean up partial export files after failure
	 */
	async function handleExportFailureCleanup(deviceId: string, mountPoint: string): Promise<void> {
		try {
			await exportApi.cleanupFailedExport(deviceId, mountPoint)
			toastStore.success('Cleaned up partial export')
		} catch (error) {
			toastStore.error('Failed to clean up export')
			console.error('Cleanup error:', error)
		}
		exportStore.reset()
	}

	/**
	 * Handle dropping a playlist/folder onto a device
	 */
	async function handlePlaylistDropOnDevice(playlistId: string, isFolder: boolean, deviceId: string): Promise<void> {
		// Check if already exporting
		if (get(isExporting)) {
			toastStore.error(get(translate)('errors.exportInProgress'))
			return
		}

		// Find the device
		const devices = getDevices()
		const device = devices.find((d) => d.id === deviceId)
		if (!device) {
			toastStore.error('Device not found')
			return
		}

		// Get playlist IDs to export
		const playlistIds = getExportPlaylistIds(playlistId, isFolder)

		// Check if folder has no playlists
		if (playlistIds.length === 0) {
			toastStore.error(get(translate)('export.noPlaylistsInFolder'))
			return
		}

		// Build export request
		const request: ExportRequest = {
			device_id: device.id,
			mount_point: device.mount_point,
			device_name: device.name,
			playlist_ids: playlistIds,
			enable_sync: true,
			use_device_library_plus: get(exportFormat) === 'device_library_plus',
		}

		// Start export immediately
		await handleExportSubmit(request)
	}

	return {
		handleDeviceExport,
		handlePlaylistExport,
		handleExportSubmit,
		handleQuickExportSubmit,
		handleExportCancel,
		handleExportFailureKeep,
		handleExportFailureCleanup,
		handlePlaylistDropOnDevice,
		getExportPlaylistIds,
	}
}
