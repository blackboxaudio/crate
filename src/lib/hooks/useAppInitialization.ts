import { listen, type UnlistenFn } from '@tauri-apps/api/event'
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
	let unlistenDevices: UnlistenFn | undefined

	// Store drag-drop event handlers for cleanup
	let dragEnterHandler: ((e: DragEvent) => void) | undefined
	let dragOverHandler: ((e: DragEvent) => void) | undefined
	let dragLeaveHandler: ((e: DragEvent) => void) | undefined
	let dropHandler: ((e: DragEvent) => void) | undefined

	// Load all stores in parallel
	await Promise.all([
		appStore.load(),
		libraryStore.loadTracks(),
		tagsStore.load(),
		playlistsStore.load(),
		settingsStore.load(),
		devicesStore.loadDevices(),
	])

	// Set up browser-native drag and drop events for external file drops
	// This approach allows both external OS file drops AND internal HTML5 drag-and-drop
	function setupDragDrop(): void {
		// Track drag enter/leave depth to handle nested elements
		let dragDepth = 0

		// Check if the drag contains files from the OS
		function hasExternalFiles(e: DragEvent): boolean {
			if (!e.dataTransfer?.types) return false
			// 'Files' type indicates files from the OS file system
			return e.dataTransfer.types.includes('Files')
		}

		dragEnterHandler = (e: DragEvent) => {
			// Debug: log ALL dragenter events to see if they're firing at all
			console.log('[Document] dragenter', {
				target: e.target,
				types: e.dataTransfer?.types ? Array.from(e.dataTransfer.types) : [],
				hasFiles: hasExternalFiles(e),
			})

			// Only handle external file drags, not internal app drags
			if (!hasExternalFiles(e)) return

			e.preventDefault()
			dragDepth++
			if (dragDepth === 1) {
				onDragStateChange(true)
			}
		}

		dragOverHandler = (e: DragEvent) => {
			// Only handle external file drags
			if (!hasExternalFiles(e)) return

			e.preventDefault()
			if (e.dataTransfer) {
				e.dataTransfer.dropEffect = 'copy'
			}
		}

		dragLeaveHandler = (e: DragEvent) => {
			// Only handle external file drags
			if (!hasExternalFiles(e)) return

			dragDepth--
			if (dragDepth === 0) {
				onDragStateChange(false)
			}
		}

		dropHandler = async (e: DragEvent) => {
			// Only handle external file drags
			if (!hasExternalFiles(e)) return

			e.preventDefault()
			dragDepth = 0
			onDragStateChange(false)

			const files = e.dataTransfer?.files
			if (!files || files.length === 0) return

			// Convert FileList to array of paths
			// Note: In Tauri, we can access the file path via the 'path' property
			const paths: string[] = []
			for (let i = 0; i < files.length; i++) {
				const file = files[i]
				// In Tauri webview, File objects have a 'path' property with the full file path
				const filePath = (file as File & { path?: string }).path
				if (filePath) {
					paths.push(filePath)
				}
			}

			if (paths.length > 0) {
				// Filter for audio files
				const audioPaths = paths.filter((p) => {
					const ext = p.split('.').pop()?.toLowerCase()
					return ext && AUDIO_EXTENSIONS.includes(ext)
				})
				if (audioPaths.length > 0) {
					await onExternalFileDrop(audioPaths)
				}
			}
		}

		// Add event listeners to document (use capture phase to catch events early)
		document.addEventListener('dragenter', dragEnterHandler, true)
		document.addEventListener('dragover', dragOverHandler, true)
		document.addEventListener('dragleave', dragLeaveHandler, true)
		document.addEventListener('drop', dropHandler, true)

		console.log('[useAppInitialization] Drag-drop event listeners registered on document (capture phase)')
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
	setupDragDrop()
	await setupDeviceListener()

	// Return cleanup function
	return () => {
		// Clean up drag-drop event listeners (must match capture phase)
		if (dragEnterHandler) document.removeEventListener('dragenter', dragEnterHandler, true)
		if (dragOverHandler) document.removeEventListener('dragover', dragOverHandler, true)
		if (dragLeaveHandler) document.removeEventListener('dragleave', dragLeaveHandler, true)
		if (dropHandler) document.removeEventListener('drop', dropHandler, true)
		// Clean up device listener
		unlistenDevices?.()
	}
}
