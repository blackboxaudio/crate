import { writable, derived, get } from 'svelte/store'
import { dedupe } from '../utils/stores'
import type { Track, PlaybackState, PreviewInfo, DiscoveryRelease } from '../types'
import * as playerApi from '../api/player'
import * as discoveryApi from '../api/discovery'
import * as previewPlayer from '../services/previewPlayer'
import * as nativePreviewPlayer from '../services/nativePreviewPlayer'
import type { NativeTrack } from '../services/nativePreviewPlayer'
import * as playbackQueue from './playbackQueue'
import type { RepeatMode } from './playbackQueue'
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

export type { RepeatMode } from './playbackQueue'

// Repeat cycles one step per button tap. Order per spec: off → track → release → context.
const REPEAT_CYCLE: RepeatMode[] = ['off', 'track', 'release', 'context']

interface PlayerState {
	currentTrack: Track | null
	playbackState: PlaybackState
	error: string | null
	isMuted: boolean
	volumeBeforeMute: number
	shuffleEnabled: boolean
	repeatMode: RepeatMode
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
	repeatMode: getStoredString<RepeatMode>('player.repeatMode', 'off', ['off', 'track', 'release', 'context']),
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
	// Called when a discovery preview starts playing, so the discovery store can clear the release's
	// "new" flag (listened → no longer new). Injected to avoid a circular import: the discovery store
	// already depends on this player store, so it registers the handler rather than us importing it.
	let onPreviewPlayed: ((releaseId: string) => void) | null = null
	// Called when the iOS lock-screen Like toggled a track natively (DB already updated by the
	// engine); the discovery store registers this to mirror the change into the JS stores. Same
	// inversion as onPreviewPlayed — this store must not import the discovery store.
	let onNativeLikeChanged: ((trackId: string, isLiked: boolean) => void) | null = null
	let previewRetryAttempted = false
	let previewRetrying = false
	// One playback failure can surface through more than one layer in quick succession (engine event,
	// element error, resolution catch), and offline auto-advance can fail several picks back-to-back.
	// Collapse them so the user sees exactly one error toast per failure burst.
	let lastPreviewErrorToastAt = 0
	function toastPreviewError(message: string) {
		const now = Date.now()
		if (now - lastPreviewErrorToastAt < 1500) return
		lastPreviewErrorToastAt = now
		toastStore.error(message)
	}
	// "Definitely offline" signal for graceful degradation: cached tracks still play through the
	// local proxy, uncached ones get a clear offline toast / are skipped by auto-advance instead
	// of a generic network error. No reachability polling — false only means "not sure".
	const isOffline = () => typeof navigator !== 'undefined' && navigator.onLine === false
	let previewSpeedCommitTimeout: ReturnType<typeof setTimeout> | null = null
	let isRestoredFromStorage = false
	let lastPositionWriteTime = 0
	// While an iOS native seek is in flight, the engine's 0.5s state ticks can still carry the
	// pre-seek position: the Rust-side `seeking` guard only engages once the seek command reaches the
	// main thread, so a tick emitted in that IPC window would clobber the optimistic position (the
	// scrubber "snaps back"). While set, stale position updates are ignored until the engine echoes a
	// position near the target (the Rust seek emits the exact target immediately on landing) or a
	// timeout passes — failing open so a dropped command can't freeze the playhead.
	let pendingNativeSeek: { targetMs: number; issuedAt: number } | null = null
	const NATIVE_SEEK_SETTLE_TOLERANCE_MS = 1500
	const NATIVE_SEEK_TIMEOUT_MS = 2000
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
	// Monotonic token making overlapping track transitions last-request-wins: rapid next/previous (swipe
	// paging) can start a new `playPreview` while an earlier one is still resolving its stream, and the
	// EARLIER one may finish LAST (cached vs uncached fetch) — without this it would clobber previewInfo
	// and the audio with a track the queue has already moved past, desyncing queue/UI/audio. Each
	// transition takes a fresh generation; a completion whose generation is stale silently stands down.
	let previewLoadGen = 0

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

	// HTML5 path safety net. The loading spinner is retired by the element's `playing` event, so a stream
	// that resolves but never actually starts (undecodable container, a proxy download that never lands)
	// would spin forever. Mirrors the native engine's load watchdog window so both platforms give up at
	// the same point: back to idle, with an error toast.
	const PREVIEW_LOAD_TIMEOUT_MS = 15000
	let previewLoadTimeout: ReturnType<typeof setTimeout> | null = null

	function clearPreviewLoadTimeout() {
		if (previewLoadTimeout) {
			clearTimeout(previewLoadTimeout)
			previewLoadTimeout = null
		}
	}

	function armPreviewLoadTimeout() {
		clearPreviewLoadTimeout()
		previewLoadTimeout = setTimeout(() => {
			previewLoadTimeout = null
			const state = getState()
			// Already playing (or moved on) — nothing to time out.
			if (state.playbackSource !== 'preview' || !state.previewLoading) return
			console.error('[preview] stream never started playing within timeout')
			clearPreviewEvents()
			stopPreviewInternal()
			update((s) => ({
				...s,
				playbackState: { ...s.playbackState, is_playing: false },
				previewLoading: null,
			}))
			toastPreviewError(get(translate)('errors.previewStreamFailed'))
		}, PREVIEW_LOAD_TIMEOUT_MS)
	}

	function wirePreviewEvents() {
		previewPlayer.setOnTimeUpdate((positionMs: number) => {
			update((state) => {
				const { duration_ms } = state.playbackState
				// When the Audio element reports a position past the metadata duration,
				// the stream container is longer than the actual audio (e.g. proxied
				// YouTube/Discogs ~2x duration). Stop playback and trigger track end.
				if (duration_ms > 0 && positionMs >= duration_ms) {
					// Repeat-track: rewind in place. The element is still rolling (the container outlasts
					// the real audio), so a seek loops the track with no stop/start and no re-fetch.
					if (state.repeatMode === 'track' && state.previewInfo) {
						setTimeout(() => {
							previewPlayer.seek(0)
							persistPositionImmediate(0)
						}, 0)
						return {
							...state,
							playbackState: { ...state.playbackState, position_ms: 0 },
						}
					}
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
			const state = getState()
			// Repeat-track: the element still holds the stream after `ended` — rewind and play it again
			// in place (no re-fetch, no session reset). The auto-advance callback never runs.
			if (state.repeatMode === 'track' && state.playbackSource === 'preview' && state.previewInfo) {
				previewPlayer.seek(0)
				previewPlayer.resume()
				persistPositionImmediate(0)
				update((s) => ({
					...s,
					playbackState: { ...s.playbackState, position_ms: 0, is_playing: true },
				}))
				return
			}
			update((s) => ({
				...s,
				playbackState: { ...s.playbackState, is_playing: false },
			}))
			onTrackEndCallback?.()
		})
		previewPlayer.setOnWaiting(() => {
			const state = getState()
			// An already-set `previewLoading` is left alone: `playPreview` points it at the track being
			// switched TO before `previewInfo` catches up, and `waiting` fires inside that gap — so
			// deriving from `previewInfo` here would move the spinner onto the outgoing track's row.
			if (state.playbackSource === 'preview' && state.previewInfo && !state.previewLoading) {
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
			clearPreviewLoadTimeout()
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
				// A route change (Bluetooth disconnect) can fault the element, so a retry must restore
				// whatever state we were in — never force playback on. Re-resolving a paused track is
				// fine; starting it is not.
				const wasPlaying = state.playbackState.is_playing
				if (track) {
					console.warn(`Preview stream error, retrying: ${msg}`)
					try {
						await discoveryApi.invalidatePreviewStreamCache(release.id)
						const streamUrl = await discoveryApi.fetchPreviewStream(release.id, track.position)
						previewPlayer.play(streamUrl)
						if (!wasPlaying) previewPlayer.pause()
						update((s) => ({
							...s,
							error: null,
							playbackState: { ...s.playbackState, is_playing: wasPlaying, position_ms: 0 },
							// A paused retry never fires `playing`, so retire the spinner here; a resuming one
							// keeps it until the element reports audio (re-armed below).
							previewLoading: wasPlaying ? s.previewLoading : null,
						}))
						if (wasPlaying) armPreviewLoadTimeout()
						else clearPreviewLoadTimeout()
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
			clearPreviewLoadTimeout()
			update((s) => ({
				...s,
				error: msg,
				playbackState: { ...s.playbackState, is_playing: false },
				previewLoading: null,
			}))
			toastPreviewError(get(translate)('errors.previewStreamFailed'))
		})
	}

	function clearPreviewEvents() {
		previewPlayer.setOnTimeUpdate(null)
		previewPlayer.setOnDurationChange(null)
		previewPlayer.setOnEnded(null)
		previewPlayer.setOnError(null)
		previewPlayer.setOnWaiting(null)
		previewPlayer.setOnPlaying(null)
		// `playing` can no longer arrive to retire the loading timeout, so drop it with the handlers.
		clearPreviewLoadTimeout()
	}

	// --- iOS native window resolution ---------------------------------------------------------------
	// Resolve (and session-cache) a track's proxy stream URL. The proxy URLs are stable per release/track,
	// so the window can be re-fed cheaply on every advance/mutation. `background` marks opportunistic
	// look-ahead resolution, which the backend throttles behind user-initiated fetches.
	async function resolveStreamUrl(release: DiscoveryRelease, trackIndex: number, background = false): Promise<string> {
		const cacheKey = `${release.id}:${trackIndex}`
		const cached = streamUrlCache.get(cacheKey)
		if (cached) return cached
		const track = release.tracks[trackIndex]
		const url = await discoveryApi.fetchPreviewStream(release.id, track.position, background)
		streamUrlCache.set(cacheKey, url)
		return url
	}

	// Build a single engine entry for a pick: resolve its (session-cached) proxy stream URL and map the
	// per-release metadata + MIME. MIME is per-release (a window can span releases / sources): YouTube/Discogs
	// need an explicit audio/mp4 (their proxy URL is extensionless and AVFoundation can't infer it);
	// Bandcamp/SoundCloud pass null. Throws if the URL can't be resolved — the caller decides whether that
	// track is required (the tapped/current one) or best-effort (an upcoming window pick).
	async function buildOneNativeTrack(
		p: { release: DiscoveryRelease; trackIndex: number },
		background = false
	): Promise<NativeTrack> {
		const track = p.release.tracks[p.trackIndex]
		const url = await resolveStreamUrl(p.release, p.trackIndex, background)
		const mimeType = p.release.source_type === 'discogs' || p.release.source_type === 'youtube' ? 'audio/mp4' : null
		return {
			url,
			title: track.name,
			artist: p.release.artist ?? '',
			album: p.release.title ?? '',
			durationMs: track.duration_ms ?? 0,
			artworkUrl: p.release.artwork_url ?? null,
			mimeType,
			trackId: track.id ?? null,
			isLiked: track.is_liked ?? false,
		}
	}

	// Resolve the upcoming window picks best-effort: a pick that fails to resolve (dead / expired /
	// rate-limited stream) is DROPPED rather than rejecting the whole window, so one bad neighbour can't
	// abort playback of the track the user actually chose. Returns survivors paired with their originating
	// pick (order preserved) so the engine's `entries` and our `nativeWindow` stay index-aligned.
	async function resolveWindowTail(
		picks: Array<{ release: DiscoveryRelease; trackIndex: number }>
	): Promise<Array<{ pick: { release: DiscoveryRelease; trackIndex: number }; track: NativeTrack }>> {
		const settled = await Promise.all(
			picks.map(async (pick) => {
				try {
					// Background priority: window tails are opportunistic and must not
					// delay the (foreground) current-track resolution.
					return { pick, track: await buildOneNativeTrack(pick, true) }
				} catch (e) {
					console.warn('[native-preview] dropping unresolvable window track:', e)
					return null
				}
			})
		)
		return settled.filter((r) => r !== null) as Array<{
			pick: { release: DiscoveryRelease; trackIndex: number }
			track: NativeTrack
		}>
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
	//
	// 'reload' resolves ONLY the tapped track before starting the engine — the upcoming tail is filled in
	// afterwards by `scheduleNativeSlide` (which the caller kicks off once `previewInfo` is set). This used
	// to await the whole window, so the track the user actually chose waited behind up to `NATIVE_WINDOW_CAP`
	// unrelated stream resolutions running two-at-a-time through the backend's fetch semaphore — seconds of
	// dead air on YouTube/Discogs, where every tail entry is its own extraction.
	async function feedNativeWindow(
		current: { release: DiscoveryRelease; trackIndex: number },
		mode: 'reload' | 'slide',
		// Only used on 'reload': begin the current track this many ms in (0 = from the start). Non-zero
		// when restoring the last session on relaunch so the engine starts at the saved position.
		startPositionMs = 0,
		// On 'reload': the transition generation this feed belongs to. If a newer transition starts while
		// this one is still resolving, it must NOT start the engine or clobber the window mapping.
		gen?: number
	) {
		if (mode === 'reload') {
			// A fresh current track supersedes any in-flight seek on the previous one.
			pendingNativeSeek = null
			// The current (tapped/restored) track is required — let it throw so a genuine failure of the
			// chosen track still surfaces to playPreview's catch.
			const currentTrack = await buildOneNativeTrack(current)
			if (gen !== undefined && gen !== previewLoadGen) return // superseded while resolving
			// The window mapping is reset BEFORE the engine is told to load. The engine emits its
			// track-changed(0) echo from the main thread, and that event can reach the WebView before
			// this invoke resolves — reconciling that echo against the PREVIOUS session's nativeIndex
			// made onTrackChanged walk the freshly-seeded queue backwards/forwards, silently corrupting
			// what plays next (wrong Up Next, wrong repeat wraps, replayed tracks). With the reset
			// first, the echo lands on a matching index and no-ops; events from any earlier load are
			// dropped by their stale load id (see onTrackChanged / onEnded).
			nativeWindow = [current]
			nativeIndex = 0
			await nativePreviewPlayer.play([currentTrack], 0, startPositionMs, gen ?? previewLoadGen)
			return
		}

		// Slide: only the upcoming tail is (re)fed, so it's entirely best-effort — the current item keeps
		// playing untouched. Keep `nativeWindow` aligned with the survivors we actually hand the engine.
		const picks = playbackQueue.peekUpcoming(nativeWindowDepth(current.release, current.trackIndex))
		const tail = await resolveWindowTail(picks)
		await nativePreviewPlayer.setUpcoming(tail.map((t) => t.track))
		nativeWindow = nativeWindow.slice(0, nativeIndex + 1).concat(tail.map((t) => t.pick))
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

	// Point the store at a native-window pick that is (about to be) the engine's current item: persist it
	// and update previewInfo/position. Shared by the engine's track-changed reconciliation and the
	// OPTIMISTIC in-window next/previous (which apply the step synchronously before telling the engine,
	// so the queue can never be consulted while a native advance is still un-reconciled).
	function applyNativeTrackChange(pick: { release: DiscoveryRelease; trackIndex: number }) {
		pendingNativeSeek = null
		// A new current track gets a fresh auto-retry budget (engine advances bypass playPreview,
		// which is where the HTML5 path resets this).
		previewRetryAttempted = false
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
			// The spinner is deliberately NOT raised here. The incoming item may or may not be buffered,
			// and the engine already tells us which: it emits state for the new item immediately after
			// this track-changed event, and its load watchdog keeps reporting until audio rolls. Guessing
			// "loading" here would flash the spinner on every gapless advance through a pre-fed window.
		}))
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
	// Native repeat-mode syncs are CHAINED, never fired concurrently: each `native_preview_set_repeat_mode`
	// invoke is an async command with no cross-invoke ordering guarantee, so rapid mode cycling (two quick
	// taps: off → track → release) could land the stale 'track' sync after the newer 'release' one — leaving
	// the engine's item-loop flag on, which replays the ending track instead of advancing. Awaiting each
	// invoke before sending the next guarantees the engine applies them in order (main-queue FIFO).
	let repeatSyncChain: Promise<void> = Promise.resolve()
	function syncNativeRepeatMode(mode: RepeatMode) {
		if (!useNative) return
		repeatSyncChain = repeatSyncChain.then(() => nativePreviewPlayer.setRepeatMode(mode)).catch(() => {})
	}

	// Change the repeat mode (button cycle or the iOS lock-screen repeat command). Persisted
	// device-locally like shuffle; the queue redraws its committed lookahead under the new scope (which
	// also re-feeds the iOS window via the queue-changed handler), and the native engine follows —
	// item-loop flag for repeat-track plus the lock-screen repeat glyph.
	function setRepeatModeInternal(mode: RepeatMode) {
		const state = getState()
		if (state.repeatMode === mode) return
		setStoredString('player.repeatMode', mode)
		update((s) => ({ ...s, repeatMode: mode }))
		playbackQueue.setRepeatMode(mode)
		syncNativeRepeatMode(mode)
	}

	playbackQueue.setQueueChangedHandler(handleQueueChanged)
	playbackQueue.initShuffle(initialState.shuffleEnabled)
	playbackQueue.initRepeatMode(initialState.repeatMode)

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
					// Switching to a library track abandons any preview that was still loading.
					previewLoading: null,
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
		 *
		 * Returns true when playback started — superseded transitions also report true, since a stale
		 * failure must not look real to nextTrack's offline skip loop — and false when it failed.
		 * `opts.silentError` suppresses the failure toast (the skip loop's non-first attempts).
		 */
		async playPreview(
			release: DiscoveryRelease,
			trackIndex: number = 0,
			queue?: DiscoveryRelease[],
			startPositionMs = 0,
			opts?: { silentError?: boolean }
		): Promise<boolean> {
			previewRetryAttempted = false
			const state = getState()
			const track = release.tracks[trackIndex]
			if (!track) return false
			// Last-request-wins: rapid next/previous can overlap transitions whose stream fetches finish
			// out of order. Only the newest transition may start audio / write state; the queue (already
			// advanced synchronously by the caller) stays the single source of truth.
			const gen = ++previewLoadGen

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
					await feedNativeWindow({ release, trackIndex }, 'reload', startPositionMs, gen)
					if (gen !== previewLoadGen) return true // superseded — the newer transition owns audio + state
					await nativePreviewPlayer.setVolume(state.isMuted ? 0 : state.playbackState.volume)
					// Apply the active/persisted tempo: native_preview_play starts a fresh AVPlayer item at
					// 1.0x, so without this a (re)start — including restore-then-resume, or starting a new
					// release after a tempo change — would play at normal speed while the UI shows the offset.
					void nativePreviewPlayer.setRate(state.playbackState.speed)
					// Re-assert the repeat mode on every native session start: a reload is a natural
					// self-heal point if the engine's item-loop flag ever fell out of step with app state.
					syncNativeRepeatMode(state.repeatMode)
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
						// `previewLoading` deliberately stays SET. `native_preview_play` is fire-and-forget —
						// it dispatches to the main thread and returns before AVPlayer has touched the URL — so
						// clearing the spinner here dropped the user into a stretch of silence that still
						// rendered as "playing". The engine's state events retire it once audio is really
						// rolling (see the `isBuffering` handling in `onState`).
					}))
					// The engine is playing the chosen track alone; fill in the upcoming window now that
					// `previewInfo` is set (the slide reads it), off the critical path.
					scheduleNativeSlide()
					// Listened → clear the release's "new" flag (desktop/mobile agnostic; no-op if unset).
					onPreviewPlayed?.(release.id)
				} catch (error) {
					if (gen !== previewLoadGen) return true // superseded — a failure of a stale transition is noise
					const errorMsg = error instanceof Error ? error.message : 'Failed to fetch preview stream'
					console.error('[native-preview] playPreview failed before/at native play:', errorMsg)
					update((s) => ({ ...s, error: errorMsg, previewLoading: null }))
					if (!opts?.silentError) {
						toastPreviewError(
							get(translate)(isOffline() ? 'errors.previewNotAvailableOffline' : 'errors.previewStreamFailed')
						)
					}
					return false
				}
				return true
			}

			try {
				const streamUrl = await discoveryApi.fetchPreviewStream(release.id, track.position)
				if (gen !== previewLoadGen) return true // superseded — the newer transition owns audio + state

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
					// Held (as on the native path) until the element's `playing` event fires — clearing it
					// here only for `waiting` to re-raise it a moment later reads as a spinner flicker.
				}))
				armPreviewLoadTimeout()
				// Listened → clear the release's "new" flag (desktop/mobile agnostic; no-op if unset).
				onPreviewPlayed?.(release.id)
			} catch (error) {
				if (gen !== previewLoadGen) return true // superseded — a failure of a stale transition is noise
				const errorMsg = error instanceof Error ? error.message : 'Failed to fetch preview stream'
				update((s) => ({ ...s, error: errorMsg, previewLoading: null }))
				if (!opts?.silentError) {
					toastPreviewError(
						get(translate)(isOffline() ? 'errors.previewNotAvailableOffline' : 'errors.previewStreamFailed')
					)
				}
				return false
			}
			return true
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
				// Pausing mid-load retires the spinner: neither the element's `playing` event nor the
				// engine's buffering signal will arrive to do it, so it would otherwise strand.
				clearPreviewLoadTimeout()
				update((s) => ({
					...s,
					playbackState: { ...s.playbackState, is_playing: false },
					error: null,
					previewLoading: null,
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
		 * The backend paused library playback on its own because the output device it was
		 * playing on disappeared (Bluetooth headphones powered off, interface unplugged).
		 * Rust has already rebuilt the stream on the new default and left it paused at this
		 * position — we only mirror that state, we don't issue another transport command.
		 *
		 * Preview playback is a WebView <audio> element the OS re-routes on its own, so this
		 * must never touch it: forcing `is_playing: false` there would desync the store from
		 * an element that is very likely still playing on the new route.
		 */
		applyExternalPause(playbackState: PlaybackState) {
			const state = getState()
			if (state.playbackSource !== 'library') return

			stopPositionTracking()
			persistPositionImmediate(playbackState.position_ms)
			update((s) => ({ ...s, playbackState, error: null }))
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
						armPreviewLoadTimeout()
						update((s) => ({
							...s,
							playbackState: { ...s.playbackState, is_playing: true },
							previewLoading: { releaseId: release.id, trackIndex },
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
			pendingNativeSeek = null
			// Invalidate any in-flight transition so its late completion can't resurrect the preview.
			previewLoadGen++

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
				clearPreviewLoadTimeout()
				update((s) => ({
					...s,
					currentTrack: null,
					playbackState: { ...initialPlaybackState, volume: s.playbackState.volume, speed: s.playbackState.speed },
					error: null,
					playbackSource: 'library',
					previewInfo: null,
					previewTrackIndex: 0,
					// Stopping mid-load must retire the spinner — no `playing` event / buffering tick is
					// coming to do it, and `previewInfo` is gone so nothing would ever match it again.
					previewLoading: null,
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
					pendingNativeSeek = { targetMs: positionMs, issuedAt: Date.now() }
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
		 * `skipRestartThreshold` bypasses the restart rule — a swipe gesture is spatial navigation, so it
		 * always means "previous track" regardless of how far into the current one playback is.
		 */
		async previousTrack(opts?: { skipRestartThreshold?: boolean }) {
			const state = getState()
			if (!state.previewInfo) return

			if (!opts?.skipRestartThreshold && state.playbackState.position_ms > PREVIOUS_RESTART_THRESHOLD_MS) {
				await this.seek(0)
				return
			}
			// iOS in-window fast path, mirroring nextTrack: the engine keeps the played front of its
			// window, so a step back within it is gapless. The queue advances and previewInfo updates
			// SYNCHRONOUSLY (the engine's track-changed echo no-ops on the matching index), so a rapid
			// next/prev flurry always consults an up-to-date queue — never a pre-advance snapshot.
			// GOTCHA: the native engine's previous() applies its OWN 3s restart threshold (player.rs) —
			// past it, it seeks to 0 instead of stepping back, which would desync our optimistic step.
			// So the fast path is only taken comfortably UNDER that threshold (margin for the IPC gap);
			// past it, a threshold-skipping previous (swipe) takes the reload path, which is unconditional.
			if (useNative && nativeIndex > 0 && state.playbackState.position_ms < PREVIOUS_RESTART_THRESHOLD_MS - 1000) {
				const pick = playbackQueue.advancePrev()
				if (pick) {
					nativeIndex -= 1
					applyNativeTrackChange(nativeWindow[nativeIndex] ?? pick)
					await nativePreviewPlayer.previous()
					return
				}
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
		 * iOS, when the engine still holds an item ahead in the loaded window we advance the queue + state
		 * optimistically and let the engine switch natively (seamless, works locked); past the window we
		 * resolve + load fresh.
		 */
		async nextTrack() {
			const state = getState()
			if (!state.previewInfo) return

			// iOS in-window fast path: advance the queue and previewInfo SYNCHRONOUSLY, then tell the
			// engine. Previously the queue only advanced when the engine's track-changed event arrived,
			// so a quick previous (or a second swipe) in that gap consulted a stale queue and jumped to
			// the wrong track. The echo event no-ops (index already matches); if the engine had also
			// auto-advanced concurrently, the echo's index diff still reconciles the remainder.
			if (useNative && nativeIndex + 1 < nativeWindow.length) {
				const pick = playbackQueue.advanceNext()
				nativeIndex += 1
				const windowPick = nativeWindow[nativeIndex] ?? pick
				if (windowPick) applyNativeTrackChange(windowPick)
				await nativePreviewPlayer.next()
				scheduleNativeSlide()
				return
			}
			// Offline, uncached picks fail fast — skip past them (bounded) so auto-advance lands on the
			// next cached track instead of halting with an error per pick. The first failure toasts
			// ("not available offline"); skipped iterations stay silent (silentError).
			const MAX_OFFLINE_SKIPS = 30
			let pick = playbackQueue.advanceNext()
			let skips = 0
			while (pick) {
				const attempted = pick
				const ok = await this.playPreview(pick.release, pick.trackIndex, undefined, 0, {
					silentError: skips > 0,
				})
				if (ok || !isOffline() || ++skips >= MAX_OFFLINE_SKIPS) return
				console.warn('[offline] skipping uncached track', pick.release.id, pick.trackIndex)
				pick = playbackQueue.advanceNext()
				// A repeat loop can hand the failed pick straight back (repeat-release on a single-track
				// release, a one-release context) — retrying it here can't succeed, so stop.
				if (pick && pick.release.id === attempted.release.id && pick.trackIndex === attempted.trackIndex) return
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
		 * Set the repeat mode directly (the iOS lock-screen repeat command reports an absolute mode).
		 * Persisted device-locally like shuffle; affects preview playback only.
		 */
		setRepeatMode(mode: RepeatMode) {
			setRepeatModeInternal(mode)
		},

		/** Advance the repeat mode one step: off → track → release → context → off. */
		cycleRepeatMode() {
			const state = getState()
			const next = REPEAT_CYCLE[(REPEAT_CYCLE.indexOf(state.repeatMode) + 1) % REPEAT_CYCLE.length]
			setRepeatModeInternal(next)
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
		 * Register a handler called when a discovery preview starts playing. The discovery store wires
		 * this to `clearNew`, so a release stops being flagged "new" once it's been listened to.
		 */
		setPreviewPlayedHandler(handler: ((releaseId: string) => void) | null) {
			onPreviewPlayed = handler
		},

		/**
		 * Register a handler for iOS lock-screen Like presses (the engine has ALREADY toggled the DB
		 * natively; the handler only mirrors the change into in-memory stores). The discovery store
		 * wires this; same import-inversion rationale as setPreviewPlayedHandler.
		 */
		setNativeLikeChangedHandler(handler: ((trackId: string, isLiked: boolean) => void) | null) {
			onNativeLikeChanged = handler
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
			// Keep the iOS lock-screen glyph honest when the user likes from the in-app UI.
			// apply_liked is id-keyed and idempotent, so the echo from a lock-screen-initiated
			// toggle is harmless.
			if (useNative) void nativePreviewPlayer.setLiked(trackId, isLiked)
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
				// logPlay: false — re-anchoring the last session's track isn't a new listen for the log.
				playbackQueue.startSession(release, restoredPreviewTrackIndex, [release], { logPlay: false })
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
			// Reflect the persisted repeat mode on the engine up front: the item-loop flag for
			// repeat-track and the lock-screen repeat glyph both live natively, and the engine boots
			// knowing neither.
			syncNativeRepeatMode(getState().repeatMode)
			return nativePreviewPlayer.startNativePreviewBridge({
				onState: ({ isPlaying, positionMs, durationMs, isBuffering }) => {
					let applyPosition = true
					if (pendingNativeSeek) {
						const settled = Math.abs(positionMs - pendingNativeSeek.targetMs) <= NATIVE_SEEK_SETTLE_TOLERANCE_MS
						const timedOut = Date.now() - pendingNativeSeek.issuedAt >= NATIVE_SEEK_TIMEOUT_MS
						if (settled || timedOut) {
							pendingNativeSeek = null
						} else {
							applyPosition = false
						}
					}
					update((s) => {
						if (s.playbackSource !== 'preview') return s
						// `isBuffering` is the engine's honest "is there audio yet" signal — `isPlaying` alone
						// is set optimistically the moment a load starts. Mirrors the HTML5 path's
						// waiting/playing pair: raise the spinner while buffering (initial load OR a mid-track
						// stall), retire it the instant audio is actually rolling.
						//
						// These events always describe whatever `previewInfo` points at. Tapping a DIFFERENT
						// track sets `previewLoading` to that pending pick while `previewInfo` still names the
						// outgoing one — which keeps ticking twice a second all through the new track's stream
						// resolution — so a tick that doesn't match `previewInfo` must leave the pending
						// spinner strictly alone, or the outgoing track would clear it the moment it arrived.
						const current = s.previewInfo
						const pendingOther =
							s.previewLoading != null &&
							(current == null ||
								s.previewLoading.releaseId !== current.releaseId ||
								s.previewLoading.trackIndex !== current.trackIndex)
						return {
							...s,
							playbackState: {
								...s.playbackState,
								is_playing: isPlaying,
								position_ms: applyPosition ? positionMs : s.playbackState.position_ms,
								duration_ms: durationMs > 0 ? durationMs : s.playbackState.duration_ms,
							},
							previewLoading: pendingOther
								? s.previewLoading
								: isBuffering && current
									? { releaseId: current.releaseId, trackIndex: current.trackIndex }
									: null,
						}
					})
					if (applyPosition) persistPosition(positionMs)
				},
				onTrackChanged: (index, loadId) => {
					// Every engine event carries the id of the load it belongs to. An event from a SUPERSEDED
					// load (the previous session's auto-advance or load echo arriving interleaved with a new
					// load's) must never be reconciled into the queue — walking advanceNext/advancePrev against
					// the new session's freshly-seeded state corrupts what plays next.
					if (loadId !== previewLoadGen) return
					// The engine moved to entries-index `index`. Sync the queue module to match: each forward
					// step consumes one upcoming pick into history; a backward step (the lock-screen Previous,
					// which the native engine handles itself within the kept-played front of the window) walks
					// history back. A forward jump greater than one can happen if JS was suspended while the
					// engine auto-advanced through several pre-fed items. Then point previewInfo at the window
					// pick and, after a forward move, slide the window so there are always fresh items ahead.
					if (index === nativeIndex) return
					// A track change makes any in-flight seek target meaningless — drop the guard so the
					// new track's position ticks apply immediately.
					pendingNativeSeek = null
					const forward = index > nativeIndex
					if (forward) {
						for (let i = nativeIndex; i < index; i++) playbackQueue.advanceNext()
					} else {
						for (let i = nativeIndex; i > index; i--) playbackQueue.advancePrev()
					}
					nativeIndex = index
					const pick = nativeWindow[index]
					if (!pick) return
					applyNativeTrackChange(pick)
					if (forward) scheduleNativeSlide()
				},
				onEnded: (loadId) => {
					console.log('[native-preview] ENDED')
					// A stale ended (from a load that has since been replaced) must not stop playback or
					// auto-advance the new session's queue.
					if (loadId !== previewLoadGen) return
					// Repeat-track normally never gets here — the engine loops the ending item natively
					// (repeat-current flag) without emitting `ended`. This is the fallback for the race where
					// the mode flipped to `track` as the last window item ended: restart via a full reload.
					const endState = getState()
					if (endState.playbackSource === 'preview' && endState.previewInfo && endState.repeatMode === 'track') {
						void this.playPreview(endState.previewInfo.release, endState.previewInfo.trackIndex)
						return
					}
					update((s) => ({
						...s,
						playbackState: { ...s.playbackState, is_playing: false },
						previewLoading: null,
					}))
					// The engine reached the end of its loaded playlist — the single shuffle track, or the last
					// track of a sequential release. Hand off to the same auto-advance the HTML5 path uses
					// (wired in +layout) so playback continues across the queue / shuffle. With repeat off the
					// queue simply has nothing more to give and playback stays stopped here.
					onTrackEndCallback?.()
				},
				onError: async (message, retryable) => {
					console.error('[native-preview] engine error:', message)
					// A windowed item failed to load (e.g. an expired/stale upstream stream). Drop the cached
					// proxy URLs so the next feed re-resolves them fresh rather than re-handing the bad ones.
					streamUrlCache.clear()
					// Ignore duplicate error callbacks fired while a retry is in-flight.
					if (previewRetrying) return
					const state = getState()
					// One-shot auto-retry with fresh stream resolution, mirroring the HTML5 path's setOnError:
					// a cached stream URL can be dead before its recorded expiry (network change, CDN whim), so
					// invalidate the release's cached URLs and re-feed before surfacing an error to the user.
					// `previewRetryAttempted` is reset per track (applyNativeTrackChange / playPreview), so a
					// repeat failure on the same track falls through to the toast instead of looping.
					//
					// A NON-retryable failure is the engine's load timeout: the stream resolved fine, it just
					// never became audible, so re-resolving would only walk the same slow path again and
					// double the time the user stares at a spinner. Go straight to idle + toast.
					if (retryable && state.playbackSource === 'preview' && state.previewInfo && !previewRetryAttempted) {
						previewRetryAttempted = true
						previewRetrying = true
						const { release, trackIndex } = state.previewInfo
						const gen = ++previewLoadGen
						// A route change (AirPods removed, Bluetooth device off) can fault the item and land
						// here, so the retry must restore the state we were in rather than force playback on.
						const wasPlaying = state.playbackState.is_playing
						console.warn(`[native-preview] stream error, retrying with fresh resolution: ${message}`)
						try {
							await discoveryApi.invalidatePreviewStreamCache(release.id)
							await feedNativeWindow({ release, trackIndex }, 'reload', 0, gen)
							if (gen !== previewLoadGen) return // superseded — the newer transition owns audio + state
							await nativePreviewPlayer.setVolume(state.isMuted ? 0 : state.playbackState.volume)
							void nativePreviewPlayer.setRate(state.playbackState.speed)
							if (!wasPlaying) await nativePreviewPlayer.pause()
							update((s) => ({
								...s,
								error: null,
								playbackState: { ...s.playbackState, is_playing: wasPlaying, position_ms: 0 },
								// A resuming retry is still loading — its watchdog clears this once audio rolls.
								// A paused one never will, so retire the spinner now.
								previewLoading: wasPlaying ? { releaseId: release.id, trackIndex } : null,
							}))
							// The reload fed the current track alone; refill the upcoming window off-path.
							scheduleNativeSlide()
							return
						} catch (e) {
							if (gen !== previewLoadGen) return // superseded — a failure of a stale retry is noise
							console.error('[native-preview] retry after engine error failed:', e)
							// Fall through to show error
						} finally {
							previewRetrying = false
						}
					}
					update((s) => ({
						...s,
						error: message,
						playbackState: { ...s.playbackState, is_playing: false },
						previewLoading: null,
					}))
					// User-facing generic string (the raw AVPlayer error is kept in console + state.error for
					// diagnostics); mirrors the HTML5 preview path's error toast.
					toastPreviewError(get(translate)('errors.previewStreamFailed'))
				},
				onLikeChanged: (trackId, isLiked) => {
					onNativeLikeChanged?.(trackId, isLiked)
				},
				onRepeatChanged: (osMode) => {
					// The lock-screen repeat command reports MPRepeatType (off/one/all). "All" can't
					// distinguish release from context — context (the broader loop) is the sane reading.
					setRepeatModeInternal(osMode === 'one' ? 'track' : osMode === 'all' ? 'context' : 'off')
				},
				onDebug: (message) => {
					// Console only — a per-load/tick toast would bury real errors now that the mobile toast host
					// renders. Re-enable a toast here temporarily if deep-debugging the native engine.
					console.log('[native-preview][engine]', message)
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
			pendingNativeSeek = null
			previewLoadGen++
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
//
// The store ticks at 10Hz while playing (`startPositionTracking`), so every derived below
// re-evaluates 10×/sec. Primitive-valued stores are harmless — svelte skips notification for
// unchanged primitives — but OBJECT-valued selections re-notify every subscriber on every tick
// (objects always fail `safe_not_equal`), fanning out to every mounted feed row. Those are
// wrapped in `dedupe` (reference equality) so subscribers only run when the selection actually
// changed.
// =============================================================================

export const isPlaying = derived(playerStore, ($player) => $player.playbackState.is_playing)

export const currentTrack = dedupe(derived(playerStore, ($player) => $player.currentTrack))

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

export const repeatMode = derived(playerStore, ($player) => $player.repeatMode)

export const playbackSource = derived(playerStore, ($player) => $player.playbackSource)

export const previewInfo = dedupe(derived(playerStore, ($player) => $player.previewInfo))

// Primitive projection for the feed rows: a row only needs "is MY release the current preview",
// so it subscribes to the release id (auto-deduped as a string) instead of the `previewInfo`
// object — a track change within the same release doesn't touch the rows at all.
export const previewReleaseId = derived(playerStore, ($player) => $player.previewInfo?.releaseId ?? null)

export const previewTrackIndex = derived(playerStore, ($player) => $player.previewTrackIndex)

export const previewLoading = dedupe(derived(playerStore, ($player) => $player.previewLoading))

// Back-compat: release-level consumers (discovery rows, mini/expanded player) only need "is this release
// loading", so they keep subscribing to this. Returns a string|null, which dedupes cleanly.
export const previewLoadingReleaseId = derived(playerStore, ($player) => $player.previewLoading?.releaseId ?? null)

export const playbackSpeed = derived(playerStore, ($player) => $player.playbackState.speed)
