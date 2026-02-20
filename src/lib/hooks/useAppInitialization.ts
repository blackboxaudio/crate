import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import type { UsbDevice } from '$lib/types'
import type { appStore as AppStoreType } from '$lib/stores/app'
import type { libraryStore as LibraryStoreType } from '$lib/stores/library'
import type { tagsStore as TagsStoreType } from '$lib/stores/tags'
import type { playlistsStore as PlaylistsStoreType } from '$lib/stores/playlists'
import type { settingsStore as SettingsStoreType } from '$lib/stores/settings'
import type { devicesStore as DevicesStoreType } from '$lib/stores/devices'
import type { syncStore as SyncStoreType } from '$lib/stores/sync'
import type { toastStore as ToastStoreType } from '$lib/stores/toast'

// =============================================================================
// Types
// =============================================================================

export interface AppInitConfig {
	stores: {
		appStore: typeof AppStoreType
		libraryStore: typeof LibraryStoreType
		tagsStore: typeof TagsStoreType
		playlistsStore: typeof PlaylistsStoreType
		settingsStore: typeof SettingsStoreType
		devicesStore: typeof DevicesStoreType
		syncStore: typeof SyncStoreType
	}
	toastStore: typeof ToastStoreType
	onExternalFileDrop: (audioPaths: string[]) => Promise<void>
	onDragStateChange: (isDragOver: boolean) => void
}

// =============================================================================
// Constants
// =============================================================================

const AUDIO_EXTENSIONS = ['mp3', 'wav', 'aiff', 'aif', 'flac', 'm4a', 'aac']

/** Whether the current native drag contains audio files. Used by the layout's dragover handler to control the OS drop cursor. */
export let hasAudioDrag = false

// =============================================================================
// Hook
// =============================================================================

/**
 * Initialize the application: load stores, set up drag-drop, and device listener.
 *
 * @returns Promise of cleanup function to remove all listeners
 */
export async function useAppInitialization(config: AppInitConfig): Promise<() => void> {
	const { stores, toastStore, onExternalFileDrop, onDragStateChange } = config
	const { appStore, libraryStore, tagsStore, playlistsStore, settingsStore, devicesStore, syncStore } = stores

	// Store unlisten functions
	let unlistenDevices: UnlistenFn | undefined
	let unlistenDragDrop: UnlistenFn | undefined

	// Load all stores in parallel
	await Promise.all([
		appStore.load(),
		libraryStore.loadTracks(),
		tagsStore.load(),
		playlistsStore.load(),
		settingsStore.load(),
		devicesStore.loadDevices(),
	])

	// Set up Tauri's native drag-drop event listener for external file drops
	// This uses Tauri's onDragDropEvent API which provides file paths directly
	async function setupDragDrop(): Promise<void> {
		const webview = getCurrentWebview()

		unlistenDragDrop = await webview.onDragDropEvent((event) => {
			const { type } = event.payload

			if (type === 'enter') {
				const paths = event.payload.paths
				hasAudioDrag = !!paths?.some((p) => {
					const ext = p.split('.').pop()?.toLowerCase()
					return ext && AUDIO_EXTENSIONS.includes(ext)
				})
				if (hasAudioDrag) {
					onDragStateChange(true)
				}
			} else if (type === 'leave') {
				hasAudioDrag = false
				onDragStateChange(false)
			} else if (type === 'drop') {
				hasAudioDrag = false
				onDragStateChange(false)

				// File paths are provided directly by Tauri
				const paths = event.payload.paths
				if (!paths || paths.length === 0) return

				// Filter for audio files
				const audioPaths = paths.filter((p) => {
					const ext = p.split('.').pop()?.toLowerCase()
					return ext && AUDIO_EXTENSIONS.includes(ext)
				})

				if (audioPaths.length > 0) {
					onExternalFileDrop(audioPaths)
				}
			}
			// 'over' events are ignored - we don't need position tracking for now
		})
	}

	// Set up device change listener
	async function setupDeviceListener(): Promise<void> {
		unlistenDevices = await listen<UsbDevice[]>('devices-changed', (event) => {
			const previousDevices = devicesStore.getDevices()
			const newDevices = event.payload
			const reformattingId = devicesStore.getReformattingDeviceId()

			// Get ignored device IDs from settings
			let ignoredIds: string[] = []
			settingsStore.subscribe((state) => {
				ignoredIds = state.ignoredDeviceIds
			})()

			// Detect new devices (connected)
			const prevIds = new Set(previousDevices.map((d) => d.id))
			for (const device of newDevices) {
				if (!prevIds.has(device.id)) {
					// Skip toast and auto-sync for ignored devices
					if (ignoredIds.includes(device.id)) {
						continue
					}

					// Suppress toast if a reformat is in progress
					if (!reformattingId) {
						toastStore.info(`${device.name} connected`)
					}

					// Trigger auto-sync on device connected (if enabled)
					syncStore.onDeviceConnected(device)
				}
			}

			// Detect removed devices (disconnected)
			const newIds = new Set(newDevices.map((d) => d.id))
			for (const device of previousDevices) {
				if (!newIds.has(device.id)) {
					// Skip toast for ignored devices
					if (ignoredIds.includes(device.id)) {
						continue
					}

					// Suppress toast if this device is being reformatted
					if (reformattingId !== device.id) {
						toastStore.info(`${device.name} disconnected`)
					}
				}
			}

			devicesStore.setDevices(newDevices)
		})
	}

	// Initialize listeners
	await setupDragDrop()
	await setupDeviceListener()

	// Return cleanup function
	return () => {
		unlistenDragDrop?.()
		unlistenDevices?.()
	}
}
