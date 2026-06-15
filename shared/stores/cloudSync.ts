import { writable, derived, get } from 'svelte/store'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { CloudSyncStatus, CloudSyncPhase, CloudDeviceRecord, LibraryRoot } from '../types'
import * as cloudSyncApi from '../api/cloudSync'
import { translate } from '../i18n'
import { toastStore } from './toast'

/** Payload of the backend `cloud-sync-override` event (one per discarded local edit). */
type OverrideNotice = { label: string; device: string }

/**
 * Native mobile auth-session function, injected by the mobile app so the web-auth plugin import
 * stays out of `shared/` (and the desktop bundle). Backed by `tauri-plugin-web-auth`'s
 * `authenticate` — iOS `ASWebAuthenticationSession` / Android Custom Tabs.
 */
export type WebAuthFn = (opts: { url: string; callbackScheme: string }) => Promise<{ callbackUrl: string }>

/**
 * Extract a human-readable message from an unknown thrown value. Tauri command errors reject with
 * the *serialized* error (often a plain string), so `instanceof Error` alone drops the real cause.
 */
function describeError(error: unknown, fallback: string): string {
	if (error instanceof Error) return error.message
	if (typeof error === 'string' && error.trim()) return error
	if (error && typeof error === 'object') {
		const m = (error as { message?: unknown }).message
		if (typeof m === 'string' && m.trim()) return m
	}
	return fallback
}

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
	display_name: null,
	photo_url: null,
	device_id: '',
	device_name: '',
	last_error: null,
	last_synced_at: null,
	onboarding: null,
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
	let overrideUnlisten: UnlistenFn | null = null

	async function pollStatus() {
		try {
			const status = await cloudSyncApi.getSyncStatus()
			update((s) => ({ ...s, status }))
		} catch {
			// Silent — status polling shouldn't surface errors
		}
	}

	/**
	 * Apply the status returned by a successful sign-in (desktop loopback or mobile native flow)
	 * and drive first-sign-in onboarding. The dirty queue is empty on a fresh sign-in, so the
	 * debounce loop won't push on its own — kick the initial op here. Fire-and-forget; the
	 * indicator reflects progress via polling.
	 */
	function applyStatusAfterSignIn(status: CloudSyncStatus) {
		update((s) => ({ ...s, status, signingIn: false, error: null }))
		if (status.onboarding === 'initial') {
			// First device: upload the local library as the initial vault.
			cloudSyncApi
				.syncNow()
				.then(pollStatus)
				.catch((e) => console.error('Initial sync failed:', e))
		} else if (status.onboarding === 'restore') {
			// Fresh device: pull the vault. The Cloud Sync tab surfaces the roots wizard once the
			// pulled roots land (it reloads on each completed sync).
			cloudSyncApi
				.pullNow()
				.then(pollStatus)
				.catch((e) => console.error('Restore pull failed:', e))
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
				applyStatusAfterSignIn(status)
			} catch (error) {
				update((s) => ({
					...s,
					signingIn: false,
					error: describeError(error, 'Sign-in failed'),
				}))
			}
		},

		/**
		 * Native mobile sign-in. `authenticate` is injected by the mobile app
		 * (`tauri-plugin-web-auth`) so the plugin import never enters `shared/`. Runs the two-step
		 * backend flow: `begin_sign_in` → present the native auth session → extract `code`/`state`
		 * from the callback URL → `complete_sign_in`.
		 */
		async signInMobile(providerId: string, authenticate: WebAuthFn) {
			update((s) => ({ ...s, signingIn: true, error: null }))
			try {
				const { authUrl, callbackScheme } = await cloudSyncApi.beginSignIn(providerId)
				const { callbackUrl } = await authenticate({ url: authUrl, callbackScheme })
				const url = new URL(callbackUrl)
				const errParam = url.searchParams.get('error')
				if (errParam) throw new Error(errParam)
				const code = url.searchParams.get('code')
				const oauthState = url.searchParams.get('state')
				if (!code || !oauthState) throw new Error('Missing code or state in OAuth callback')
				const status = await cloudSyncApi.completeSignIn(code, oauthState)
				applyStatusAfterSignIn(status)
			} catch (error) {
				update((s) => ({
					...s,
					signingIn: false,
					error: describeError(error, 'Sign-in failed'),
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
			update((s) => ({ ...s, status: { ...s.status, phase: 'syncing' as CloudSyncPhase } }))
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
			update((s) => ({
				...s,
				status: { ...s.status, device_name: name },
				devices: s.devices.map((d) => (d.device_id === s.status.device_id ? { ...d, name } : d)),
			}))
			try {
				await cloudSyncApi.renameDevice(name)
			} catch (error) {
				console.error('Failed to rename device:', error)
				toastStore.error(get(translate)('cloudSync.devices.renameFailed'))
				throw error
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

		async deleteCloudVault() {
			try {
				await cloudSyncApi.deleteCloudVault()
				// The backend signs out after wiping; reflect the signed-out state.
				const status = await cloudSyncApi.getSyncStatus()
				update((s) => ({ ...s, status, devices: [], libraryRoots: [], error: null }))
			} catch (error) {
				console.error('Failed to delete cloud vault:', error)
				toastStore.error(get(translate)('cloudSync.danger.error'))
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

		/** Listen for override conflicts and toast the discarded edit's owner. */
		async startOverrideListener() {
			if (overrideUnlisten) return
			overrideUnlisten = await listen<OverrideNotice[]>('cloud-sync-override', (event) => {
				const t = get(translate)
				// Cap the burst so a large concurrent merge can't flood the UI.
				for (const notice of event.payload.slice(0, 5)) {
					toastStore.warning(
						t('cloudSync.conflicts.overridden', { values: { label: notice.label, device: notice.device } })
					)
				}
			})
		},

		stopOverrideListener() {
			if (overrideUnlisten) {
				overrideUnlisten()
				overrideUnlisten = null
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
	($s) =>
		$s.status.phase === 'idle' ||
		$s.status.phase === 'syncing' ||
		$s.status.phase === 'offline' ||
		$s.status.phase === 'error'
)

export const isSyncAvailable = derived(cloudSyncStore, ($s) => $s.status.phase !== 'disabled')

export const cloudDevices = derived(cloudSyncStore, ($s) => $s.devices)

export const libraryRoots = derived(cloudSyncStore, ($s) => $s.libraryRoots)

export const unmappedRootIds = derived(
	cloudSyncStore,
	($s) => new Set($s.libraryRoots.filter((r) => !r.local_path).map((r) => r.id))
)

export const signingIn = derived(cloudSyncStore, ($s) => $s.signingIn)

export const cloudSyncError = derived(cloudSyncStore, ($s) => $s.error)
