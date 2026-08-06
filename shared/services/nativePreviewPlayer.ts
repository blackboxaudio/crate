/**
 * iOS native preview playback client (#54).
 *
 * The iOS counterpart to `previewPlayer.ts`: instead of an HTML5 `<audio>` element, it drives a native
 * `AVPlayer` engine (Rust/objc2) via the `native_preview_*` Tauri commands. The engine owns the lock
 * screen (`MPRemoteCommandCenter` / `MPNowPlayingInfoCenter`), so prev/next/scrubber keep working
 * while the WebView's JavaScript is suspended on lock — which the HTML5 path can't do.
 *
 * Orchestrated by `playerStore`, which branches to this module on iOS (see `isIOS()`), pre-resolves
 * every track's proxy URL, and subscribes to the engine's events via `startNativePreviewBridge`.
 */

import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

/** One pre-resolved track handed to the native engine (matches the Rust `NativeTrackEntry`). */
export interface NativeTrack {
	url: string
	title: string
	artist: string
	album: string
	durationMs: number
	artworkUrl: string | null
	/**
	 * Explicit container MIME type, or null to let AVFoundation infer it. The proxy URL is
	 * extensionless, and AVPlayer can't infer YouTube/Discogs' `audio/mp4` from it (it fails to load
	 * silently), so those sources pass `'audio/mp4'`; Bandcamp/SoundCloud (`audio/mpeg`) are
	 * unambiguous and pass null.
	 */
	mimeType: string | null
	/**
	 * Identity + liked state for the lock-screen Like command: the engine toggles the DB natively
	 * while JS is suspended, so it needs the track's row id in hand. A null id disables Like for
	 * that entry.
	 */
	trackId: string | null
	isLiked: boolean
}

export interface NativeStateEvent {
	isPlaying: boolean
	positionMs: number
	durationMs: number
	/**
	 * We want to play but AVPlayer isn't rendering audio yet — the item is still loading, or playback
	 * stalled on an empty buffer. `isPlaying` alone can't tell you this: the engine sets it optimistically
	 * the moment a load is requested. This is the native equivalent of the HTML5 element's
	 * `waiting` / `playing` pair, and it's what drives the loading spinner.
	 */
	isBuffering: boolean
}

/** OS-level repeat state as the lock screen reports/displays it (MPRepeatType). */
export type NativeRepeatMode = 'off' | 'one' | 'all'

export interface NativeBridgeHandlers {
	onState: (state: NativeStateEvent) => void
	onTrackChanged: (index: number) => void
	onEnded: () => void
	/**
	 * `retryable` says whether re-resolving the stream could plausibly fix the failure. An AVFoundation
	 * load error usually means a dead/expired upstream URL, which one silent re-resolve does fix; a load
	 * TIMEOUT would just walk the same slow path again, so the caller should go straight to idle + toast.
	 */
	onError: (message: string, retryable: boolean) => void
	/** A lock-screen Like press toggled the DB natively; mirror it into the JS stores. */
	onLikeChanged?: (trackId: string, isLiked: boolean) => void
	/** The lock-screen repeat command changed the OS repeat state; mirror it into the app. */
	onRepeatChanged?: (mode: NativeRepeatMode) => void
	/** Temporary diagnostic channel (#54 debugging): engine traces routed to the webview console. */
	onDebug?: (message: string) => void
}

/**
 * Load `tracks` and start playing from `startIndex`, beginning `startPositionMs` into that track
 * (0 = from the start). A non-zero offset is used when restoring the last session on relaunch so the
 * engine begins at the saved position instead of playing from the start and then seeking back.
 */
export async function play(tracks: NativeTrack[], startIndex: number, startPositionMs = 0): Promise<void> {
	await invoke('native_preview_play', { tracks, startIndex, startPositionMs })
}

/**
 * Replace the engine's UPCOMING tail (everything after the currently-playing item) without disturbing
 * the current track, its position, or the lock screen. Used to slide the lazy window forward as playback
 * advances and to apply Add-to-queue / Play-next mutations live — including while the screen is locked.
 */
export async function setUpcoming(tracks: NativeTrack[]): Promise<void> {
	await invoke('native_preview_set_upcoming', { tracks })
}

export async function pause(): Promise<void> {
	await invoke('native_preview_pause')
}

export async function resume(): Promise<void> {
	await invoke('native_preview_resume')
}

export async function seek(positionMs: number): Promise<void> {
	await invoke('native_preview_seek', { positionMs })
}

export async function next(): Promise<void> {
	await invoke('native_preview_next')
}

export async function previous(): Promise<void> {
	await invoke('native_preview_previous')
}

export async function stop(): Promise<void> {
	await invoke('native_preview_stop')
}

export async function setVolume(volume: number): Promise<void> {
	await invoke('native_preview_set_volume', { volume })
}

export async function setRate(rate: number): Promise<void> {
	await invoke('native_preview_set_rate', { rate })
}

/** Reflect an in-app like toggle on the native engine (window entries + lock-screen glyph). */
export async function setLiked(trackId: string, liked: boolean): Promise<void> {
	await invoke('native_preview_set_liked', { trackId, liked })
}

/**
 * Reflect the app's repeat mode on the engine. Repeat-track turns on the item-loop flag — the engine
 * rewinds the ending item instead of advancing, so the loop is gapless and works while the screen is
 * locked — and the lock-screen repeat glyph shows one/all/off (release and context both display "all",
 * MPRepeatType has no finer notion).
 */
export async function setRepeatMode(mode: 'off' | 'track' | 'release' | 'context'): Promise<void> {
	await invoke('native_preview_set_repeat_mode', { mode })
}

/**
 * Subscribe to the native engine's events and forward them to the provided handlers. Returns a
 * cleanup function that detaches all listeners.
 */
export async function startNativePreviewBridge(handlers: NativeBridgeHandlers): Promise<() => void> {
	const unlisten: UnlistenFn[] = []
	unlisten.push(await listen<NativeStateEvent>('native-preview-state', (e) => handlers.onState(e.payload)))
	unlisten.push(
		await listen<{ index: number }>('native-preview-track-changed', (e) => handlers.onTrackChanged(e.payload.index))
	)
	unlisten.push(await listen('native-preview-ended', () => handlers.onEnded()))
	unlisten.push(
		await listen<{ message: string; retryable: boolean }>('native-preview-error', (e) =>
			handlers.onError(e.payload.message, e.payload.retryable)
		)
	)
	if (handlers.onLikeChanged) {
		unlisten.push(
			await listen<{ trackId: string; isLiked: boolean }>('native-preview-like-changed', (e) =>
				handlers.onLikeChanged!(e.payload.trackId, e.payload.isLiked)
			)
		)
	}
	if (handlers.onRepeatChanged) {
		unlisten.push(
			await listen<{ mode: NativeRepeatMode }>('native-preview-repeat-changed', (e) =>
				handlers.onRepeatChanged!(e.payload.mode)
			)
		)
	}
	if (handlers.onDebug) {
		unlisten.push(
			await listen<{ message: string }>('native-preview-debug', (e) => handlers.onDebug!(e.payload.message))
		)
	}
	return () => {
		for (const u of unlisten) u()
	}
}
