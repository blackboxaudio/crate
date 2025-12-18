import { writable, derived } from 'svelte/store'
import type { Track, PlaybackState } from '$lib/types'
import * as playerApi from '$lib/api/player'

// =============================================================================
// State
// =============================================================================

interface PlayerState {
	currentTrack: Track | null
	playbackState: PlaybackState
	error: string | null
}

const initialPlaybackState: PlaybackState = {
	is_playing: false,
	position_ms: 0,
	duration_ms: 0,
	volume: 1.0,
	current_track_id: null,
	current_track_path: null,
}

const initialState: PlayerState = {
	currentTrack: null,
	playbackState: initialPlaybackState,
	error: null,
}

// =============================================================================
// Store
// =============================================================================

function createPlayerStore() {
	const { subscribe, set, update } = writable<PlayerState>(initialState)

	let positionInterval: ReturnType<typeof setInterval> | null = null

	function startPositionTracking() {
		stopPositionTracking()
		positionInterval = setInterval(() => {
			update((state) => {
				if (state.playbackState.is_playing) {
					return {
						...state,
						playbackState: {
							...state.playbackState,
							position_ms: Math.min(state.playbackState.position_ms + 100, state.playbackState.duration_ms),
						},
					}
				}
				return state
			})
		}, 100)
	}

	function stopPositionTracking() {
		if (positionInterval) {
			clearInterval(positionInterval)
			positionInterval = null
		}
	}

	return {
		subscribe,

		/**
		 * Play a track
		 */
		async play(track: Track) {
			try {
				const playbackState = await playerApi.playTrack(track.id)
				update((state) => ({
					...state,
					currentTrack: track,
					playbackState,
					error: null,
				}))
				startPositionTracking()
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to play track',
				}))
			}
		},

		/**
		 * Pause playback
		 */
		async pause() {
			try {
				const playbackState = await playerApi.pause()
				update((state) => ({
					...state,
					playbackState,
					error: null,
				}))
				stopPositionTracking()
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to pause',
				}))
			}
		},

		/**
		 * Resume playback
		 */
		async resume() {
			try {
				const playbackState = await playerApi.resume()
				update((state) => ({
					...state,
					playbackState,
					error: null,
				}))
				startPositionTracking()
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to resume',
				}))
			}
		},

		/**
		 * Stop playback
		 */
		async stop() {
			try {
				const playbackState = await playerApi.stop()
				update((state) => ({
					...state,
					currentTrack: null,
					playbackState,
					error: null,
				}))
				stopPositionTracking()
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to stop',
				}))
			}
		},

		/**
		 * Seek to position
		 */
		async seek(positionMs: number) {
			try {
				const playbackState = await playerApi.seek(positionMs)
				update((state) => ({
					...state,
					playbackState,
					error: null,
				}))
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to seek',
				}))
			}
		},

		/**
		 * Set volume
		 */
		async setVolume(volume: number) {
			try {
				const playbackState = await playerApi.setVolume(volume)
				update((state) => ({
					...state,
					playbackState,
					error: null,
				}))
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to set volume',
				}))
			}
		},

		/**
		 * Toggle play/pause
		 */
		async togglePlayPause() {
			let isPlaying = false
			const unsubscribe = subscribe((state) => {
				isPlaying = state.playbackState.is_playing
			})
			unsubscribe()

			if (isPlaying) {
				await this.pause()
			} else {
				await this.resume()
			}
		},

		/**
		 * Reset store to initial state
		 */
		reset() {
			stopPositionTracking()
			set(initialState)
		},
	}
}

export const playerStore = createPlayerStore()

// =============================================================================
// Derived Stores
// =============================================================================

export const isPlaying = derived(playerStore, ($player) => $player.playbackState.is_playing)

export const currentTrack = derived(playerStore, ($player) => $player.currentTrack)

export const playbackPosition = derived(playerStore, ($player) => $player.playbackState.position_ms)

export const playbackDuration = derived(playerStore, ($player) => $player.playbackState.duration_ms)

export const volume = derived(playerStore, ($player) => $player.playbackState.volume)

export const playbackProgress = derived(playerStore, ($player) => {
	const { position_ms, duration_ms } = $player.playbackState
	if (duration_ms === 0) return 0
	return (position_ms / duration_ms) * 100
})
