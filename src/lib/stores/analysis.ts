import { writable, derived } from 'svelte/store'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { AnalysisResult, AnalysisStatus, TrackAnalysisEvent } from '$lib/types'
import * as analysisApi from '$lib/api/analysis'
import { libraryStore } from './library'

// =============================================================================
// State
// =============================================================================

interface AnalysisState {
	trackStates: Map<string, AnalysisStatus>
	isAnalyzing: boolean
	lastResults: AnalysisResult[]
	error: string | null
}

const initialState: AnalysisState = {
	trackStates: new Map(),
	isAnalyzing: false,
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
		 * Start listening for per-track analysis events
		 */
		async startListening() {
			if (unlisten) return

			unlisten = await listen<TrackAnalysisEvent>('analysis-track-event', (event) => {
				const { track_id, state, result, updated_track, error } = event.payload

				update((s) => {
					const newTrackStates = new Map(s.trackStates)

					// Remove from tracking on terminal states
					if (state === 'completed' || state === 'cancelled' || state === 'failed') {
						newTrackStates.delete(track_id)
					} else {
						// Update state for pending/analyzing
						newTrackStates.set(track_id, state)
					}

					// Update track in library store if we have an updated track
					if (updated_track) {
						libraryStore.updateTracksInState([updated_track])
					}

					return {
						...s,
						trackStates: newTrackStates,
						isAnalyzing: newTrackStates.size > 0,
						lastResults: result ? [...s.lastResults, result] : s.lastResults,
						error: error || s.error,
					}
				})
			})
		},

		/**
		 * Stop listening for analysis events
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
		async analyzeTracks(trackIds: string[]): Promise<void> {
			// Mark tracks as pending
			update((state) => ({
				...state,
				isAnalyzing: true,
				trackStates: new Map(trackIds.map((id) => [id, 'pending' as AnalysisStatus])),
				error: null,
				lastResults: [],
			}))

			try {
				// Start listening if not already
				await this.startListening()

				// Call backend - returns immediately, results come via events
				await analysisApi.analyzeTracks(trackIds)
			} catch (error) {
				const errorMessage = error instanceof Error ? error.message : 'Analysis failed'

				update((state) => ({
					...state,
					isAnalyzing: false,
					trackStates: new Map(),
					error: errorMessage,
				}))

				throw error
			}
		},

		/**
		 * Cancel analysis for a specific track
		 */
		async cancelTrackAnalysis(trackId: string): Promise<boolean> {
			const cancelled = await analysisApi.cancelTrackAnalysis(trackId)

			// Optimistically remove from state if backend confirms cancellation
			// The event will also update the state, but this provides immediate feedback
			if (cancelled) {
				update((state) => {
					const newTrackStates = new Map(state.trackStates)
					newTrackStates.delete(trackId)
					return {
						...state,
						trackStates: newTrackStates,
						isAnalyzing: newTrackStates.size > 0,
					}
				})
			}

			return cancelled
		},

		/**
		 * Cancel all running analysis (legacy)
		 */
		async cancelAnalysis(): Promise<void> {
			await analysisApi.cancelAnalysis()
		},

		/**
		 * Check if a specific track is being analyzed
		 */
		isTrackAnalyzing(trackId: string): boolean {
			let result = false
			const unsubscribe = subscribe((state) => {
				result = state.trackStates.has(trackId)
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

export const analyzingTrackIds = derived(analysisStore, ($store) => new Set($store.trackStates.keys()))

export const isAnalyzing = derived(analysisStore, ($store) => $store.isAnalyzing)

export const analysisError = derived(analysisStore, ($store) => $store.error)
