import { writable, derived } from 'svelte/store'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { ExportProgress, ExportResult } from '$lib/types'

// =============================================================================
// Types
// =============================================================================

interface ExportState {
	isExporting: boolean
	progress: ExportProgress | null
	activeDeviceId: string | null
	activeDeviceName: string | null
	playlistCount: number | null
	lastResult: ExportResult | null
	error: string | null
}

// =============================================================================
// State
// =============================================================================

const initialState: ExportState = {
	isExporting: false,
	progress: null,
	activeDeviceId: null,
	activeDeviceName: null,
	playlistCount: null,
	lastResult: null,
	error: null,
}

// =============================================================================
// Store
// =============================================================================

function createExportStore() {
	const { subscribe, set, update } = writable<ExportState>(initialState)

	let unlisten: UnlistenFn | null = null

	return {
		subscribe,

		/**
		 * Start listening for export progress events
		 */
		async startListening() {
			if (unlisten) return

			unlisten = await listen<ExportProgress>('export-progress', (event) => {
				update((state) => ({
					...state,
					progress: event.payload,
					// Update isExporting based on status
					isExporting: event.payload.status !== 'completed' && event.payload.status !== 'failed',
				}))
			})
		},

		/**
		 * Stop listening for export progress events
		 */
		stopListening() {
			if (unlisten) {
				unlisten()
				unlisten = null
			}
		},

		/**
		 * Mark export as started
		 */
		startExport(deviceId: string, deviceName: string, playlistCount: number) {
			update((state) => ({
				...state,
				isExporting: true,
				activeDeviceId: deviceId,
				activeDeviceName: deviceName,
				playlistCount,
				error: null,
				lastResult: null,
			}))
		},

		/**
		 * Mark export as completed
		 */
		completeExport(result: ExportResult) {
			update((state) => ({
				...state,
				isExporting: false,
				activeDeviceId: null,
				activeDeviceName: null,
				playlistCount: null,
				lastResult: result,
				progress: null,
			}))
		},

		/**
		 * Mark export as failed
		 */
		failExport(error: string) {
			update((state) => ({
				...state,
				isExporting: false,
				activeDeviceId: null,
				activeDeviceName: null,
				playlistCount: null,
				error,
			}))
		},

		/**
		 * Clear the last result
		 */
		clearResult() {
			update((state) => ({
				...state,
				lastResult: null,
				error: null,
			}))
		},

		/**
		 * Reset to initial state
		 */
		reset() {
			set(initialState)
		},
	}
}

export const exportStore = createExportStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const isExporting = derived(exportStore, ($store) => $store.isExporting)

export const exportProgress = derived(exportStore, ($store) => $store.progress)

export const activeDeviceId = derived(exportStore, ($store) => $store.activeDeviceId)

export const activeDeviceName = derived(exportStore, ($store) => $store.activeDeviceName)

export const playlistCount = derived(exportStore, ($store) => $store.playlistCount)

export const exportError = derived(exportStore, ($store) => $store.error)

export const lastExportResult = derived(exportStore, ($store) => $store.lastResult)

/**
 * Export progress as a percentage (0-100)
 */
export const exportProgressPercent = derived(exportStore, ($store) => {
	if (!$store.progress) return 0
	if ($store.progress.files_total === 0) return 0
	return Math.round(($store.progress.files_copied / $store.progress.files_total) * 100)
})

/**
 * Human-readable export status
 */
export const exportStatusLabel = derived(exportStore, ($store) => {
	if (!$store.progress) return ''

	switch ($store.progress.status) {
		case 'pending':
			return 'Preparing...'
		case 'copying':
			return 'Copying files...'
		case 'generating_database':
			return 'Generating database...'
		case 'completed':
			return 'Complete'
		case 'failed':
			return 'Failed'
		default:
			return ''
	}
})
