import { writable, derived } from 'svelte/store'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { BackupProgress } from '$shared/types'

// =============================================================================
// Types
// =============================================================================

interface BackupState {
	isCreating: boolean
	isRestoring: boolean
	progress: BackupProgress | null
	error: string | null
}

// =============================================================================
// State
// =============================================================================

const initialState: BackupState = {
	isCreating: false,
	isRestoring: false,
	progress: null,
	error: null,
}

// =============================================================================
// Store
// =============================================================================

function createBackupStore() {
	const { subscribe, set, update } = writable<BackupState>(initialState)

	let unlisten: UnlistenFn | null = null

	return {
		subscribe,

		async startListening() {
			if (unlisten) return

			unlisten = await listen<BackupProgress>('backup-progress', (event) => {
				update((state) => {
					const isDone = event.payload.status === 'completed'
					return {
						...state,
						progress: event.payload,
						isCreating: isDone ? false : state.isCreating,
						isRestoring: isDone ? false : state.isRestoring,
					}
				})
			})
		},

		stopListening() {
			if (unlisten) {
				unlisten()
				unlisten = null
			}
		},

		startBackup() {
			update((state) => ({
				...state,
				isCreating: true,
				error: null,
				progress: null,
			}))
		},

		startRestore() {
			update((state) => ({
				...state,
				isRestoring: true,
				error: null,
				progress: null,
			}))
		},

		complete() {
			update((state) => ({
				...state,
				isCreating: false,
				isRestoring: false,
			}))
		},

		fail(error: string) {
			update((state) => ({
				...state,
				isCreating: false,
				isRestoring: false,
				error,
			}))
		},

		reset() {
			set(initialState)
		},
	}
}

export const backupStore = createBackupStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const isBackupBusy = derived(backupStore, ($store) => $store.isCreating || $store.isRestoring)

export const backupProgress = derived(backupStore, ($store) => $store.progress)

export const backupError = derived(backupStore, ($store) => $store.error)
