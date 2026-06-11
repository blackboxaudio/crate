import { writable, derived } from 'svelte/store'
import * as libraryApi from '$shared/api/library'

// =============================================================================
// State
// =============================================================================

interface MissingTracksState {
	/** Set of track IDs with missing files */
	missingIds: Set<string>
	/** Set of track IDs currently being checked */
	checkingIds: Set<string>
}

const initialState: MissingTracksState = {
	missingIds: new Set(),
	checkingIds: new Set(),
}

// =============================================================================
// Store
// =============================================================================

function createMissingTracksStore() {
	const { subscribe, update } = writable<MissingTracksState>(initialState)

	return {
		subscribe,

		/**
		 * Check if a track's file exists and update the missing status
		 */
		async checkTrack(trackId: string): Promise<boolean> {
			// Mark as checking
			update((state) => ({
				...state,
				checkingIds: new Set([...state.checkingIds, trackId]),
			}))

			try {
				const exists = await libraryApi.checkFileExists(trackId)

				update((state) => {
					const newMissingIds = new Set(state.missingIds)
					if (exists) {
						newMissingIds.delete(trackId)
					} else {
						newMissingIds.add(trackId)
					}

					const newCheckingIds = new Set(state.checkingIds)
					newCheckingIds.delete(trackId)

					return {
						missingIds: newMissingIds,
						checkingIds: newCheckingIds,
					}
				})

				return exists
			} catch {
				// On error, assume file is missing
				update((state) => {
					const newCheckingIds = new Set(state.checkingIds)
					newCheckingIds.delete(trackId)

					return {
						...state,
						missingIds: new Set([...state.missingIds, trackId]),
						checkingIds: newCheckingIds,
					}
				})

				return false
			}
		},

		/**
		 * Mark a track as missing (e.g., after playback error)
		 */
		markMissing(trackId: string): void {
			update((state) => ({
				...state,
				missingIds: new Set([...state.missingIds, trackId]),
			}))
		},

		/**
		 * Mark a track as found (e.g., after successful relocation)
		 */
		markFound(trackId: string): void {
			update((state) => {
				const newMissingIds = new Set(state.missingIds)
				newMissingIds.delete(trackId)
				return {
					...state,
					missingIds: newMissingIds,
				}
			})
		},

		/**
		 * Check if a track is currently being checked
		 */
		isChecking(trackId: string): boolean {
			let checking = false
			const unsubscribe = subscribe((state) => {
				checking = state.checkingIds.has(trackId)
			})
			unsubscribe()
			return checking
		},

		/**
		 * Clear all missing track data
		 */
		reset(): void {
			update(() => initialState)
		},
	}
}

export const missingTracksStore = createMissingTracksStore()

// =============================================================================
// Derived Stores
// =============================================================================

/** Set of track IDs with missing files */
export const missingTrackIds = derived(missingTracksStore, ($store) => $store.missingIds)

/** Set of track IDs currently being checked */
export const checkingTrackIds = derived(missingTracksStore, ($store) => $store.checkingIds)
