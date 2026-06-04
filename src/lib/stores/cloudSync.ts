import { writable, derived } from 'svelte/store'
import type { CloudSyncStatus, CloudSyncPhase, CloudDeviceRecord, LibraryRoot } from '$lib/types'
import * as cloudSyncApi from '$lib/api/cloudSync'

// =============================================================================
// State
// =============================================================================

interface CloudSyncState {
	status: CloudSyncStatus
	devices: CloudDeviceRecord[]
	libraryRoots: LibraryRoot[]
	signingIn: boolean
	loading: boolean
	error: string | null
}

const initialStatus: CloudSyncStatus = {
	phase: 'disabled',
	email: null,
	device_id: '',
	device_name: '',
	last_error: null,
	last_synced_at: null,
}

const initialState: CloudSyncState = {
	status: initialStatus,
	devices: [],
	libraryRoots: [],
	signingIn: false,
	loading: false,
	error: null,
}

const POLL_INTERVAL_MS = 5000

// =============================================================================
// Store
// =============================================================================

function createCloudSyncStore() {
	const { subscribe, set, update } = writable<CloudSyncState>(initialState)

	let pollTimer: ReturnType<typeof setInterval> | null = null

	async function pollStatus() {
		try {
			const status = await cloudSyncApi.getSyncStatus()
			update((s) => ({ ...s, status }))
		} catch {
			// Silent — status polling shouldn't surface errors
		}
	}

	return {
		subscribe,

		async load() {
			update((s) => ({ ...s, loading: true, error: null }))
			try {
				const status = await cloudSyncApi.getSyncStatus()
				update((s) => ({ ...s, status, loading: false }))
			} catch (error) {
				update((s) => ({
					...s,
					loading: false,
					error: error instanceof Error ? error.message : 'Failed to load sync status',
				}))
			}
		},

		async refreshStatus() {
			try {
				const status = await cloudSyncApi.getSyncStatus()
				update((s) => ({ ...s, status }))
			} catch {
				// Silent — status polling shouldn't surface errors
			}
		},

		async signIn(providerId: string) {
			update((s) => ({ ...s, signingIn: true, error: null }))
			try {
				const status = await cloudSyncApi.signIn(providerId)
				update((s) => ({ ...s, status, signingIn: false }))
			} catch (error) {
				update((s) => ({
					...s,
					signingIn: false,
					error: error instanceof Error ? error.message : 'Sign-in failed',
				}))
			}
		},

		async signOut() {
			try {
				await cloudSyncApi.signOut()
				const status = await cloudSyncApi.getSyncStatus()
				update((s) => ({ ...s, status, devices: [], error: null }))
			} catch (error) {
				console.error('Failed to sign out:', error)
			}
		},

		async syncNow() {
			try {
				await cloudSyncApi.syncNow()
				await cloudSyncApi.pullNow()
				const status = await cloudSyncApi.getSyncStatus()
				update((s) => ({ ...s, status }))
			} catch (error) {
				const status = await cloudSyncApi.getSyncStatus().catch(() => null)
				if (status) {
					update((s) => ({ ...s, status }))
				}
				console.error('Sync failed:', error)
			}
		},

		async loadDevices() {
			try {
				const devices = await cloudSyncApi.listDevices()
				update((s) => ({ ...s, devices }))
			} catch (error) {
				console.error('Failed to load devices:', error)
			}
		},

		async renameDevice(name: string) {
			try {
				await cloudSyncApi.renameDevice(name)
				const status = await cloudSyncApi.getSyncStatus()
				update((s) => ({ ...s, status }))
				await this.loadDevices()
			} catch (error) {
				console.error('Failed to rename device:', error)
			}
		},

		async revokeDevice(deviceId: string) {
			try {
				await cloudSyncApi.revokeDevice(deviceId)
				const status = await cloudSyncApi.getSyncStatus()
				update((s) => ({ ...s, status }))
				if (status.phase !== 'signedout' && status.phase !== 'disabled') {
					await this.loadDevices()
				}
			} catch (error) {
				console.error('Failed to revoke device:', error)
			}
		},

		async loadLibraryRoots() {
			try {
				const libraryRoots = await cloudSyncApi.listLibraryRoots()
				update((s) => ({ ...s, libraryRoots }))
			} catch (error) {
				console.error('Failed to load library roots:', error)
			}
		},

		async setRootMapping(rootId: string, localPath: string) {
			try {
				await cloudSyncApi.setLibraryRootMapping(rootId, localPath)
				await this.loadLibraryRoots()
			} catch (error) {
				console.error('Failed to set root mapping:', error)
			}
		},

		async locateTrack(trackId: string, localPath: string) {
			try {
				await cloudSyncApi.locateTrack(trackId, localPath)
				await this.loadLibraryRoots()
			} catch (error) {
				console.error('Failed to locate track:', error)
			}
		},

		startPolling() {
			if (pollTimer) return
			pollTimer = setInterval(pollStatus, POLL_INTERVAL_MS)
		},

		stopPolling() {
			if (pollTimer) {
				clearInterval(pollTimer)
				pollTimer = null
			}
		},

		reset() {
			set(initialState)
		},
	}
}

export const cloudSyncStore = createCloudSyncStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const syncStatus = derived(cloudSyncStore, ($s) => $s.status)

export const syncPhase = derived(cloudSyncStore, ($s) => $s.status.phase)

export const isSignedIn = derived(
	cloudSyncStore,
	($s) => $s.status.phase === 'idle' || $s.status.phase === 'syncing' || $s.status.phase === 'error'
)

export const isSyncAvailable = derived(cloudSyncStore, ($s) => $s.status.phase !== 'disabled')

export const cloudDevices = derived(cloudSyncStore, ($s) => $s.devices)

export const libraryRoots = derived(cloudSyncStore, ($s) => $s.libraryRoots)

export const unmappedRootIds = derived(
	cloudSyncStore,
	($s) => new Set($s.libraryRoots.filter((r) => !r.local_path).map((r) => r.id))
)

export const signingIn = derived(cloudSyncStore, ($s) => $s.signingIn)
