import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import type { DiscoveryRelease, FollowedReleasesFound, UsbDevice } from '$lib/types'
import type { appStore as AppStoreType } from '$lib/stores/app'
import type { libraryStore as LibraryStoreType } from '$lib/stores/library'
import type { tagsStore as TagsStoreType } from '$lib/stores/tags'
import type { playlistsStore as PlaylistsStoreType } from '$lib/stores/playlists'
import type { settingsStore as SettingsStoreType } from '$lib/stores/settings'
import type { devicesStore as DevicesStoreType } from '$lib/stores/devices'
import type { syncStore as SyncStoreType } from '$lib/stores/sync'
import type { toastStore as ToastStoreType } from '$lib/stores/toast'
import type { discoveryStore as DiscoveryStoreType } from '$lib/stores/discovery'
import { followStore } from '$lib/stores/follow'
import { uiStore } from '$lib/stores/ui'
import { translate } from '$lib/i18n'
import { get } from 'svelte/store'

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
		discoveryStore: typeof DiscoveryStoreType
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
	const { appStore, libraryStore, tagsStore, playlistsStore, settingsStore, devicesStore, syncStore, discoveryStore } =
		stores

	// Store unlisten functions
	let unlistenDevices: UnlistenFn | undefined
	let unlistenDragDrop: UnlistenFn | undefined
	let unlistenDiscoveryUpdate: UnlistenFn | undefined
	let unlistenEnrichmentQueued: UnlistenFn | undefined
	let unlistenCloudSyncMerge: UnlistenFn | undefined
	let unlistenFollowed: UnlistenFn | undefined

	// Load all stores in parallel
	await Promise.all([
		appStore.load(),
		libraryStore.loadTracks(),
		tagsStore.load(),
		playlistsStore.load(),
		settingsStore.load(),
		devicesStore.loadDevices(),
		followStore.load(),
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

	// Set up discovery release update listener (background enrichment)
	async function setupDiscoveryUpdateListener(): Promise<void> {
		unlistenDiscoveryUpdate = await listen<DiscoveryRelease>('discovery-release-updated', (event) => {
			discoveryStore.replaceRelease(event.payload)
		})
	}

	// Set up enrichment queued listener (shows spinners on releases about to be enriched)
	async function setupEnrichmentQueuedListener(): Promise<void> {
		unlistenEnrichmentQueued = await listen<string[]>('discovery-enrichment-queued', (event) => {
			discoveryStore.markEnriching(event.payload)
		})
	}

	// Set up cloud-sync merge listener: a background pull/push (or "Sync now") merged a
	// peer's changes into the local DB. The payload is the list of merged sync buckets;
	// reload only the stores those buckets feed so the change shows without an app restart.
	async function setupCloudSyncMergeListener(): Promise<void> {
		unlistenCloudSyncMerge = await listen<string[]>('cloud-sync-merged', (event) => {
			reloadStoresForBuckets(event.payload)
		})
	}

	// Set up followed-releases listener: a background check surfaced new releases. Bump
	// the per-source new counts and reload Discovery so the new rows appear. When the window
	// is focused, show an in-app "Review" toast (the backend suppresses its native summary
	// notification while focused, so this is the only surface — no double-notify); when
	// backgrounded, the native notification is the backend's job and we stay quiet.
	async function setupFollowedReleasesListener(): Promise<void> {
		unlistenFollowed = await listen<FollowedReleasesFound>('followed-releases-found', (event) => {
			followStore.applyAggregate(event.payload)
			const { totalNew } = event.payload
			if (totalNew > 0) {
				discoveryStore.loadReleases()
				if (document.hasFocus()) {
					toastStore.info(get(translate)('discovery.following.foundToast', { values: { count: totalNew } }), 8000, {
						label: get(translate)('discovery.following.review'),
						onClick: () => {
							uiStore.setActiveView('discovery')
							discoveryStore.toggleNewFilter(true)
						},
					})
				}
			}
		})
	}

	// Map merged sync buckets to the stores that must reload. Tag name/color and track-tag
	// links are embedded on tracks and discovery releases, so a tag-related merge reloads
	// those stores too.
	function reloadStoresForBuckets(buckets: string[]): void {
		let library = false
		let playlists = false
		let tags = false
		let discovery = false
		let settings = false
		let follow = false

		for (const bucket of buckets) {
			if (bucket.startsWith('tracks/') || bucket === 'cues' || bucket === 'library_roots') {
				library = true
			} else if (bucket === 'playlists' || bucket === 'playlist_tracks') {
				playlists = true
			} else if (bucket === 'playlist_discovery_releases') {
				playlists = true
				discovery = true
			} else if (bucket === 'tag_categories' || bucket === 'tags') {
				tags = true
				library = true
				discovery = true
			} else if (bucket === 'track_tags') {
				tags = true
				library = true
			} else if (bucket === 'discovery_release_tags') {
				tags = true
				discovery = true
			} else if (bucket === 'discovery_releases' || bucket === 'discovery_tracks') {
				discovery = true
			} else if (bucket === 'followed_sources') {
				follow = true
			} else if (bucket === 'discovery_release_sources') {
				follow = true
				discovery = true
			} else if (bucket === 'settings') {
				settings = true
			}
		}

		if (library) libraryStore.loadTracks()
		if (playlists) playlistsStore.load()
		if (tags) tagsStore.load()
		if (discovery) discoveryStore.loadReleases()
		if (settings) settingsStore.load()
		if (follow) followStore.load()
	}

	// Initialize listeners
	await setupDragDrop()
	await setupDeviceListener()
	await setupDiscoveryUpdateListener()
	await setupEnrichmentQueuedListener()
	await setupCloudSyncMergeListener()
	await setupFollowedReleasesListener()

	// Return cleanup function
	return () => {
		unlistenDragDrop?.()
		unlistenDevices?.()
		unlistenDiscoveryUpdate?.()
		unlistenEnrichmentQueued?.()
		unlistenCloudSyncMerge?.()
		unlistenFollowed?.()
	}
}
