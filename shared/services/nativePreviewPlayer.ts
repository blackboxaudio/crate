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
}

export interface NativeStateEvent {
	isPlaying: boolean
	positionMs: number
	durationMs: number
}

export interface NativeBridgeHandlers {
	onState: (state: NativeStateEvent) => void
	onTrackChanged: (index: number) => void
	onEnded: () => void
	onError: (message: string) => void
}

export async function play(tracks: NativeTrack[], startIndex: number): Promise<void> {
	await invoke('native_preview_play', { tracks, startIndex })
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
	unlisten.push(await listen<{ message: string }>('native-preview-error', (e) => handlers.onError(e.payload.message)))
	return () => {
		for (const u of unlisten) u()
	}
}
