import { get } from 'svelte/store'
import { mobileUIStore } from '$lib/stores/mobileUI'

// Android hardware/gesture Back handling (#62). MainActivity's OnBackPressedCallback evaluates
// `window.__CRATE_BACK__()` synchronously per press: `true` means the frontend consumed the press,
// anything else (including a JS error) makes Kotlin call `moveTaskToBack` — fail-safe, the user is
// never trapped. Svelte store updates are synchronous, so the whole decision fits in one call.
//
// Layered surfaces (drawers, sheets, context menus, prompt dialogs) self-register while open via
// `registerBackLayer` — open order IS stacking order, so the last registration is the topmost
// surface and Back closes exactly one layer per press through the surface's own close path (same
// animations/choreography as a swipe or scrim tap). The store fallbacks below the stack cover
// non-layered state (swipe-open row, multi-select, folder trail, non-home tab). The system handles
// what never reaches us: an open keyboard and native AlertDialogs consume Back themselves.

type CloseLayer = () => void

const layers: CloseLayer[] = []

/**
 * Register an open dismissable surface. Returns the unregister function — call it the moment a
 * close *starts* (not when it finishes), so a rapid second Back falls through to the next layer
 * instead of re-closing an already-animating one.
 */
export function registerBackLayer(close: CloseLayer): () => void {
	layers.push(close)
	return () => {
		const i = layers.lastIndexOf(close)
		if (i !== -1) layers.splice(i, 1)
	}
}

/** Handle one Back press. Returns true when consumed, false to let Android background the app. */
export function handleBack(): boolean {
	// 1. Topmost open surface (drawer / sheet / context menu / prompt) closes first.
	const top = layers[layers.length - 1]
	if (top) {
		top()
		return true
	}

	const s = get(mobileUIStore)

	// 2. A swipe-revealed delete row snaps shut.
	if (s.openRowId !== null) {
		mobileUIStore.setOpenRow(null)
		return true
	}

	// 3. Multi-select exits (mirrors activateTab's re-tap pop order).
	if (s.selectMode) {
		mobileUIStore.exitSelectMode()
		return true
	}

	// 4. The Playlists folder drill-down backs out one level.
	if (s.activeTab === 'playlists' && s.playlistFolderTrail.length > 0) {
		mobileUIStore.popPlaylistFolder()
		return true
	}

	// 5. Any non-home tab returns to Discovery (standard Android back-to-home-tab behavior).
	if (s.activeTab !== 'discovery') {
		mobileUIStore.setTab('discovery')
		return true
	}

	// 6. Nothing left — Android backgrounds the app (state survives; playback keeps running).
	return false
}

declare global {
	interface Window {
		__CRATE_BACK__?: () => boolean
	}
}

/** Install the bridge entry point. Returns a cleanup function. Call only on Android. */
export function initAndroidBack(): () => void {
	window.__CRATE_BACK__ = handleBack
	return () => {
		delete window.__CRATE_BACK__
	}
}
