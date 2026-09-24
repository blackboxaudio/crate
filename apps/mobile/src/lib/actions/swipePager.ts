import type { Action } from 'svelte/action'
import { DRAG_THRESHOLD } from '$shared/utils/drag'

/**
 * Horizontal pointer-based paging swipe for the expanded player's cover strip — the bidirectional
 * (±dx) sibling of `swipe.ts`/`swipeVertical.ts`. Same pointer-event conventions and axis-lock
 * approach: the gesture is only claimed once horizontal intent exceeds vertical, so the player
 * sheet's drag-to-dismiss (which claims the complementary case, ties included) is never hijacked,
 * and a stationary tap is never claimed — child taps still fire normally.
 *
 * Unlike the drawer swipes this doesn't model 0→1 openness: it reports a live signed `dx`
 * (finger-follow), rubber-bands when paging in a direction isn't possible, and on release commits
 * to a page (`dir: 1` = next, leftward drag; `-1` = previous) by distance or flick, else cancels.
 */

export interface SwipePagerOptions {
	/** Claimed drag began (past the axis-locked threshold). Snapshot pager state here. */
	onDragStart?: () => void
	/** Live signed horizontal offset in px during a claimed drag; edge resistance already applied. */
	onDrag?: (dx: number) => void
	/** Released committing to a page: `dir` 1 = next (dragged left), -1 = previous (dragged right). */
	onCommit?: (dir: 1 | -1, releaseDx: number, velocity: number) => void
	/** Released without committing (didn't cross the threshold, or paging that way isn't possible). */
	onCancel?: () => void
	/** Whether a page exists in a direction. False → that direction rubber-bands and never commits. */
	canPage?: (dir: 1 | -1) => boolean
	/** Page width in px used for the commit fraction and resistance curve. Defaults to node width. */
	width?: () => number
	/** Fraction of the width that commits on release. Default 0.35. */
	commitFraction?: number
	/** Disable the gesture. Default true. */
	enabled?: boolean
}

const FLICK_VELOCITY = 0.4 // px/ms — above this, direction of travel wins regardless of distance

// Asymptotic rubber-band for a blocked direction: approaches (but never reaches) half a page of
// travel, so the card visibly resists instead of following the finger.
function resist(dx: number, width: number): number {
	return width * (1 - 1 / (1 + Math.abs(dx) / (width * 2))) * Math.sign(dx)
}

export const swipePager: Action<HTMLElement, SwipePagerOptions> = (node, initial) => {
	let opts = initial ?? {}

	let pointerId: number | null = null
	let startX = 0
	let startY = 0
	let lastX = 0
	let lastT = 0
	let velocity = 0
	let claimed = false
	let abandoned = false
	// Coalesce per-move drag reports to one callback per frame (see swipe.ts) — `teardownWindow`
	// drops any pending flush before `onCommit`/`onCancel`, so a stale dx can't land mid-settle.
	let dragRaf = 0
	let pendingDx: number | null = null

	function flushDrag() {
		dragRaf = 0
		if (pendingDx === null) return
		const v = pendingDx
		pendingDx = null
		opts.onDrag?.(v)
	}

	function pageWidth(): number {
		return opts.width?.() ?? node.clientWidth ?? 0
	}

	function reportDx(rawDx: number): number {
		const dir: 1 | -1 = rawDx < 0 ? 1 : -1
		const blocked = opts.canPage ? !opts.canPage(dir) : false
		return blocked ? resist(rawDx, Math.max(1, pageWidth())) : rawDx
	}

	function onPointerDown(e: PointerEvent) {
		if (opts.enabled === false || pointerId !== null) return
		if (e.pointerType === 'mouse' && e.button !== 0) return

		pointerId = e.pointerId
		startX = lastX = e.clientX
		startY = e.clientY
		lastT = e.timeStamp
		velocity = 0
		claimed = false
		abandoned = false

		window.addEventListener('pointermove', onPointerMove, { passive: false })
		window.addEventListener('pointerup', onPointerUp)
		window.addEventListener('pointercancel', onPointerCancel)
	}

	function onPointerMove(e: PointerEvent) {
		if (e.pointerId !== pointerId || abandoned) return

		const dx = e.clientX - startX
		const dy = e.clientY - startY

		if (!claimed) {
			if (Math.abs(dx) < DRAG_THRESHOLD && Math.abs(dy) < DRAG_THRESHOLD) return

			// Vertical intent (ties included) → release to the sheet's drag-to-dismiss.
			if (Math.abs(dy) >= Math.abs(dx)) {
				abandoned = true
				teardownWindow()
				pointerId = null
				return
			}

			claimed = true
			opts.onDragStart?.()
		}

		if (e.cancelable) e.preventDefault()

		const now = e.timeStamp
		if (now > lastT) velocity = (e.clientX - lastX) / (now - lastT)
		lastX = e.clientX
		lastT = now

		pendingDx = reportDx(dx)
		if (!dragRaf) dragRaf = requestAnimationFrame(flushDrag)
	}

	function onPointerUp(e: PointerEvent) {
		if (e.pointerId !== pointerId) return

		const dx = e.clientX - startX
		const wasClaimed = claimed

		teardownWindow()
		pointerId = null

		if (!wasClaimed) return

		const dir: 1 | -1 = dx < 0 ? 1 : -1
		const width = Math.max(1, pageWidth())
		const pageable = opts.canPage ? opts.canPage(dir) : true
		const byDistance = Math.abs(dx) > (opts.commitFraction ?? 0.35) * width
		// A flick only commits when it travels the same way as the drag (no reverse-flick commits).
		const byFlick = Math.abs(velocity) > FLICK_VELOCITY && Math.sign(velocity) === Math.sign(dx)

		if (pageable && dx !== 0 && (byDistance || byFlick)) opts.onCommit?.(dir, dx, velocity)
		else opts.onCancel?.()
	}

	function onPointerCancel(e: PointerEvent) {
		if (e.pointerId !== pointerId) return
		const wasClaimed = claimed
		teardownWindow()
		pointerId = null
		if (wasClaimed) opts.onCancel?.()
	}

	function teardownWindow() {
		window.removeEventListener('pointermove', onPointerMove)
		window.removeEventListener('pointerup', onPointerUp)
		window.removeEventListener('pointercancel', onPointerCancel)
		if (dragRaf) cancelAnimationFrame(dragRaf)
		dragRaf = 0
		pendingDx = null
	}

	function applyTouchAction() {
		// The strip contains no scrollable content, so claim all native panning: vertical drags stay
		// cancelable pointer events for the sheet's window-level dismiss handler (JS axis-lock
		// arbitrates between the two), and horizontal ones are ours.
		node.style.touchAction = opts.enabled === false ? '' : 'none'
	}

	node.addEventListener('pointerdown', onPointerDown)
	applyTouchAction()

	return {
		update(next: SwipePagerOptions) {
			opts = next ?? {}
			applyTouchAction()
		},
		destroy() {
			node.removeEventListener('pointerdown', onPointerDown)
			teardownWindow()
		},
	}
}
