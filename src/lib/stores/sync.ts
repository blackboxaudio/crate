import { writable, derived, get } from 'svelte/store'
import type { UsbDevice } from '$lib/types'
import * as syncApi from '$lib/api/sync'
import { devicesStore } from './devices'
import { settingsStore } from './settings'

// =============================================================================
// Constants
// =============================================================================

const DEBOUNCE_MS = 3000

// =============================================================================
// Types
// =============================================================================

interface SyncState {
	/** Device IDs currently being synced */
	syncingDeviceIds: string[]
	/** Playlist IDs that have pending changes (accumulated during debounce) */
	pendingPlaylistIds: string[]
	/** Last error message */
	error: string | null
}

// =============================================================================
// State
// =============================================================================

const initialState: SyncState = {
	syncingDeviceIds: [],
	pendingPlaylistIds: [],
	error: null,
}

// =============================================================================
// Store
// =============================================================================

function createSyncStore() {
	const { subscribe, set, update } = writable<SyncState>(initialState)

	let debounceTimer: ReturnType<typeof setTimeout> | null = null

	return {
		subscribe,

		/**
		 * Check if a specific device is currently syncing
		 */
		isDeviceSyncing(deviceId: string): boolean {
			let syncing = false
			subscribe((state) => {
				syncing = state.syncingDeviceIds.includes(deviceId)
			})()
			return syncing
		},

		/**
		 * Add playlist IDs to pending changes (for debouncing)
		 */
		addPendingChanges(playlistIds: string[]) {
			update((state) => ({
				...state,
				pendingPlaylistIds: [...new Set([...state.pendingPlaylistIds, ...playlistIds])],
			}))

			// Reset debounce timer
			if (debounceTimer) {
				clearTimeout(debounceTimer)
			}

			// Check if auto-sync on change is enabled
			const settings = get(settingsStore)
			if (!settings.autoSyncOnChange) {
				return
			}

			debounceTimer = setTimeout(() => {
				this.triggerPendingSync()
			}, DEBOUNCE_MS)
		},

		/**
		 * Clear pending changes
		 */
		clearPendingChanges() {
			update((state) => ({
				...state,
				pendingPlaylistIds: [],
			}))
		},

		/**
		 * Trigger sync for all pending changes
		 */
		async triggerPendingSync() {
			const state = get({ subscribe })
			const playlistIds = state.pendingPlaylistIds

			if (playlistIds.length === 0) {
				return
			}

			// Clear pending changes immediately
			this.clearPendingChanges()

			// Get connected devices
			const devices = devicesStore.getDevices()

			// For each device, check if it has any of these playlists exported
			for (const device of devices) {
				try {
					const devicePlaylistIds = await syncApi.getDevicesForPlaylists(playlistIds)
					const affectedDevice = devicePlaylistIds.find((d) => d.deviceId === device.id)

					if (affectedDevice) {
						// Get the specific playlists that need syncing for this device
						const pendingPlaylists = await syncApi.getPendingSyncPlaylists(device.id)
						if (pendingPlaylists.length > 0) {
							await this.syncDevice(device, pendingPlaylists)
						}
					}
				} catch (error) {
					console.error(`Failed to check sync status for device ${device.id}:`, error)
				}
			}
		},

		/**
		 * Sync a device with specified playlists
		 */
		async syncDevice(device: UsbDevice, playlistIds: string[]) {
			// Mark device as syncing
			update((state) => ({
				...state,
				syncingDeviceIds: [...state.syncingDeviceIds, device.id],
				error: null,
			}))

			try {
				await syncApi.syncDevice(device.id, device.name, device.mount_point, playlistIds)
			} catch (error) {
				console.error(`Failed to sync device ${device.id}:`, error)
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Sync failed',
				}))
			} finally {
				// Remove device from syncing list
				update((state) => ({
					...state,
					syncingDeviceIds: state.syncingDeviceIds.filter((id) => id !== device.id),
				}))
			}
		},

		/**
		 * Check and sync a device on connection (if auto-sync on connect is enabled)
		 */
		async onDeviceConnected(device: UsbDevice) {
			const settings = get(settingsStore)
			if (!settings.autoSyncOnConnect) {
				return
			}

			try {
				// Check if this device has pending changes
				const pendingPlaylists = await syncApi.getPendingSyncPlaylists(device.id)
				if (pendingPlaylists.length > 0) {
					await this.syncDevice(device, pendingPlaylists)
				}
			} catch (error) {
				console.error(`Failed to check pending sync for device ${device.id}:`, error)
			}
		},

		/**
		 * Notify that tracks have been modified (triggers sync if auto-sync is enabled)
		 * Looks up which playlists contain the modified tracks and adds them to pending changes
		 */
		async notifyTrackChanges(trackIds: string[]) {
			if (trackIds.length === 0) return

			try {
				const playlistIds = await syncApi.getPlaylistsContainingTracks(trackIds)
				if (playlistIds.length > 0) {
					this.addPendingChanges(playlistIds)
				}
			} catch (error) {
				console.error('Failed to get playlists for track changes:', error)
			}
		},

		/**
		 * Notify that playlists have been modified (triggers sync if auto-sync is enabled)
		 */
		notifyPlaylistChanges(playlistIds: string[]) {
			if (playlistIds.length === 0) return
			this.addPendingChanges(playlistIds)
		},

		/**
		 * Cancel any active debounce timer
		 */
		cancelDebounce() {
			if (debounceTimer) {
				clearTimeout(debounceTimer)
				debounceTimer = null
			}
		},

		/**
		 * Reset to initial state
		 */
		reset() {
			this.cancelDebounce()
			set(initialState)
		},
	}
}

export const syncStore = createSyncStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const syncingDeviceIds = derived(syncStore, ($store) => $store.syncingDeviceIds)

export const isSyncing = derived(syncStore, ($store) => $store.syncingDeviceIds.length > 0)

export const pendingPlaylistIds = derived(syncStore, ($store) => $store.pendingPlaylistIds)

export const hasPendingChanges = derived(syncStore, ($store) => $store.pendingPlaylistIds.length > 0)

export const syncError = derived(syncStore, ($store) => $store.error)
