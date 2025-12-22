import { writable, derived } from 'svelte/store'
import type { AnalysisResult } from '$lib/types'
import * as analysisApi from '$lib/api/analysis'
import { libraryStore } from './library'

// =============================================================================
// State
// =============================================================================

interface AnalysisState {
	analyzingTrackIds: Set<string>
	lastResults: AnalysisResult[] | null
	error: string | null
}

const initialState: AnalysisState = {
	analyzingTrackIds: new Set(),
	lastResults: null,
	error: null,
}

// =============================================================================
// Store
// =============================================================================

function createAnalysisStore() {
	const { subscribe, set, update } = writable<AnalysisState>(initialState)

	return {
		subscribe,

		/**
		 * Analyze tracks for BPM and key detection
		 */
		async analyzeTracks(trackIds: string[]): Promise<AnalysisResult[]> {
			// Mark tracks as being analyzed
			update((state) => ({
				...state,
				analyzingTrackIds: new Set(trackIds),
				error: null,
				lastResults: null,
			}))

			try {
				const results = await analysisApi.analyzeTracks(trackIds)

				// Get the updated tracks from the backend
				const successIds = results.filter((r) => r.success).map((r) => r.track_id)

				if (successIds.length > 0) {
					const updatedTracks = await analysisApi.getAnalyzedTracks(successIds)
					// Update the library store with the new track data
					libraryStore.updateTracksInState(updatedTracks)
				}

				update((state) => ({
					...state,
					analyzingTrackIds: new Set(),
					lastResults: results,
				}))

				return results
			} catch (error) {
				const errorMessage = error instanceof Error ? error.message : 'Analysis failed'

				update((state) => ({
					...state,
					analyzingTrackIds: new Set(),
					error: errorMessage,
				}))

				throw error
			}
		},

		/**
		 * Check if a specific track is being analyzed
		 */
		isTrackAnalyzing(trackId: string): boolean {
			let result = false
			const unsubscribe = subscribe((state) => {
				result = state.analyzingTrackIds.has(trackId)
			})
			unsubscribe()
			return result
		},

		/**
		 * Reset to initial state
		 */
		reset() {
			set(initialState)
		},
	}
}

export const analysisStore = createAnalysisStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const analyzingTrackIds = derived(analysisStore, ($store) => $store.analyzingTrackIds)

export const isAnalyzing = derived(analysisStore, ($store) => $store.analyzingTrackIds.size > 0)
