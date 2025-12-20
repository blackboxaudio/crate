import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { UsbDevice } from '$lib/types'
import type { appStore as AppStoreType } from '$lib/stores/app'
import type { libraryStore as LibraryStoreType } from '$lib/stores/library'
import type { tagsStore as TagsStoreType } from '$lib/stores/tags'
import type { playlistsStore as PlaylistsStoreType } from '$lib/stores/playlists'
import type { settingsStore as SettingsStoreType } from '$lib/stores/settings'
import type { devicesStore as DevicesStoreType } from '$lib/stores/devices'
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
	}
	toastStore: typeof ToastStoreType
	onExternalFileDrop: (audioPaths: string[]) => Promise<void>
	onDragStateChange: (isDragOver: boolean) => void
}

// =============================================================================
// Constants
// =============================================================================

const AUDIO_EXTENSIONS = ['mp3', 'wav', 'aiff', 'aif', 'flac', 'm4a', 'aac']

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
	const { appStore, libraryStore, tagsStore, playlistsStore, settingsStore, devicesStore } = stores

	// Store unlisten functions
	let unlistenDrop: UnlistenFn | undefined
	let unlistenDevices: UnlistenFn | undefined

	// Load all stores in parallel
	await Promise.all([
		appStore.load(),
		libraryStore.loadTracks(),
		tagsStore.load(),
		playlistsStore.load(),
		settingsStore.load(),
		devicesStore.loadDevices(),
	])

	// Set up Tauri drag and drop events
	async function setupDragDrop(): Promise<void> {
		const appWindow = getCurrentWindow()

		// Listen for file drop from OS file explorer
		// Note: Tauri's onDragDropEvent only fires for external OS file drags,
		// not for internal HTML5 drags (like dragging tracks to playlists)
		unlistenDrop = await appWindow.onDragDropEvent(async (event) => {
			if (event.payload.type === 'drop') {
				onDragStateChange(false)
				const paths = event.payload.paths
				if (paths && paths.length > 0) {
					// Filter for audio files
					const audioPaths = paths.filter((p) => {
						const ext = p.split('.').pop()?.toLowerCase()
						return ext && AUDIO_EXTENSIONS.includes(ext)
					})
					if (audioPaths.length > 0) {
						await onExternalFileDrop(audioPaths)
					}
				}
			} else if (event.payload.type === 'enter') {
				// 'enter' event fires when external files are dragged into the window
				// and includes the file paths. 'over' events don't include paths.
				if (event.payload.paths && event.payload.paths.length > 0) {
					onDragStateChange(true)
				}
			} else if (event.payload.type === 'leave' || event.payload.type === 'cancel') {
				onDragStateChange(false)
			}
		})
	}

	// Set up device change listener
	async function setupDeviceListener(): Promise<void> {
		unlistenDevices = await listen<UsbDevice[]>('devices-changed', (event) => {
			const previousDevices = devicesStore.getDevices()
			const newDevices = event.payload

			// Detect new devices (connected)
			const prevIds = new Set(previousDevices.map((d) => d.id))
			for (const device of newDevices) {
				if (!prevIds.has(device.id)) {
					toastStore.info(`${device.name} connected`)
				}
			}

			// Detect removed devices (disconnected)
			const newIds = new Set(newDevices.map((d) => d.id))
			for (const device of previousDevices) {
				if (!newIds.has(device.id)) {
					toastStore.info(`${device.name} disconnected`)
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
		unlistenDrop?.()
		unlistenDevices?.()
	}
}
