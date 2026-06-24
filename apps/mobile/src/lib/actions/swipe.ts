import type { Action } from 'svelte/action'
import { DRAG_THRESHOLD } from '$shared/utils/drag'

/**
 * Reusable pointer-based swipe action for the mobile drawers — no gesture library, matching the
 * codebase's existing pointer-event conventions (see PlaylistItem.svelte / shared/utils/drag.ts).
 *
 * It supports two modes:
 *  - `mode: 'open'`  — an edge gesture: the drag must start within `edgeSize` px of the screen edge
 *                      and moves the drawer from closed (0) toward open (1).
 *  - `mode: 'close'` — a panel gesture: the drag starts anywhere on the node and moves the drawer
 *                      from open (1) toward closed (0).
 *
 * During a claimed drag it emits `onProgress(openness)` (0 = closed, 1 = open) for finger-follow, and
 * on release resolves to `onOpen()` / `onClose()` using a distance threshold or a velocity flick.
 *
 * Axis-lock: the gesture is only claimed once horizontal intent exceeds vertical, so vertical
 * scrolling of the underlying list is never hijacked. `touch-action: pan-y` is set on the node so the
 * browser keeps handling vertical scroll while reserving horizontal movement for this action.
 */

export type SwipeSide = 'left' | 'right'

export interface SwipeOptions {
	/** Which side the drawer is anchored to. */
	side: SwipeSide
	/** 'open' = edge-to-open gesture; 'close' = drag-the-panel-closed gesture. */
	mode: 'open' | 'close'
	/** Live openness 0→1 during a drag (translate the drawer to follow the finger). */
	onProgress?: (openness: number) => void
	/** Gesture released committing to open. */
	onOpen?: () => void
	/** Gesture released committing to closed. */
	onClose?: () => void
	/** Drawer width in px; defaults to the node's measured width (falls back to viewport width). */
	width?: number
	/** Hit zone from the screen edge for `mode: 'open'`, in px. Default 24. */
	edgeSize?: number
	/** For `mode: 'close'`, restrict the drag to start within this many px of an edge (iOS-style
	 *  interactive back-swipe). When unset, a close drag can start anywhere on the node. */
	closeEdgeSize?: number
	/** Which screen edge `closeEdgeSize` is measured from (defaults to `side`). A panel that slides
	 *  off to the right (`side: 'right'`) but is dismissed by an edge-swipe from the LEFT sets
	 *  `closeEdgeFrom: 'left'`. */
	closeEdgeFrom?: SwipeSide
	/** Fraction of width that commits the gesture on release. Default 0.4. */
	snapFraction?: number
	/** Disable the gesture (e.g. when the drawer isn't in the relevant open/closed state). Default true. */
	enabled?: boolean
}

interface ResolvedOptions {
	side: SwipeSide
	mode: 'open' | 'close'
	onProgress?: (openness: number) => void
	onOpen?: () => void
	onClose?: () => void
	width?: number
	edgeSize: number
	closeEdgeSize?: number
	closeEdgeFrom?: SwipeSide
	snapFraction: number
	enabled: boolean
}

const FLICK_VELOCITY = 0.4 // px/ms — above this, direction of travel wins regardless of distance

function resolve(opts: SwipeOptions): ResolvedOptions {
	return {
		side: opts.side,
		mode: opts.mode,
		onProgress: opts.onProgress,
		onOpen: opts.onOpen,
		onClose: opts.onClose,
		width: opts.width,
		edgeSize: opts.edgeSize ?? 24,
		closeEdgeSize: opts.closeEdgeSize,
		closeEdgeFrom: opts.closeEdgeFrom,
		snapFraction: opts.snapFraction ?? 0.4,
		enabled: opts.enabled ?? true,
	}
}

function clamp(value: number, min: number, max: number): number {
	return Math.min(max, Math.max(min, value))
}

export const swipe: Action<HTMLElement, SwipeOptions> = (node, initial) => {
	let opts = resolve(initial)

	let pointerId: number | null = null
	let startX = 0
	let startY = 0
	let lastX = 0
	let lastT = 0
	let velocity = 0
	let claimed = false
	let abandoned = false

	// Sign of the "opening" direction along x: left drawer opens with +dx, right drawer with -dx.
	const dir = () => (opts.side === 'left' ? 1 : -1)
	// Openness at the start of the gesture: opening starts closed (0), closing starts open (1).
	const base = () => (opts.mode === 'open' ? 0 : 1)
	const width = () => {
		if (opts.width) return opts.width
		return node.getBoundingClientRect().width || window.innerWidth
	}

	function opennessFor(dx: number): number {
		return clamp(base() + (dir() * dx) / width(), 0, 1)
	}

	function onPointerDown(e: PointerEvent) {
		if (!opts.enabled || pointerId !== null) return
		if (e.pointerType === 'mouse' && e.button !== 0) return

		// Edge gate: 'open' always starts from the screen edge; 'close' is edge-gated only when
		// `closeEdgeSize` is set (iOS-style back-swipe). The edge defaults to the anchor `side` but can
		// be overridden via `closeEdgeFrom` (e.g. a right-exiting panel dismissed from the left edge).
		const edgeLimit = opts.mode === 'open' ? opts.edgeSize : opts.closeEdgeSize
		if (edgeLimit != null) {
			const edgeSide = opts.mode === 'open' ? opts.side : (opts.closeEdgeFrom ?? opts.side)
			const fromEdge = edgeSide === 'left' ? e.clientX : window.innerWidth - e.clientX
			if (fromEdge > edgeLimit) return
		}

		pointerId = e.pointerId
		startX = lastX = e.clientX
		startY = e.clientY
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

			// Vertical intent → release to the scroll container.
			if (Math.abs(dy) > Math.abs(dx)) {
				abandoned = true
				teardownWindow()
				pointerId = null
				return
			}

			// Horizontal, but in the wrong direction (e.g. trying to "open" past fully-open) → ignore.
			const wantsForward = opts.mode === 'open' ? dir() * dx > 0 : dir() * dx < 0
			if (!wantsForward) {
				abandoned = true
				teardownWindow()
				pointerId = null
				return
			}

			claimed = true
		}

		if (e.cancelable) e.preventDefault()

		const now = e.timeStamp
		if (now > lastT) velocity = (e.clientX - lastX) / (now - lastT)
		lastX = e.clientX
		lastT = now

		opts.onProgress?.(opennessFor(dx))
	}

	function onPointerUp(e: PointerEvent) {
		if (e.pointerId !== pointerId) return

		const dx = e.clientX - startX
		const openness = opennessFor(dx)
		const flick = dir() * velocity // > 0 means travelling in the opening direction
		const wasClaimed = claimed

		teardownWindow()
		pointerId = null

		if (!wasClaimed) return

		let open: boolean
		if (Math.abs(flick) > FLICK_VELOCITY) {
			open = flick > 0
		} else if (opts.mode === 'open') {
			open = openness >= opts.snapFraction
		} else {
			open = openness > 1 - opts.snapFraction
		}

		if (open) opts.onOpen?.()
		else opts.onClose?.()
	}

	function teardownWindow() {
		window.removeEventListener('pointermove', onPointerMove)
		window.removeEventListener('pointerup', onPointerUp)
		window.removeEventListener('pointercancel', onPointerUp)
	}

	function applyTouchAction() {
		node.style.touchAction = opts.enabled ? 'pan-y' : ''
	}

	node.addEventListener('pointerdown', onPointerDown)
	applyTouchAction()

	return {
		update(next: SwipeOptions) {
			opts = resolve(next)
			applyTouchAction()
		},
		destroy() {
			node.removeEventListener('pointerdown', onPointerDown)
			teardownWindow()
		},
	}
}
