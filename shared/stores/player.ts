import { writable, derived, get } from 'svelte/store'
import type { Track, PlaybackState, PreviewInfo, DiscoveryRelease } from '../types'
import * as playerApi from '../api/player'
import * as discoveryApi from '../api/discovery'
import * as previewPlayer from '../services/previewPlayer'
import * as nativePreviewPlayer from '../services/nativePreviewPlayer'
import type { NativeTrack } from '../services/nativePreviewPlayer'
import * as playbackQueue from './playbackQueue'
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

// Upper bound on how many upcoming tracks the iOS native window pre-resolves up front (each costs a
// stream fetch). Large enough to cover a typical release for seamless locked playback, capped so a huge
// album / queue can't trigger an unbounded burst of resolves.
const NATIVE_WINDOW_CAP = 20

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

	// --- iOS native sliding window ------------------------------------------------------------------
	// The two-tier queue logic (context + explicit user queue, shuffle, history) lives in
	// playbackQueue.ts; this store drives playback and, on iOS, keeps the native engine fed with a
	// lazily-resolved window of upcoming items so queued tracks and cross-release advances stay gapless
	// while the screen is locked. `nativeWindow` mirrors the engine's `entries` array index-for-index (so
	// its track-changed index maps straight to a pick); `nativeIndex` mirrors the engine's current index.
	// Both stay empty/unused on the HTML5 path; the queue stays empty on desktop (its own queue runs in
	// useAppSetup), so every queue branch below is inert there.
	let nativeWindow: Array<{ release: DiscoveryRelease; trackIndex: number }> = []
	let nativeIndex = 0
	// Session cache of resolved stream (proxy) URLs, keyed `releaseId:trackIndex`. The proxy URLs are
	// stable per release/track, so re-feeding the window on each advance/mutation doesn't re-resolve the
	// same picks. Cleared on stop/reset; an entry is dropped + the backend cache invalidated on error.
	const streamUrlCache = new Map<string, string>()

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
				// Persist the playhead (throttled) so a preview's progress survives an app restart. The
				// HTML5 path has no equivalent of the iOS native bridge's position persistence, and the
				// library-only JS position timer doesn't run for previews, so without this the saved
				// position only updates on pause/seek — a kill mid-play would lose the progress.
				persistPosition(positionMs)
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

	// --- iOS native window resolution ---------------------------------------------------------------
	// Resolve (and session-cache) a track's proxy stream URL. The proxy URLs are stable per release/track,
	// so the window can be re-fed cheaply on every advance/mutation.
	async function resolveStreamUrl(release: DiscoveryRelease, trackIndex: number): Promise<string> {
		const cacheKey = `${release.id}:${trackIndex}`
		const cached = streamUrlCache.get(cacheKey)
		if (cached) return cached
		const track = release.tracks[trackIndex]
		const url = await discoveryApi.fetchPreviewStream(release.id, track.position)
		streamUrlCache.set(cacheKey, url)
		return url
	}

	// Turn picks into the engine's NativeTrack entries, resolving each one's stream URL. MIME + metadata
	// are per-release (a window can span releases / sources): YouTube/Discogs need an explicit audio/mp4
	// (their proxy URL is extensionless and AVFoundation can't infer it); Bandcamp/SoundCloud pass null.
	async function buildNativeTracks(
		picks: Array<{ release: DiscoveryRelease; trackIndex: number }>
	): Promise<NativeTrack[]> {
		return Promise.all(
			picks.map(async (p) => {
				const track = p.release.tracks[p.trackIndex]
				const url = await resolveStreamUrl(p.release, p.trackIndex)
				const mimeType = p.release.source_type === 'discogs' || p.release.source_type === 'youtube' ? 'audio/mp4' : null
				return {
					url,
					title: track.name,
					artist: p.release.artist ?? '',
					album: p.release.title ?? '',
					durationMs: track.duration_ms ?? 0,
					artworkUrl: p.release.artwork_url ?? null,
					mimeType,
				}
			})
		)
	}

	// How deep to pre-resolve the window: the rest of the current release (so locked within-release
	// advance stays seamless, as before) + the user queue + a small cross-release/shuffle margin, capped.
	function nativeWindowDepth(release: DiscoveryRelease, trackIndex: number): number {
		const shuffle = getState().shuffleEnabled
		const ahead = shuffle ? 3 : Math.max(0, release.tracks.length - 1 - trackIndex) + 2
		return Math.min(NATIVE_WINDOW_CAP, playbackQueue.userQueueLength() + ahead)
	}

	// Feed the native engine the window around `current`. 'reload' loads it fresh (a new/changed current
	// track); 'slide' replaces only the upcoming tail in place (a mutation or a post-advance refill) so the
	// current item keeps playing untouched. Keeps `nativeWindow` in lockstep with the engine's `entries`.
	async function feedNativeWindow(
		current: { release: DiscoveryRelease; trackIndex: number },
		mode: 'reload' | 'slide',
		// Only used on 'reload': begin the current track this many ms in (0 = from the start). Non-zero
		// when restoring the last session on relaunch so the engine starts at the saved position.
		startPositionMs = 0
	) {
		const picks = playbackQueue.peekUpcoming(nativeWindowDepth(current.release, current.trackIndex))
		if (mode === 'reload') {
			const tracks = await buildNativeTracks([current, ...picks])
			await nativePreviewPlayer.play(tracks, 0, startPositionMs)
			nativeWindow = [current, ...picks]
			nativeIndex = 0
		} else {
			const tracks = await buildNativeTracks(picks)
			await nativePreviewPlayer.setUpcoming(tracks)
			nativeWindow = nativeWindow.slice(0, nativeIndex + 1).concat(picks)
		}
	}

	// Re-feed the upcoming window tail (slide forward / apply a queue mutation) around the current track.
	// Coalesced on a short timer so a burst of events settles into ONE slide: that matters when the iOS
	// WebView resumes from suspension and replays a backlog of native track-changed events — we must wait
	// until our index has caught up to the engine's before truncating its tail, or the two desync.
	let slideTimer: ReturnType<typeof setTimeout> | null = null
	function scheduleNativeSlide() {
		if (!useNative) return
		if (slideTimer) clearTimeout(slideTimer)
		slideTimer = setTimeout(() => {
			slideTimer = null
			const state = getState()
			if (state.playbackSource !== 'preview' || !state.previewInfo) return
			const { release, trackIndex } = state.previewInfo
			void feedNativeWindow({ release, trackIndex }, 'slide').catch((e) =>
				console.error('[native-preview] window slide failed:', e)
			)
		}, 60)
	}

	// A queue mutation (add / play-next / remove / reorder / shuffle toggle) changed what's upcoming. On
	// iOS, re-feed the window tail so it takes effect — even mid-playback — without disturbing the current
	// track. No-op on the HTML5 path (it re-resolves per track) or with no preview.
	function handleQueueChanged() {
		if (!useNative) return
		const state = getState()
		if (state.playbackSource !== 'preview' || !state.previewInfo) return
		scheduleNativeSlide()
	}
	playbackQueue.setQueueChangedHandler(handleQueueChanged)
	playbackQueue.initShuffle(initialState.shuffleEnabled)

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
		 * Play a preview of a discovery release track. `startPositionMs` begins playback that many ms into
		 * the track (0 = from the start); it's non-zero only when resuming a session restored from storage,
		 * so the track picks up where it left off. Currently honored on the iOS native path (the HTML5
		 * restore path seeks in `resume()` itself).
		 */
		async playPreview(
			release: DiscoveryRelease,
			trackIndex: number = 0,
			queue?: DiscoveryRelease[],
			startPositionMs = 0
		) {
			previewRetryAttempted = false
			const state = getState()
			const track = release.tracks[trackIndex]
			if (!track) return

			// Capture the playback queue (the whole list from the view) on a user-initiated preview, so
			// next/previous/auto-advance + shuffle span every release on screen. Internal re-drives
			// (next/previous, the iOS mode re-feed) omit `queue` to preserve the captured list + session.
			if (queue) {
				playbackQueue.startSession(release, trackIndex, queue)
			}

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

			// iOS: feed the native engine a window starting at this track so it can switch tracks — including
			// across releases and into queued items, and while the screen is locked — without further JS. The
			// window's first item is what we're about to play; playbackQueue (synced by the caller) supplies
			// the upcoming tail.
			if (useNative) {
				try {
					await feedNativeWindow({ release, trackIndex }, 'reload', startPositionMs)
					await nativePreviewPlayer.setVolume(state.isMuted ? 0 : state.playbackState.volume)
					// Apply the active/persisted tempo: native_preview_play starts a fresh AVPlayer item at
					// 1.0x, so without this a (re)start — including restore-then-resume, or starting a new
					// release after a tempo change — would play at normal speed while the UI shows the offset.
					void nativePreviewPlayer.setRate(state.playbackState.speed)
					isRestoredFromStorage = false
					setStoredString('player.playbackSource', 'preview')
					setStoredString('player.previewReleaseId', release.id)
					setStoredNumber('player.previewTrackIndex', trackIndex)
					setStoredNumber('player.durationMs', track.duration_ms || 0)
					persistPositionImmediate(startPositionMs)
					update((s) => ({
						...s,
						currentTrack: null,
						playbackState: {
							...s.playbackState,
							is_playing: true,
							position_ms: startPositionMs,
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
					console.error('[native-preview] playPreview failed before/at native play:', errorMsg)
					update((s) => ({ ...s, error: errorMsg, previewLoading: null }))
					// Show the real error during debugging (normally the generic string).
					toastStore.error(`Preview failed: ${errorMsg}`)
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
					// Restored from storage (app relaunch): the native engine lost its in-memory playlist, so
					// re-issue playPreview to rebuild it — starting at the persisted position so the track picks
					// up where it left off. The engine seeks before the item renders, so there's no blip back to
					// the start (unlike seeking after playback has already begun from 0).
					if (isRestoredFromStorage && state.previewInfo) {
						isRestoredFromStorage = false
						await this.playPreview(
							state.previewInfo.release,
							state.previewInfo.trackIndex,
							undefined,
							state.playbackState.position_ms
						)
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
			playbackQueue.clearAll()
			streamUrlCache.clear()
			nativeWindow = []
			nativeIndex = 0

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
		 * Go to the previous preview track. Honours the shared PREVIOUS_RESTART_THRESHOLD_MS restart rule,
		 * then steps back through the unified play history (which spans the user queue and both shuffle and
		 * sequential context); at the very start it crosses to the previous context release (sequential) or
		 * restarts. Shared by the in-app transport, the OS media session, and the iOS native command.
		 */
		async previousTrack() {
			const state = getState()
			if (!state.previewInfo) return

			if (state.playbackState.position_ms > PREVIOUS_RESTART_THRESHOLD_MS) {
				await this.seek(0)
				return
			}
			const pick = playbackQueue.advancePrev()
			if (pick) await this.playPreview(pick.release, pick.trackIndex)
			else await this.seek(0)
		},

		/**
		 * Advance to the next preview track via the two-tier queue: the explicit user queue first, then the
		 * context (shuffle picks across the whole list, or sequential within/across releases, wrapping). On
		 * iOS, when the engine still holds an item ahead in the loaded window we let it advance natively
		 * (seamless, works locked) and reconcile in onTrackChanged; past the window we resolve + load fresh.
		 */
		async nextTrack() {
			const state = getState()
			if (!state.previewInfo) return

			if (useNative && nativeIndex + 1 < nativeWindow.length) {
				await nativePreviewPlayer.next()
				return
			}
			const pick = playbackQueue.advanceNext()
			if (pick) await this.playPreview(pick.release, pick.trackIndex)
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
			const state = getState()
			const next = !state.shuffleEnabled
			setStoredBoolean('player.shuffleEnabled', next)
			update((s) => ({ ...s, shuffleEnabled: next }))
			// Re-anchor + redraw the CONTEXT order on the current track (the user queue stays put). On iOS
			// this also re-feeds the native window tail in place via the queue-changed handler, so toggling
			// mid-track takes effect immediately without disturbing the currently-playing track.
			playbackQueue.setShuffle(next)
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
				// Seed the queue module so transport + Up Next work after a restore: the context is just this
				// release (the feed isn't persisted), then re-hydrate the persisted explicit user queue.
				playbackQueue.startSession(release, restoredPreviewTrackIndex, [release])
				void playbackQueue.hydrate()
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
					// The engine moved to entries-index `index`. Sync the queue module to match: each forward
					// step consumes one upcoming pick into history; a backward step (the lock-screen Previous,
					// which the native engine handles itself within the kept-played front of the window) walks
					// history back. A forward jump greater than one can happen if JS was suspended while the
					// engine auto-advanced through several pre-fed items. Then point previewInfo at the window
					// pick and, after a forward move, slide the window so there are always fresh items ahead.
					if (index === nativeIndex) return
					const forward = index > nativeIndex
					if (forward) {
						for (let i = nativeIndex; i < index; i++) playbackQueue.advanceNext()
					} else {
						for (let i = nativeIndex; i > index; i--) playbackQueue.advancePrev()
					}
					nativeIndex = index
					const pick = nativeWindow[index]
					if (!pick) return
					setStoredNumber('player.previewTrackIndex', pick.trackIndex)
					setStoredString('player.previewReleaseId', pick.release.id)
					persistPositionImmediate(0)
					update((s) => ({
						...s,
						previewInfo: { releaseId: pick.release.id, release: pick.release, trackIndex: pick.trackIndex },
						previewTrackIndex: pick.trackIndex,
						playbackState: {
							...s.playbackState,
							position_ms: 0,
							duration_ms: pick.release.tracks[pick.trackIndex]?.duration_ms ?? s.playbackState.duration_ms,
						},
					}))
					if (forward) scheduleNativeSlide()
				},
				onEnded: () => {
					console.log('[native-preview] ENDED')
					update((s) => ({
						...s,
						playbackState: { ...s.playbackState, is_playing: false },
						previewLoading: null,
					}))
					// The engine reached the end of its loaded playlist — the single shuffle track, or the last
					// track of a sequential release. Hand off to the same auto-advance the HTML5 path uses
					// (wired in +layout) so playback continues across the queue / shuffle.
					onTrackEndCallback?.()
				},
				onError: (message) => {
					console.error('[native-preview] engine error:', message)
					// A windowed item failed to load (e.g. an expired/stale upstream stream). Drop the cached
					// proxy URLs so the next feed re-resolves them fresh rather than re-handing the bad ones.
					streamUrlCache.clear()
					update((s) => ({
						...s,
						error: message,
						playbackState: { ...s.playbackState, is_playing: false },
						previewLoading: null,
					}))
					// Show the real AVPlayer error during debugging (normally the generic string).
					toastStore.error(`Player error: ${message}`)
				},
				onDebug: (message) => {
					console.log('[native-preview][engine]', message)
					toastStore.error(`dbg: ${message}`)
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
			playbackQueue.clearAll()
			streamUrlCache.clear()
			nativeWindow = []
			nativeIndex = 0
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
