import { writable, derived, get } from 'svelte/store'
import type { Track, PlaybackState, PreviewInfo, DiscoveryRelease } from '../types'
import * as playerApi from '../api/player'
import * as discoveryApi from '../api/discovery'
import * as previewPlayer from '../services/previewPlayer'
import * as nativePreviewPlayer from '../services/nativePreviewPlayer'
import { isIOS } from '../utils/platform'
import { toastStore } from './toast'
import { translate } from '../i18n'
import {
	getStoredNumber,
	setStoredNumber,
	getStoredString,
	setStoredString,
	getStoredBoolean,
	setStoredBoolean,
} from '../utils/storage'

// =============================================================================
// State
// =============================================================================

type PlaybackSource = 'library' | 'preview'

interface PlayerState {
	currentTrack: Track | null
	playbackState: PlaybackState
	error: string | null
	isMuted: boolean
	volumeBeforeMute: number
	shuffleEnabled: boolean
	playbackSource: PlaybackSource
	previewInfo: PreviewInfo | null
	previewTrackIndex: number
	previewLoading: { releaseId: string; trackIndex: number } | null
}

const initialPlaybackState: PlaybackState = {
	is_playing: false,
	position_ms: getStoredNumber('player.positionMs', 0),
	duration_ms: getStoredNumber('player.durationMs', 0),
	volume: getStoredNumber('player.volume', 1.0),
	speed: getStoredNumber('player.speed', 1.0),
	current_track_id: null,
	current_track_path: null,
}

const restoredTrackId = getStoredString('player.trackId', '')
const restoredPlaybackSource = getStoredString<PlaybackSource>('player.playbackSource', 'library', [
	'library',
	'preview',
])
const restoredPreviewReleaseId = getStoredString('player.previewReleaseId', '')
const restoredPreviewTrackIndex = getStoredNumber('player.previewTrackIndex', 0)

const initialState: PlayerState = {
	currentTrack: null,
	playbackState: initialPlaybackState,
	error: null,
	isMuted: getStoredBoolean('player.isMuted', false),
	volumeBeforeMute: getStoredNumber('player.volumeBeforeMute', 1.0),
	shuffleEnabled: getStoredBoolean('player.shuffleEnabled', false),
	playbackSource: 'library',
	previewInfo: null,
	previewTrackIndex: 0,
	previewLoading: null,
}

// Pressing "previous" within this window restarts the current track instead of jumping to the
// previous one (matches iOS Music / most players). Shared by the in-app transport controls and the
// OS media-session handlers (webMediaSession.ts) so both honour the same rule.
export const PREVIOUS_RESTART_THRESHOLD_MS = 3000

// =============================================================================
// Store
// =============================================================================

function createPlayerStore() {
	const { subscribe, set, update } = writable<PlayerState>(initialState)

	let positionInterval: ReturnType<typeof setInterval> | null = null
	let onTrackEndCallback: (() => void) | null = null
	// Desktop registers this to flag a track as missing when playback fails with a
	// file-not-found error. Injected so this shared store needs no dependency on the
	// desktop-only missingTracks store.
	let onTrackMissing: ((trackId: string) => void) | null = null
	let previewRetryAttempted = false
	let previewRetrying = false
	let previewSpeedCommitTimeout: ReturnType<typeof setTimeout> | null = null
	let isRestoredFromStorage = false
	let lastPositionWriteTime = 0
	// On iOS, discovery preview plays through the native AVPlayer engine (lock-screen transport that
	// survives JS suspension) instead of the HTML5 <audio> element. Every preview transport method
	// branches on this; desktop/Android keep the HTML5 path.
	const useNative = isIOS()

	function persistPosition(positionMs: number) {
		const now = Date.now()
		if (now - lastPositionWriteTime >= 1000) {
			setStoredNumber('player.positionMs', positionMs)
			lastPositionWriteTime = now
		}
	}

	function persistPositionImmediate(positionMs: number) {
		setStoredNumber('player.positionMs', positionMs)
		lastPositionWriteTime = Date.now()
	}

	function getState(): PlayerState {
		let state: PlayerState = initialState
		const unsub = subscribe((s) => (state = s))
		unsub()
		return state
	}

	function startPositionTracking() {
		stopPositionTracking()
		positionInterval = setInterval(() => {
			update((state) => {
				if (state.playbackState.is_playing) {
					const speed = state.playbackState.speed ?? 1.0
					const newPosition = Math.min(
						state.playbackState.position_ms + Math.round(100 * speed),
						state.playbackState.duration_ms
					)
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
					if (state.playbackSource === 'library') {
						persistPosition(newPosition)
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

	function stopPreviewInternal() {
		previewPlayer.stop()
		stopPositionTracking()
	}

	function wirePreviewEvents() {
		previewPlayer.setOnTimeUpdate((positionMs: number) => {
			update((state) => {
				const { duration_ms } = state.playbackState
				// When the Audio element reports a position past the metadata duration,
				// the stream container is longer than the actual audio (e.g. proxied
				// YouTube/Discogs ~2x duration). Stop playback and trigger track end.
				if (duration_ms > 0 && positionMs >= duration_ms) {
					setTimeout(() => {
						stopPreviewInternal()
						onTrackEndCallback?.()
					}, 0)
					return {
						...state,
						playbackState: { ...state.playbackState, position_ms: duration_ms, is_playing: false },
					}
				}
				return {
					...state,
					playbackState: { ...state.playbackState, position_ms: positionMs },
				}
			})
		})
		previewPlayer.setOnDurationChange((durationMs: number) => {
			update((state) => {
				const metadataDuration = state.playbackState.duration_ms
				// When we have a metadata duration from the API, only accept the Audio
				// element's duration if it's within 10% of the known value. Proxied
				// YouTube/Discogs streams can report ~2x the real duration due to
				// container quirks; rejecting those prevents overwriting the correct value.
				if (metadataDuration > 0) {
					const ratio = durationMs / metadataDuration
					if (ratio < 0.9 || ratio > 1.1) {
						return state
					}
				}
				return {
					...state,
					playbackState: { ...state.playbackState, duration_ms: durationMs },
				}
			})
		})
		previewPlayer.setOnEnded(() => {
			update((state) => ({
				...state,
				playbackState: { ...state.playbackState, is_playing: false },
			}))
			onTrackEndCallback?.()
		})
		previewPlayer.setOnWaiting(() => {
			const state = getState()
			if (state.playbackSource === 'preview' && state.previewInfo) {
				update((s) => ({
					...s,
					previewLoading: { releaseId: state.previewInfo!.releaseId, trackIndex: state.previewInfo!.trackIndex },
				}))
			}
		})
		previewPlayer.setOnPlaying(() => {
			if (previewSpeedCommitTimeout) {
				clearTimeout(previewSpeedCommitTimeout)
				previewSpeedCommitTimeout = null
			}
			update((s) => ({ ...s, previewLoading: null }))
		})
		previewPlayer.setOnError(async (msg: string) => {
			// Ignore duplicate error callbacks fired while a retry is in-flight
			// (HTML5 Audio fires both an 'error' event and a play().catch() for one failure)
			if (previewRetrying) return

			const state = getState()
			if (state.playbackSource === 'preview' && state.previewInfo && !previewRetryAttempted) {
				previewRetryAttempted = true
				previewRetrying = true
				const { release, trackIndex } = state.previewInfo
				const track = release.tracks[trackIndex]
				if (track) {
					console.warn(`Preview stream error, retrying: ${msg}`)
					try {
						await discoveryApi.invalidatePreviewStreamCache(release.id)
						const streamUrl = await discoveryApi.fetchPreviewStream(release.id, track.position)
						previewPlayer.play(streamUrl)
						update((s) => ({
							...s,
							error: null,
							playbackState: { ...s.playbackState, is_playing: true, position_ms: 0 },
						}))
						return
					} catch {
						// Retry failed, fall through to show error
					} finally {
						previewRetrying = false
					}
				} else {
					previewRetrying = false
				}
			}
			clearPreviewEvents()
			stopPreviewInternal()
			update((s) => ({
				...s,
				error: msg,
				playbackState: { ...s.playbackState, is_playing: false },
			}))
			toastStore.error(get(translate)('errors.previewStreamFailed'))
		})
	}

	function clearPreviewEvents() {
		previewPlayer.setOnTimeUpdate(null)
		previewPlayer.setOnDurationChange(null)
		previewPlayer.setOnEnded(null)
		previewPlayer.setOnError(null)
		previewPlayer.setOnWaiting(null)
		previewPlayer.setOnPlaying(null)
	}

	return {
		subscribe,

		/**
		 * Play a library track. If preview is active, stop it first.
		 */
		async play(track: Track) {
			const state = getState()

			// Stop preview if active
			if (state.playbackSource === 'preview') {
				stopPreviewInternal()
				clearPreviewEvents()
				// Sync speed to backend since preview speed changes are frontend-only
				try {
					await playerApi.setSpeed(state.playbackState.speed)
				} catch {
					// Best effort
				}
			}

			try {
				const playbackState = await playerApi.playTrack(track.id)
				isRestoredFromStorage = false
				setStoredString('player.playbackSource', 'library')
				setStoredString('player.trackId', track.id)
				setStoredString('player.previewReleaseId', '')
				setStoredNumber('player.durationMs', playbackState.duration_ms)
				persistPositionImmediate(0)
				update((s) => ({
					...s,
					currentTrack: track,
					playbackState,
					error: null,
					playbackSource: 'library',
					previewInfo: null,
					previewTrackIndex: 0,
				}))
				startPositionTracking()
			} catch (error) {
				const errorMsg = error instanceof Error ? error.message : 'Failed to play track'
				if (errorMsg.toLowerCase().includes('file not found') || errorMsg.toLowerCase().includes('filenotfound')) {
					onTrackMissing?.(track.id)
				}
				update((s) => ({ ...s, error: errorMsg }))
			}
		},

		/**
		 * Play a preview of a discovery release track.
		 */
		async playPreview(release: DiscoveryRelease, trackIndex: number = 0) {
			previewRetryAttempted = false
			const state = getState()
			const track = release.tracks[trackIndex]
			if (!track) return

			// Clear stale preview events before the async gap to prevent the old
			// error handler from firing when audio.src='' triggers an error event
			clearPreviewEvents()

			// Stop library audio if playing
			if (state.playbackSource === 'library' && state.playbackState.is_playing) {
				try {
					await playerApi.stop()
				} catch {
					// Best effort
				}
			}

			stopPositionTracking()
			update((s) => ({ ...s, previewLoading: { releaseId: release.id, trackIndex } }))

			// iOS: hand the whole release (pre-resolved proxy URLs) to the native engine so it can
			// switch tracks — including while the screen is locked — without any further JS.
			if (useNative) {
				try {
					const urls = await Promise.all(
						release.tracks.map((t) => discoveryApi.fetchPreviewStream(release.id, t.position))
					)
					const nativeTracks = release.tracks.map((t, i) => ({
						url: urls[i],
						title: t.name,
						artist: release.artist ?? '',
						album: release.title ?? '',
						durationMs: t.duration_ms ?? 0,
						artworkUrl: release.artwork_url ?? null,
					}))
					await nativePreviewPlayer.play(nativeTracks, trackIndex)
					await nativePreviewPlayer.setVolume(state.isMuted ? 0 : state.playbackState.volume)
					isRestoredFromStorage = false
					setStoredString('player.playbackSource', 'preview')
					setStoredString('player.previewReleaseId', release.id)
					setStoredNumber('player.previewTrackIndex', trackIndex)
					setStoredNumber('player.durationMs', track.duration_ms || 0)
					persistPositionImmediate(0)
					update((s) => ({
						...s,
						currentTrack: null,
						playbackState: {
							...s.playbackState,
							is_playing: true,
							position_ms: 0,
							duration_ms: track.duration_ms || 0,
							current_track_id: null,
							current_track_path: null,
						},
						error: null,
						playbackSource: 'preview',
						previewInfo: { releaseId: release.id, release, trackIndex },
						previewTrackIndex: trackIndex,
						previewLoading: null,
					}))
				} catch (error) {
					const errorMsg = error instanceof Error ? error.message : 'Failed to fetch preview stream'
					update((s) => ({ ...s, error: errorMsg, previewLoading: null }))
					toastStore.error(get(translate)('errors.previewStreamFailed'))
				}
				return
			}

			try {
				const streamUrl = await discoveryApi.fetchPreviewStream(release.id, track.position)

				wirePreviewEvents()

				// Sync volume and speed for preview player
				previewPlayer.play(streamUrl)
				const currentVolume = state.isMuted ? 0 : state.playbackState.volume
				previewPlayer.setVolume(currentVolume)
				previewPlayer.setPlaybackRate(state.playbackState.speed)

				isRestoredFromStorage = false
				setStoredString('player.playbackSource', 'preview')
				setStoredString('player.previewReleaseId', release.id)
				setStoredNumber('player.previewTrackIndex', trackIndex)
				setStoredNumber('player.durationMs', track.duration_ms || 0)
				persistPositionImmediate(0)
				update((s) => ({
					...s,
					currentTrack: null,
					playbackState: {
						...s.playbackState,
						is_playing: true,
						position_ms: 0,
						duration_ms: track.duration_ms || 0,
						current_track_id: null,
						current_track_path: null,
					},
					error: null,
					playbackSource: 'preview',
					previewInfo: { releaseId: release.id, release, trackIndex },
					previewTrackIndex: trackIndex,
					previewLoading: null,
				}))
			} catch (error) {
				const errorMsg = error instanceof Error ? error.message : 'Failed to fetch preview stream'
				update((s) => ({ ...s, error: errorMsg, previewLoading: null }))
				toastStore.error(get(translate)('errors.previewStreamFailed'))
			}
		},

		/**
		 * Pause playback (source-aware)
		 */
		async pause() {
			const state = getState()

			if (state.playbackSource === 'preview') {
				if (useNative) {
					void nativePreviewPlayer.pause()
				} else {
					previewPlayer.pause()
				}
				persistPositionImmediate(state.playbackState.position_ms)
				update((s) => ({
					...s,
					playbackState: { ...s.playbackState, is_playing: false },
					error: null,
				}))
				return
			}

			try {
				const playbackState = await playerApi.pause()
				persistPositionImmediate(playbackState.position_ms)
				update((s) => ({ ...s, playbackState, error: null }))
				stopPositionTracking()
			} catch (error) {
				update((s) => ({
					...s,
					error: error instanceof Error ? error.message : 'Failed to pause',
				}))
			}
		},

		/**
		 * Resume playback (source-aware)
		 */
		async resume() {
			const state = getState()

			if (state.playbackSource === 'preview') {
				if (useNative) {
					// Restored from storage (app relaunch): the native engine lost its in-memory playlist,
					// so re-issue playPreview to rebuild it; otherwise just resume.
					if (isRestoredFromStorage && state.previewInfo) {
						isRestoredFromStorage = false
						await this.playPreview(state.previewInfo.release, state.previewInfo.trackIndex)
						return
					}
					void nativePreviewPlayer.resume()
					update((s) => ({
						...s,
						playbackState: { ...s.playbackState, is_playing: true },
						error: null,
					}))
					return
				}
				// If restored from storage, the audio element has no source — load the stream
				if (isRestoredFromStorage && state.previewInfo) {
					isRestoredFromStorage = false
					const { release, trackIndex } = state.previewInfo
					const track = release.tracks[trackIndex]
					if (!track) return
					const restoredPosition = state.playbackState.position_ms
					try {
						const streamUrl = await discoveryApi.fetchPreviewStream(release.id, track.position)
						wirePreviewEvents()
						previewPlayer.play(streamUrl)
						const currentVolume = state.isMuted ? 0 : state.playbackState.volume
						previewPlayer.setVolume(currentVolume)
						previewPlayer.setPlaybackRate(state.playbackState.speed)
						if (restoredPosition > 0) {
							previewPlayer.seek(restoredPosition)
						}
						update((s) => ({
							...s,
							playbackState: { ...s.playbackState, is_playing: true },
							error: null,
						}))
					} catch {
						update((s) => ({
							...s,
							error: 'Failed to load preview stream',
							playbackState: { ...s.playbackState, is_playing: false },
						}))
					}
					return
				}
				// Sync playback rate in case speed was changed while paused
				previewPlayer.setPlaybackRate(state.playbackState.speed)
				previewPlayer.resume()
				update((s) => ({
					...s,
					playbackState: { ...s.playbackState, is_playing: true },
					error: null,
				}))
				return
			}

			// If restored from storage, the backend has no player loaded — load the track fully
			if (isRestoredFromStorage && state.currentTrack) {
				isRestoredFromStorage = false
				const restoredPosition = state.playbackState.position_ms
				try {
					// Sync volume and speed to backend before playing so create_player uses them
					await playerApi.setVolume(state.isMuted ? 0 : state.playbackState.volume)
					await playerApi.setSpeed(state.playbackState.speed)
					const playbackState = await playerApi.playTrack(state.currentTrack.id)
					// Seek to restored position
					if (restoredPosition > 0) {
						const seekedState = await playerApi.seek(restoredPosition)
						update((s) => ({ ...s, playbackState: seekedState, error: null }))
					} else {
						update((s) => ({ ...s, playbackState, error: null }))
					}
					startPositionTracking()
				} catch (error) {
					const errorMsg = error instanceof Error ? error.message : 'Failed to play track'
					if (errorMsg.toLowerCase().includes('file not found') || errorMsg.toLowerCase().includes('filenotfound')) {
						onTrackMissing?.(state.currentTrack.id)
					}
					update((s) => ({ ...s, error: errorMsg }))
				}
				return
			}

			try {
				const playbackState = await playerApi.resume()
				update((s) => ({ ...s, playbackState, error: null }))
				startPositionTracking()
			} catch (error) {
				update((s) => ({
					...s,
					error: error instanceof Error ? error.message : 'Failed to resume',
				}))
			}
		},

		/**
		 * Stop playback (source-aware). Preview mode resets to library source.
		 */
		async stop() {
			const state = getState()
			previewRetryAttempted = false

			if (state.playbackSource === 'preview') {
				if (useNative) {
					void nativePreviewPlayer.stop()
				} else {
					stopPreviewInternal()
					clearPreviewEvents()
				}
				isRestoredFromStorage = false
				setStoredString('player.playbackSource', 'library')
				setStoredString('player.previewReleaseId', '')
				setStoredNumber('player.positionMs', 0)
				setStoredNumber('player.durationMs', 0)
				update((s) => ({
					...s,
					currentTrack: null,
					playbackState: { ...initialPlaybackState, volume: s.playbackState.volume, speed: s.playbackState.speed },
					error: null,
					playbackSource: 'library',
					previewInfo: null,
					previewTrackIndex: 0,
				}))
				return
			}

			try {
				const playbackState = await playerApi.stop()
				isRestoredFromStorage = false
				setStoredString('player.playbackSource', 'library')
				setStoredString('player.trackId', '')
				setStoredNumber('player.positionMs', 0)
				setStoredNumber('player.durationMs', 0)
				update((s) => ({
					...s,
					currentTrack: null,
					playbackState,
					error: null,
				}))
				stopPositionTracking()
			} catch (error) {
				update((s) => ({
					...s,
					error: error instanceof Error ? error.message : 'Failed to stop',
				}))
			}
		},

		/**
		 * Seek to position (source-aware)
		 */
		async seek(positionMs: number) {
			const state = getState()

			if (state.playbackSource === 'preview') {
				if (useNative) {
					void nativePreviewPlayer.seek(positionMs)
				} else {
					previewPlayer.seek(positionMs)
				}
				update((s) => ({
					...s,
					playbackState: { ...s.playbackState, position_ms: positionMs },
				}))
				return
			}

			// Optimistic position update to prevent playhead jump-back
			persistPositionImmediate(positionMs)
			update((s) => ({
				...s,
				playbackState: { ...s.playbackState, position_ms: positionMs },
			}))
			try {
				const playbackState = await playerApi.seek(positionMs)
				update((s) => ({ ...s, playbackState, error: null }))
			} catch (error) {
				update((s) => ({
					...s,
					error: error instanceof Error ? error.message : 'Failed to seek',
				}))
			}
		},

		/**
		 * Set volume (source-aware)
		 */
		async setVolume(volume: number) {
			const state = getState()
			setStoredNumber('player.volume', volume)

			if (state.playbackSource === 'preview') {
				if (useNative) {
					void nativePreviewPlayer.setVolume(volume)
				} else {
					previewPlayer.setVolume(volume)
				}
				update((s) => ({
					...s,
					playbackState: { ...s.playbackState, volume },
					error: null,
				}))
				return
			}

			try {
				const playbackState = await playerApi.setVolume(volume)
				update((s) => ({ ...s, playbackState, error: null }))
			} catch (error) {
				update((s) => ({
					...s,
					error: error instanceof Error ? error.message : 'Failed to set volume',
				}))
			}
		},

		/**
		 * Set playback speed (source-aware)
		 */
		async setSpeed(speed: number) {
			const state = getState()
			setStoredNumber('player.speed', speed)

			if (state.playbackSource === 'preview') {
				// Native applies the rate immediately; HTML5 defers to commitPreviewSpeed (frontend-only).
				if (useNative) {
					void nativePreviewPlayer.setRate(speed)
				}
				update((s) => ({
					...s,
					playbackState: { ...s.playbackState, speed },
					error: null,
				}))
				return
			}

			// Optimistic update for immediate UI response
			update((s) => ({
				...s,
				playbackState: { ...s.playbackState, speed },
			}))
			try {
				const playbackState = await playerApi.setSpeed(speed)
				update((s) => ({ ...s, playbackState, error: null }))
			} catch (error) {
				update((s) => ({
					...s,
					error: error instanceof Error ? error.message : 'Failed to set speed',
				}))
			}
		},

		/**
		 * Commit a preview speed change: apply the rate to the audio element,
		 * show the loading spinner, and force a pause/resume to guarantee the
		 * 'playing' event fires when audio actually resumes.
		 */
		commitPreviewSpeed() {
			const state = getState()
			if (state.playbackSource !== 'preview' || !state.previewInfo) return
			if (!state.playbackState.is_playing) return

			// Clear any pending safety timeout from a previous commit
			if (previewSpeedCommitTimeout) {
				clearTimeout(previewSpeedCommitTimeout)
				previewSpeedCommitTimeout = null
			}

			// Apply rate change and show spinner
			previewPlayer.setPlaybackRate(state.playbackState.speed)
			update((s) => ({
				...s,
				previewLoading: s.previewInfo
					? { releaseId: s.previewInfo.releaseId, trackIndex: s.previewInfo.trackIndex }
					: null,
			}))

			// Force pause+resume so the 'playing' event fires when audio resumes
			previewPlayer.pause()
			previewPlayer.resume()

			// Safety timeout: clear spinner if 'playing' never fires
			previewSpeedCommitTimeout = setTimeout(() => {
				previewSpeedCommitTimeout = null
				update((s) => ({ ...s, previewLoading: null }))
			}, 5000)
		},

		/**
		 * Toggle play/pause (source-aware)
		 */
		async togglePlayPause() {
			const state = getState()

			if (state.playbackState.is_playing) {
				await this.pause()
			} else {
				await this.resume()
			}
		},

		/**
		 * Go to the previous preview track. Restarts the current track if past the
		 * PREVIOUS_RESTART_THRESHOLD_MS window (or already on the first track), otherwise jumps to the
		 * previous track in the release. Shared by the in-app transport, the OS media session, and (on
		 * iOS) the native MPRemoteCommandCenter previous command.
		 */
		async previousTrack() {
			const state = getState()
			const info = state.previewInfo
			if (!info) return
			// Native engine applies the same 3s rule and emits track-changed back to the store.
			if (useNative) {
				await nativePreviewPlayer.previous()
				return
			}
			if (state.playbackState.position_ms > PREVIOUS_RESTART_THRESHOLD_MS || info.trackIndex <= 0) {
				await this.seek(0)
				return
			}
			await this.playPreview(info.release, info.trackIndex - 1)
		},

		/**
		 * Go to the next preview track in the release (no-op on the last track).
		 */
		async nextTrack() {
			const state = getState()
			const info = state.previewInfo
			if (!info) return
			if (useNative) {
				await nativePreviewPlayer.next()
				return
			}
			if (info.trackIndex + 1 < info.release.tracks.length) {
				await this.playPreview(info.release, info.trackIndex + 1)
			}
		},

		/**
		 * Seek relative to current position
		 */
		async seekRelative(offsetMs: number) {
			const state = getState()
			const { position_ms, duration_ms } = state.playbackState
			const newPosition = Math.max(0, Math.min(duration_ms, position_ms + offsetMs))
			await this.seek(newPosition)
		},

		/**
		 * Adjust volume by a relative amount
		 */
		async adjustVolume(delta: number) {
			const state = getState()

			// If muted and trying to increase volume, unmute first
			if (state.isMuted && delta > 0) {
				setStoredBoolean('player.isMuted', false)
				update((s) => ({ ...s, isMuted: false }))
			}

			// Clamp to valid range (0.0 - 1.0)
			const newVolume = Math.max(0, Math.min(1, state.playbackState.volume + delta))
			await this.setVolume(newVolume)
		},

		/**
		 * Toggle mute/unmute
		 */
		async toggleMute() {
			const state = getState()

			if (state.isMuted) {
				setStoredBoolean('player.isMuted', false)
				update((s) => ({ ...s, isMuted: false }))
				await this.setVolume(state.volumeBeforeMute)
			} else {
				setStoredBoolean('player.isMuted', true)
				setStoredNumber('player.volumeBeforeMute', state.playbackState.volume)
				update((s) => ({ ...s, isMuted: true, volumeBeforeMute: state.playbackState.volume }))
				await this.setVolume(0)
			}
		},

		/**
		 * Toggle shuffle mode. Persisted device-locally; affects playback order only.
		 */
		toggleShuffle() {
			update((s) => {
				const next = !s.shuffleEnabled
				setStoredBoolean('player.shuffleEnabled', next)
				return { ...s, shuffleEnabled: next }
			})
		},

		/**
		 * Register a callback for when a track finishes playing
		 */
		onTrackEnd(callback: (() => void) | null) {
			onTrackEndCallback = callback
		},

		/**
		 * Register a handler called when playback fails because the track file is missing.
		 * Desktop wires this to the missingTracks store; mobile can leave it unset.
		 */
		setTrackMissingHandler(handler: ((trackId: string) => void) | null) {
			onTrackMissing = handler
		},

		/**
		 * Update is_liked for a preview track (keeps player store in sync with discovery store)
		 */
		setPreviewTrackLiked(trackId: string, isLiked: boolean) {
			update((s) => {
				if (!s.previewInfo) return s
				return {
					...s,
					previewInfo: {
						...s.previewInfo,
						release: {
							...s.previewInfo.release,
							tracks: s.previewInfo.release.tracks.map((t) => (t.id === trackId ? { ...t, is_liked: isLiked } : t)),
						},
					},
				}
			})
		},

		/**
		 * Restore the last-playing library track from localStorage after app init.
		 * Sets the track in the UI at the stored position without loading audio in the backend.
		 */
		restoreTrack(tracks: Track[]) {
			if (restoredPlaybackSource !== 'library' || !restoredTrackId) return
			const track = tracks.find((t) => t.id === restoredTrackId)
			if (!track) return
			isRestoredFromStorage = true
			update((s) => ({
				...s,
				currentTrack: track,
				playbackState: {
					...s.playbackState,
					duration_ms: track.duration_ms || s.playbackState.duration_ms,
				},
			}))
		},

		/**
		 * Restore a preview track from localStorage after app init.
		 * Fetches the release by ID and sets the UI state without loading audio.
		 */
		async restorePreview() {
			if (restoredPlaybackSource !== 'preview' || !restoredPreviewReleaseId) return
			try {
				const release = await discoveryApi.getRelease(restoredPreviewReleaseId)
				const track = release.tracks[restoredPreviewTrackIndex]
				if (!track) return
				isRestoredFromStorage = true
				update((s) => ({
					...s,
					currentTrack: null,
					playbackState: {
						...s.playbackState,
						duration_ms: track.duration_ms || s.playbackState.duration_ms,
					},
					playbackSource: 'preview',
					previewInfo: { releaseId: release.id, release, trackIndex: restoredPreviewTrackIndex },
					previewTrackIndex: restoredPreviewTrackIndex,
				}))
			} catch {
				// Release no longer exists — clear stale persistence silently
				setStoredString('player.playbackSource', 'library')
				setStoredString('player.previewReleaseId', '')
			}
		},

		/**
		 * iOS only: subscribe to the native engine's events and reconcile them into the store. The
		 * native engine is the source of truth for position / play-state / current track on iOS (the
		 * JS timer + previewPlayer events are not used there), so the UI catches up from these events —
		 * including after the WebView resumes from suspension. Returns a cleanup function.
		 */
		async startNativeBridge() {
			return nativePreviewPlayer.startNativePreviewBridge({
				onState: ({ isPlaying, positionMs, durationMs }) => {
					update((s) => {
						if (s.playbackSource !== 'preview') return s
						return {
							...s,
							playbackState: {
								...s.playbackState,
								is_playing: isPlaying,
								position_ms: positionMs,
								duration_ms: durationMs > 0 ? durationMs : s.playbackState.duration_ms,
							},
							previewLoading: isPlaying ? null : s.previewLoading,
						}
					})
					persistPosition(positionMs)
				},
				onTrackChanged: (index) => {
					setStoredNumber('player.previewTrackIndex', index)
					persistPositionImmediate(0)
					update((s) => {
						if (!s.previewInfo) return s
						const track = s.previewInfo.release.tracks[index]
						return {
							...s,
							previewInfo: { ...s.previewInfo, trackIndex: index },
							previewTrackIndex: index,
							playbackState: {
								...s.playbackState,
								position_ms: 0,
								duration_ms: track?.duration_ms ?? s.playbackState.duration_ms,
							},
						}
					})
				},
				onEnded: () => {
					update((s) => ({
						...s,
						playbackState: { ...s.playbackState, is_playing: false },
						previewLoading: null,
					}))
				},
				onError: (message) => {
					update((s) => ({
						...s,
						error: message,
						playbackState: { ...s.playbackState, is_playing: false },
						previewLoading: null,
					}))
					toastStore.error(get(translate)('errors.previewStreamFailed'))
				},
			})
		},

		/**
		 * Reset store to initial state
		 */
		reset() {
			stopPreviewInternal()
			clearPreviewEvents()
			stopPositionTracking()
			onTrackEndCallback = null
			isRestoredFromStorage = false
			setStoredString('player.playbackSource', 'library')
			setStoredString('player.trackId', '')
			setStoredString('player.previewReleaseId', '')
			setStoredNumber('player.positionMs', 0)
			setStoredNumber('player.durationMs', 0)
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

export const shuffleEnabled = derived(playerStore, ($player) => $player.shuffleEnabled)

export const playbackSource = derived(playerStore, ($player) => $player.playbackSource)

export const previewInfo = derived(playerStore, ($player) => $player.previewInfo)

export const previewTrackIndex = derived(playerStore, ($player) => $player.previewTrackIndex)

export const previewLoading = derived(playerStore, ($player) => $player.previewLoading)

// Back-compat: release-level consumers (discovery rows, mini/expanded player) only need "is this release
// loading", so they keep subscribing to this. Returns a string|null, which dedupes cleanly.
export const previewLoadingReleaseId = derived(playerStore, ($player) => $player.previewLoading?.releaseId ?? null)

export const playbackSpeed = derived(playerStore, ($player) => $player.playbackState.speed)
