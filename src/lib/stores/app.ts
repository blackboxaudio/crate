import { writable, derived } from 'svelte/store'
import type { AppInfo } from '$lib/api/app'
import * as appApi from '$lib/api/app'

// =============================================================================
// State
// =============================================================================

interface AppState {
	info: AppInfo | null
	loading: boolean
	error: string | null
}

const initialState: AppState = {
	info: null,
	loading: false,
	error: null,
}

// =============================================================================
// Store
// =============================================================================

function createAppStore() {
	const { subscribe, set, update } = writable<AppState>(initialState)

	return {
		subscribe,

		/**
		 * Load app info from backend
		 */
		async load() {
			update((s) => ({ ...s, loading: true, error: null }))

			try {
				const info = await appApi.getAppInfo()
				update((s) => ({ ...s, info, loading: false }))
			} catch (error) {
				update((s) => ({
					...s,
					loading: false,
					error: error instanceof Error ? error.message : 'Failed to load app info',
				}))
			}
		},

		/**
		 * Reset store to initial state
		 */
		reset() {
			set(initialState)
		},
	}
}

export const appStore = createAppStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const appInfo = derived(appStore, ($s) => $s.info)

export const isDev = derived(appStore, ($s) => $s.info?.isDev ?? false)

export const appVersion = derived(appStore, ($s) => $s.info?.version ?? '0.0.0')

export const appEnvironment = derived(appStore, ($s) => $s.info?.environment ?? 'unknown')

export const appDataDir = derived(appStore, ($s) => $s.info?.dataDir ?? '')

export const appLoading = derived(appStore, ($s) => $s.loading)
