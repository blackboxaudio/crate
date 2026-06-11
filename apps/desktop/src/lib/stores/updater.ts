import { writable, derived, get } from 'svelte/store'
import type { Update } from '@tauri-apps/plugin-updater'
import { checkForUpdate, relaunch } from '$shared/api/updater'
import { appVersion, isDev } from '$lib/stores/app'
import { toastStore } from '$shared/stores/toast'
import { translate } from '$shared/i18n'

// =============================================================================
// Types
// =============================================================================

type UpdaterStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'installing' | 'upToDate' | 'error'

interface UpdaterState {
	status: UpdaterStatus
	update: Update | null
	version: string | null
	body: string | null
	progress: number
	error: string | null
	dismissed: boolean
}

// =============================================================================
// State
// =============================================================================

const initialState: UpdaterState = {
	status: 'idle',
	update: null,
	version: null,
	body: null,
	progress: 0,
	error: null,
	dismissed: false,
}

// =============================================================================
// Store
// =============================================================================

function createUpdaterStore() {
	const { subscribe, set, update } = writable<UpdaterState>(initialState)

	return {
		subscribe,

		async check(silent = false) {
			// Block automatic (silent) checks in dev — manual clicks still work
			if (get(isDev) && silent) return

			const current = get({ subscribe })
			if (current.status === 'checking') return

			update((s) => ({ ...s, status: 'checking', error: null }))

			try {
				const result = await checkForUpdate()

				if (result) {
					const currentVersion = get(appVersion)
					if (result.version === currentVersion) {
						update((s) => ({ ...s, status: 'upToDate', update: null }))
						return
					}

					const newVersion = result.version
					update((s) => ({
						...s,
						status: 'available',
						update: result,
						version: newVersion,
						body: result.body ?? null,
						// Reset dismissed if a different version is now available
						dismissed: s.version === newVersion ? s.dismissed : false,
					}))
				} else {
					update((s) => ({ ...s, status: 'upToDate', update: null }))
				}
			} catch (error) {
				const message = error instanceof Error ? error.message : String(error)
				update((s) => ({ ...s, status: 'error', error: message }))
				if (!silent) {
					toastStore.error(get(translate)('errors.updateCheckFailed'))
				}
			}
		},

		async install() {
			const current = get({ subscribe })
			if (!current.update) return

			update((s) => ({ ...s, status: 'downloading', progress: 0, error: null }))

			try {
				let contentLength = 0
				let downloaded = 0

				await current.update.downloadAndInstall((event) => {
					switch (event.event) {
						case 'Started':
							contentLength = event.data.contentLength ?? 0
							break
						case 'Progress':
							downloaded += event.data.chunkLength
							if (contentLength > 0) {
								const progress = Math.round((downloaded / contentLength) * 100)
								update((s) => ({ ...s, progress }))
							}
							break
						case 'Finished':
							update((s) => ({ ...s, status: 'installing', progress: 100 }))
							break
					}
				})

				await relaunch()
			} catch (error) {
				const message = error instanceof Error ? error.message : String(error)
				update((s) => ({ ...s, status: 'error', error: message }))
				toastStore.error(get(translate)('errors.updateInstallFailed'))
			}
		},

		dismiss() {
			update((s) => ({ ...s, dismissed: true }))
		},

		reset() {
			set(initialState)
		},
	}
}

export const updaterStore = createUpdaterStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const updateStatus = derived(updaterStore, ($s) => $s.status)

export const updateAvailable = derived(updaterStore, ($s) => $s.status === 'available' && !$s.dismissed)
