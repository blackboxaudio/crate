import { writable, derived, get } from 'svelte/store'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { CloudSyncStatus, CloudSyncPhase, CloudSyncErrorKind, CloudDeviceRecord, LibraryRoot } from '../types'
import * as cloudSyncApi from '../api/cloudSync'
import { translate } from '../i18n'
import { toastStore } from './toast'
import { isMobile } from '../utils/platform'

/**
 * Arm/disarm opportunistic background sync (mobile: iOS BGTaskScheduler / Android WorkManager).
 * Guarded to mobile so the desktop bundle never invokes the mobile-only command; errors are
 * swallowed (the schedule is best-effort and the OS owns actual cadence).
 */
function armBackgroundSync() {
	if (!isMobile()) return
	void cloudSyncApi.scheduleBackgroundSync().catch(() => {})
}
function disarmBackgroundSync() {
	if (!isMobile()) return
	void cloudSyncApi.cancelBackgroundSync().catch(() => {})
}

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
	deletingAccount: boolean
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
	last_error_kind: null,
	last_synced_at: null,
	onboarding: null,
}

/**
 * The i18n key for a sync failure's headline, by error category. Falls back to the
 * generic `cloudSync.status.error` when the backend didn't classify the failure.
 */
export function syncErrorMessageKey(kind: CloudSyncErrorKind | null | undefined): string {
	switch (kind) {
		case 'network':
		case 'auth':
		case 'permission':
		case 'quota':
		case 'toolarge':
		case 'conflict':
		case 'server':
		case 'merge':
			return `cloudSync.errors.${kind}`
		default:
			return 'cloudSync.status.error'
	}
}

const initialState: CloudSyncState = {
	status: initialStatus,
	devices: [],
	libraryRoots: [],
	signingIn: false,
	loading: false,
	deletingAccount: false,
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
	let foregroundCleanup: (() => void) | null = null

	async function pollStatus() {
		try {
			const status = await cloudSyncApi.getSyncStatus()
			update((s) => ({ ...s, status }))
		} catch {
			// Silent — status polling shouldn't surface errors
		}
	}

	/**
	 * Mobile foreground sync: one pull-then-push pass, then refresh the status indicator. Unlike
	 * desktop (which runs an always-on poll loop), mobile has no background loop, so this runs on
	 * launch and every time the app returns to the foreground. Failures are swallowed — a transient
	 * offline error already surfaces via the `Offline` phase, and this fires often enough that a
	 * toast would be intrusive.
	 */
	async function foregroundSync() {
		try {
			await cloudSyncApi.syncForeground()
		} catch {
			// Silent — offline/foreground failures reflect in the sync phase, not a toast.
		}
		await pollStatus()
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
		// Now signed in — arm opportunistic background sync (mobile only).
		armBackgroundSync()
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

		/**
		 * Native iOS Sign in with Apple (App Store Guideline 4.8). Single-step and fully native — the
		 * backend presents the AuthenticationServices sheet and does the Firebase exchange, so unlike
		 * `signInMobile` there's no `authenticate`/web-auth plugin to inject. Dismissing the sheet
		 * surfaces the backend's cancel sentinel, which is swallowed silently (no error toast).
		 */
		async signInApple() {
			update((s) => ({ ...s, signingIn: true, error: null }))
			try {
				const status = await cloudSyncApi.signInWithApple()
				applyStatusAfterSignIn(status)
			} catch (error) {
				const message = error instanceof Error ? error.message : String(error)
				update((s) => ({
					...s,
					signingIn: false,
					error: message.includes('sign-in canceled') ? null : describeError(error, 'Sign-in failed'),
				}))
			}
		},

		async signOut() {
			try {
				await cloudSyncApi.signOut()
				const status = await cloudSyncApi.getSyncStatus()
				update((s) => ({ ...s, status, devices: [], error: null }))
				disarmBackgroundSync()
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

		async deleteAccount() {
			update((s) => ({ ...s, deletingAccount: true }))
			try {
				await cloudSyncApi.deleteAccount()
				// The backend deletes the account then signs out; reflect the signed-out state.
				const status = await cloudSyncApi.getSyncStatus()
				update((s) => ({ ...s, status, devices: [], libraryRoots: [], error: null }))
				disarmBackgroundSync()
				// Deletion looks identical to a plain sign-out in the UI, so confirm it explicitly.
				toastStore.success(get(translate)('cloudSync.danger.deleteAccountSuccess'))
			} catch (error) {
				console.error('Failed to delete account:', error)
				toastStore.error(get(translate)('cloudSync.danger.deleteAccountError'))
			} finally {
				update((s) => ({ ...s, deletingAccount: false }))
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

		/** Run one foreground sync pass on demand (pull, then push if dirty). */
		syncForeground() {
			return foregroundSync()
		},

		/**
		 * Mobile-only: sync on launch and whenever the app returns to the foreground, in place of an
		 * always-on poll. Listens for `visibilitychange` (tab/app becomes visible) and window `focus`,
		 * and kicks an initial pass immediately. Desktop keeps using `startPolling` instead.
		 */
		startForegroundSync() {
			if (foregroundCleanup) return
			const onVisible = () => {
				if (document.visibilityState === 'visible') void foregroundSync()
			}
			const onFocus = () => void foregroundSync()
			document.addEventListener('visibilitychange', onVisible)
			window.addEventListener('focus', onFocus)
			foregroundCleanup = () => {
				document.removeEventListener('visibilitychange', onVisible)
				window.removeEventListener('focus', onFocus)
			}
			void foregroundSync()
			// Arm opportunistic background sync on launch too (no-ops in the backend when signed out).
			armBackgroundSync()
		},

		stopForegroundSync() {
			foregroundCleanup?.()
			foregroundCleanup = null
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

/** True while an account-deletion request is in flight — both platforms show a progress state. */
export const deletingAccount = derived(cloudSyncStore, ($s) => $s.deletingAccount)

export const cloudDevices = derived(cloudSyncStore, ($s) => $s.devices)

export const libraryRoots = derived(cloudSyncStore, ($s) => $s.libraryRoots)

export const unmappedRootIds = derived(
	cloudSyncStore,
	($s) => new Set($s.libraryRoots.filter((r) => !r.local_path).map((r) => r.id))
)

export const signingIn = derived(cloudSyncStore, ($s) => $s.signingIn)

export const cloudSyncError = derived(cloudSyncStore, ($s) => $s.error)
