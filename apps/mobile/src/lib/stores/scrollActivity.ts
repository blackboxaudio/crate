import { writable } from 'svelte/store'

// App-wide "a feed is actively scrolling" signal, driven by ReleaseFeedList's per-frame scroll
// callback. Exists for the fixed glass chrome floating OVER the scrolling lists (the mini player):
// a backdrop-filter whose backdrop changes forces WKWebView to re-blur that region every scrolled
// frame, so such surfaces subscribe here and swap to their opaque `data-glass-suspend` state while
// the list is moving — the same trade Drawer makes while a sheet slides. The blur returns once
// scrolling settles.
const SETTLE_MS = 150

const scrolling = writable(false)
let settleTimer = 0

export const feedScrolling = { subscribe: scrolling.subscribe }

// Called at most once per frame (the callers are rAF-coalesced). The repeated `set(true)` is free:
// svelte stores skip notification when a primitive value is unchanged.
export function markFeedScrolling() {
	scrolling.set(true)
	clearTimeout(settleTimer)
	settleTimer = window.setTimeout(() => scrolling.set(false), SETTLE_MS)
}
