import type { Action } from 'svelte/action'
import { DRAG_THRESHOLD } from '$shared/utils/drag'
import { rigidTap } from '$lib/utils/haptics'

/**
 * Shared long-press gesture for opening a row/tile ContextMenu — the timer pattern previously
 * hand-rolled per view (grid tiles, playlist/tag/follow rows). No gesture library; same
 * pointer-event conventions as `swipe.ts` / `swipeVertical.ts`.
 *
 * Hold for 450 ms without moving past the drag threshold and it fires: haptic, then `onLongPress`
 * with the node's viewport rect (the anchor for ContextMenu's lifted preview). Movement, release, or
 * cancel before that disarms it. A stationary long-press on a real `<button>` also synthesizes a
 * click on release — the action swallows that one click (capture-phase on the node) so opening the
 * menu doesn't also activate the row.
 *
 * Apply to the interactive element itself or a wrapper around it; the swallow relies on the click
 * targeting a descendant of the node (which a wrapper guarantees).
 */

export interface LongPressRect {
	top: number
	left: number
	width: number
	height: number
}

export interface LongPressOptions {
	onLongPress: (rect: LongPressRect) => void
	/** Disable the gesture (e.g. in select mode). Default true. */
	enabled?: boolean
}

const LONG_PRESS_MS = 450

export const longPress: Action<HTMLElement, LongPressOptions> = (node, initial) => {
	let opts = initial

	let timer = 0
	let startX = 0
	let startY = 0
	// Latched when the press fires, so the click synthesized on release can be swallowed. Reset on the
	// next pointerdown as well, in case the click never materializes (finger released off the node).
	let suppressNextClick = false

	function clear() {
		if (timer) {
			clearTimeout(timer)
			timer = 0
		}
	}

	function detach() {
		window.removeEventListener('pointermove', onMove)
		window.removeEventListener('pointerup', onEnd)
		window.removeEventListener('pointercancel', onEnd)
	}

	function onMove(e: PointerEvent) {
		if (Math.abs(e.clientX - startX) < DRAG_THRESHOLD && Math.abs(e.clientY - startY) < DRAG_THRESHOLD) return
		clear()
		detach()
	}

	function onEnd() {
		clear()
		detach()
	}

	function onPointerDown(e: PointerEvent) {
		suppressNextClick = false
		if (opts.enabled === false) return
		if (e.pointerType === 'mouse' && e.button !== 0) return
		startX = e.clientX
		startY = e.clientY
		clear()
		timer = window.setTimeout(() => {
			timer = 0
			suppressNextClick = true
			void rigidTap()
			const r = node.getBoundingClientRect()
			opts.onLongPress({ top: r.top, left: r.left, width: r.width, height: r.height })
		}, LONG_PRESS_MS)
		window.addEventListener('pointermove', onMove, { passive: true })
		window.addEventListener('pointerup', onEnd)
		window.addEventListener('pointercancel', onEnd)
	}

	function onClickCapture(e: MouseEvent) {
		if (!suppressNextClick) return
		suppressNextClick = false
		e.preventDefault()
		e.stopPropagation()
	}

	node.addEventListener('pointerdown', onPointerDown)
	node.addEventListener('click', onClickCapture, true)

	return {
		update(next: LongPressOptions) {
			opts = next
		},
		destroy() {
			node.removeEventListener('pointerdown', onPointerDown)
			node.removeEventListener('click', onClickCapture, true)
			clear()
			detach()
		},
	}
}
