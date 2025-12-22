import { writable, derived } from 'svelte/store'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { AnalysisResult, AnalysisProgress } from '$lib/types'
import * as analysisApi from '$lib/api/analysis'
import { libraryStore } from './library'

// =============================================================================
// State
// =============================================================================

interface AnalysisState {
	isAnalyzing: boolean
	progress: AnalysisProgress | null
	analyzingTrackIds: Set<string>
	completedTrackIds: Set<string>
	lastResults: AnalysisResult[]
	error: string | null
}

const initialState: AnalysisState = {
	isAnalyzing: false,
	progress: null,
	analyzingTrackIds: new Set(),
	completedTrackIds: new Set(),
	lastResults: [],
	error: null,
}

// =============================================================================
// Store
// =============================================================================

function createAnalysisStore() {
	const { subscribe, set, update } = writable<AnalysisState>(initialState)

	let unlisten: UnlistenFn | null = null

	return {
		subscribe,

		/**
		 * Start listening for analysis progress events
		 */
		async startListening() {
			if (unlisten) return

			unlisten = await listen<AnalysisProgress>('analysis-progress', (event) => {
				const progress = event.payload

				update((state) => {
					const newState = { ...state, progress }

					// Update track in library store if we have an updated track
					if (progress.updated_track) {
						libraryStore.updateTracksInState([progress.updated_track])
					}

					// Track completed results
					if (progress.result) {
						newState.lastResults = [...state.lastResults, progress.result]
						if (progress.result.success && progress.current_track_id) {
							newState.completedTrackIds = new Set([...state.completedTrackIds, progress.current_track_id])
						}
					}

					// Update analyzing state based on status
					if (progress.status === 'completed' || progress.status === 'failed' || progress.status === 'cancelled') {
						newState.isAnalyzing = false
						newState.analyzingTrackIds = new Set()
					}

					return newState
				})
			})
		},

		/**
		 * Stop listening for analysis progress events
		 */
		stopListening() {
			if (unlisten) {
				unlisten()
				unlisten = null
			}
		},

		/**
		 * Analyze tracks for BPM and key detection
		 */
		async analyzeTracks(trackIds: string[]): Promise<AnalysisResult[]> {
			// Mark tracks as being analyzed
			update((state) => ({
				...state,
				isAnalyzing: true,
				analyzingTrackIds: new Set(trackIds),
				completedTrackIds: new Set(),
				error: null,
				lastResults: [],
				progress: null,
			}))

			try {
				// Start listening if not already
				await this.startListening()

				// Call backend - results stream via events
				const results = await analysisApi.analyzeTracks(trackIds)

				update((state) => ({
					...state,
					isAnalyzing: false,
					analyzingTrackIds: new Set(),
				}))

				return results
			} catch (error) {
				const errorMessage = error instanceof Error ? error.message : 'Analysis failed'

				update((state) => ({
					...state,
					isAnalyzing: false,
					analyzingTrackIds: new Set(),
					error: errorMessage,
				}))

				throw error
			}
		},

		/**
		 * Cancel the current analysis
		 */
		async cancelAnalysis(): Promise<void> {
			await analysisApi.cancelAnalysis()
		},

		/**
		 * Check if a specific track is being analyzed (not yet completed)
		 */
		isTrackAnalyzing(trackId: string): boolean {
			let result = false
			const unsubscribe = subscribe((state) => {
				result = state.analyzingTrackIds.has(trackId) && !state.completedTrackIds.has(trackId)
			})
			unsubscribe()
			return result
		},

		/**
		 * Check if a specific track has completed analysis
		 */
		isTrackCompleted(trackId: string): boolean {
			let result = false
			const unsubscribe = subscribe((state) => {
				result = state.completedTrackIds.has(trackId)
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

export const completedTrackIds = derived(analysisStore, ($store) => $store.completedTrackIds)

export const isAnalyzing = derived(analysisStore, ($store) => $store.isAnalyzing)

export const analysisProgress = derived(analysisStore, ($store) => $store.progress)

export const analysisProgressPercent = derived(analysisStore, ($store) => {
	if (!$store.progress || $store.progress.tracks_total === 0) return 0
	return Math.round(($store.progress.tracks_analyzed / $store.progress.tracks_total) * 100)
})
