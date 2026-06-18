import type { Action } from 'svelte/action'
import { DRAG_THRESHOLD } from '$shared/utils/drag'

/**
 * Reusable pointer-based vertical swipe action for the preview player — the vertical sibling of
 * `swipe.ts` (which handles the horizontal drawer gestures). No gesture library; same pointer-event
 * conventions and axis-lock approach.
 *
 * The mini-player uses `onSwipeUp` to expand; the expanded player uses `onProgress` (finger-follow
 * translate) + `onSwipeDown` to dismiss. The gesture is only claimed once vertical intent exceeds
 * horizontal, so a horizontal drawer swipe started on the bar is never hijacked, and a stationary tap
 * (no movement past the threshold) is never claimed — letting child button `onclick`s fire normally.
 */

export interface SwipeVerticalOptions {
	/** Released committing to an upward swipe (distance or flick). */
	onSwipeUp?: () => void
	/** Released committing to a downward swipe (distance or flick). */
	onSwipeDown?: () => void
	/** Live vertical offset in px during a claimed drag (negative = up, positive = down); 0 on release. */
	onProgress?: (dy: number) => void
	/** Distance in px that commits the gesture on release. Default 56. */
	threshold?: number
	/** Disable the gesture. Default true. */
	enabled?: boolean
}

interface ResolvedOptions {
	onSwipeUp?: () => void
	onSwipeDown?: () => void
	onProgress?: (dy: number) => void
	threshold: number
	enabled: boolean
}

const FLICK_VELOCITY = 0.4 // px/ms — above this, direction of travel wins regardless of distance

function resolve(opts: SwipeVerticalOptions): ResolvedOptions {
	return {
		onSwipeUp: opts.onSwipeUp,
		onSwipeDown: opts.onSwipeDown,
		onProgress: opts.onProgress,
		threshold: opts.threshold ?? 56,
		enabled: opts.enabled ?? true,
	}
}

export const swipeVertical: Action<HTMLElement, SwipeVerticalOptions> = (node, initial) => {
	let opts = resolve(initial)

	let pointerId: number | null = null
	let startX = 0
	let startY = 0
	let lastY = 0
	let lastT = 0
	let velocity = 0
	let claimed = false
	let abandoned = false

	function onPointerDown(e: PointerEvent) {
		if (!opts.enabled || pointerId !== null) return
		if (e.pointerType === 'mouse' && e.button !== 0) return

		pointerId = e.pointerId
		startX = e.clientX
		startY = lastY = e.clientY
		lastT = e.timeStamp
		velocity = 0
		claimed = false
		abandoned = false

		window.addEventListener('pointermove', onPointerMove, { passive: false })
		window.addEventListener('pointerup', onPointerUp)
		window.addEventListener('pointercancel', onPointerUp)
	}

	function onPointerMove(e: PointerEvent) {
		if (e.pointerId !== pointerId || abandoned) return

		const dx = e.clientX - startX
		const dy = e.clientY - startY

		if (!claimed) {
			if (Math.abs(dx) < DRAG_THRESHOLD && Math.abs(dy) < DRAG_THRESHOLD) return

			// Horizontal intent → release to the drawer gesture / underlying content.
			if (Math.abs(dx) > Math.abs(dy)) {
				abandoned = true
				teardownWindow()
				pointerId = null
				return
			}

			claimed = true
		}

		if (e.cancelable) e.preventDefault()

		const now = e.timeStamp
		if (now > lastT) velocity = (e.clientY - lastY) / (now - lastT)
		lastY = e.clientY
		lastT = now

		opts.onProgress?.(dy)
	}

	function onPointerUp(e: PointerEvent) {
		if (e.pointerId !== pointerId) return

		const dy = e.clientY - startY
		const wasClaimed = claimed

		teardownWindow()
		pointerId = null

		if (!wasClaimed) return

		opts.onProgress?.(0)

		let direction: 'up' | 'down' | null = null
		if (Math.abs(velocity) > FLICK_VELOCITY) {
			direction = velocity < 0 ? 'up' : 'down'
		} else if (Math.abs(dy) > opts.threshold) {
			direction = dy < 0 ? 'up' : 'down'
		}

		if (direction === 'up') opts.onSwipeUp?.()
		else if (direction === 'down') opts.onSwipeDown?.()
	}

	function teardownWindow() {
		window.removeEventListener('pointermove', onPointerMove)
		window.removeEventListener('pointerup', onPointerUp)
		window.removeEventListener('pointercancel', onPointerUp)
	}

	function applyTouchAction() {
		// Let the browser keep horizontal panning; we own vertical movement on this node.
		node.style.touchAction = opts.enabled ? 'pan-x' : ''
	}

	node.addEventListener('pointerdown', onPointerDown)
	applyTouchAction()

	return {
		update(next: SwipeVerticalOptions) {
			opts = resolve(next)
			applyTouchAction()
		},
		destroy() {
			node.removeEventListener('pointerdown', onPointerDown)
			teardownWindow()
		},
	}
}
