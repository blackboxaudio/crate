import { writable, derived } from 'svelte/store'
import type { Track, PlaybackState } from '$lib/types'
import * as playerApi from '$lib/api/player'
import { missingTracksStore } from './missingTracks'

// =============================================================================
// State
// =============================================================================

interface PlayerState {
	currentTrack: Track | null
	playbackState: PlaybackState
	error: string | null
	isMuted: boolean
	volumeBeforeMute: number
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
	isMuted: false,
	volumeBeforeMute: 1.0,
}

// =============================================================================
// Store
// =============================================================================

function createPlayerStore() {
	const { subscribe, set, update } = writable<PlayerState>(initialState)

	let positionInterval: ReturnType<typeof setInterval> | null = null
	let onTrackEndCallback: (() => void) | null = null

	function startPositionTracking() {
		stopPositionTracking()
		positionInterval = setInterval(() => {
			update((state) => {
				if (state.playbackState.is_playing) {
					const newPosition = Math.min(state.playbackState.position_ms + 100, state.playbackState.duration_ms)
					if (newPosition >= state.playbackState.duration_ms && state.playbackState.duration_ms > 0) {
						// Track ended — defer callback to avoid store update conflicts
						setTimeout(() => {
							stopPositionTracking()
							onTrackEndCallback?.()
						}, 0)
						return {
							...state,
							playbackState: {
								...state.playbackState,
								position_ms: state.playbackState.duration_ms,
								is_playing: false,
							},
						}
					}
					return {
						...state,
						playbackState: { ...state.playbackState, position_ms: newPosition },
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
				const errorMsg = error instanceof Error ? error.message : 'Failed to play track'

				// Check if this is a file-not-found error
				if (errorMsg.toLowerCase().includes('file not found') || errorMsg.toLowerCase().includes('filenotfound')) {
					missingTracksStore.markMissing(track.id)
				}

				update((state) => ({
					...state,
					error: errorMsg,
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
		 * Seek relative to current position
		 */
		async seekRelative(offsetMs: number) {
			let currentPosition = 0
			let duration = 0
			const unsubscribe = subscribe((state) => {
				currentPosition = state.playbackState.position_ms
				duration = state.playbackState.duration_ms
			})
			unsubscribe()

			// Clamp to valid range
			const newPosition = Math.max(0, Math.min(duration, currentPosition + offsetMs))
			await this.seek(newPosition)
		},

		/**
		 * Adjust volume by a relative amount
		 */
		async adjustVolume(delta: number) {
			let currentVolume = 1.0
			let isMuted = false
			const unsubscribe = subscribe((state) => {
				currentVolume = state.playbackState.volume
				isMuted = state.isMuted
			})
			unsubscribe()

			// If muted and trying to increase volume, unmute first
			if (isMuted && delta > 0) {
				update((state) => ({ ...state, isMuted: false }))
			}

			// Clamp to valid range (0.0 - 1.0)
			const newVolume = Math.max(0, Math.min(1, currentVolume + delta))
			await this.setVolume(newVolume)
		},

		/**
		 * Toggle mute/unmute
		 */
		async toggleMute() {
			let currentVolume = 1.0
			let isMuted = false
			let volumeBeforeMute = 1.0
			const unsubscribe = subscribe((state) => {
				currentVolume = state.playbackState.volume
				isMuted = state.isMuted
				volumeBeforeMute = state.volumeBeforeMute
			})
			unsubscribe()

			if (isMuted) {
				// Unmute: restore previous volume
				update((state) => ({ ...state, isMuted: false }))
				await this.setVolume(volumeBeforeMute)
			} else {
				// Mute: save current volume and set to 0
				update((state) => ({ ...state, isMuted: true, volumeBeforeMute: currentVolume }))
				await this.setVolume(0)
			}
		},

		/**
		 * Register a callback for when a track finishes playing
		 */
		onTrackEnd(callback: (() => void) | null) {
			onTrackEndCallback = callback
		},

		/**
		 * Reset store to initial state
		 */
		reset() {
			stopPositionTracking()
			onTrackEndCallback = null
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

export const isMuted = derived(playerStore, ($player) => $player.isMuted)
