<script lang="ts">
	import type { Snippet } from 'svelte'
	import type { Action } from 'svelte/action'
	import { onMount } from 'svelte'
	import { translate } from '$shared/i18n'
	import { swipe, type SwipeOptions } from '$lib/actions/swipe'
	import { swipeVertical, type SwipeVerticalOptions } from '$lib/actions/swipeVertical'

	// The single baseline behind every mobile drawer-like surface (nav drawers, the release-detail push,
	// the bottom-sheet modal, the expanded player). It owns the one shared motion "feel": slide-in on open,
	// finger-follow drag-to-dismiss, snap-back, slide-out, the dimming scrim, and reduced-motion — all
	// parameterized by `direction`. Consumers supply only their content + chrome (`class`) and a few props.
	//
	// Controlled by `open`: the parent flips it; the component animates. A user dismissal (scrim tap, drag
	// commit, Esc) calls `onClose` (so the parent can flip `open` false); once the slide-out finishes,
	// `onClosed` fires (so a parent that mounts this via `{#if}` can clear its store only after the anim).
	type Direction = 'left' | 'right' | 'top' | 'bottom'
	type Props = {
		open: boolean
		direction: Direction
		onClose: () => void
		onClosed?: () => void
		/** Receives live drawer state + the dismiss-drag action (apply `use:drag` to a handle to confine it).
		 *  `animating` is true while the panel slides — switch a scroll container to overflow-hidden then, so
		 *  its content can't scroll mid-transition while the panel itself stays grabbable / finger-followable. */
		children: Snippet<[{ openness: number; dragging: boolean; drag: Action<HTMLElement>; animating: boolean }]>

		/** Panel chrome: bg / border / width|height / max-h / rounding / safe-area. Position + z come from here. */
		class?: string
		ariaLabel: string
		/** Panel z-index (inline style; avoids dynamic-class purge). */
		z?: number
		/** Scrim z-index; defaults to `z`. Nav drawers drop it below the mini-player. */
		scrimZ?: number
		scrim?: boolean
		scrimOpacity?: number
		/** Whether tapping the scrim dismisses. False for fullscreen surfaces whose scrim only shows mid-slide. */
		scrimDismiss?: boolean
		/** External finger-follow OPEN progress 0→1 (the shell's edge-open gesture). Horizontal only. */
		openProgress?: number | null
		/** Apply the dismiss gesture to the whole panel (default). A bottom sheet with scrollable content sets
		 *  this false and confines the drag to a handle via the exposed `drag` action — otherwise the panel's
		 *  `pan-x` touch-action would block the content's vertical scroll. */
		panelDrag?: boolean
		/** iOS-style back-swipe: restrict the close drag to start within this many px of an edge. Horizontal only. */
		closeEdgeSize?: number
		closeEdgeFrom?: 'left' | 'right'
	}
	let {
		open,
		direction,
		onClose,
		onClosed,
		children,
		class: className = '',
		ariaLabel,
		z = 40,
		scrimZ,
		scrim = true,
		scrimOpacity = 0.5,
		scrimDismiss = true,
		openProgress = null,
		panelDrag = true,
		closeEdgeSize,
		closeEdgeFrom,
	}: Props = $props()

	const DURATION = 500 // ms — the one shared slide duration; keep in sync with the `duration-500` class below
	const horizontal = $derived(direction === 'left' || direction === 'right')
	const effectiveScrimZ = $derived(scrimZ ?? z)
	const clampUnit = (v: number) => Math.min(1, Math.max(0, v))

	// Lifecycle: `visible` keeps the panel mounted, `entered` slides it into place, `closing` slides it out.
	let visible = $state(false)
	let entered = $state(false)
	let closing = $state(false)
	// Live 0→1 openness while a dismiss drag is in progress (else null). The `swipe` action reports openness
	// directly; the vertical drag is converted from px below.
	let closeDrag = $state<number | null>(null)
	let panelH = $state(0)
	// True while the panel's slide is mid-flight; exposed to content as `animating` so a scroll container can
	// stop scrolling during the transition (the panel itself stays grabbable / finger-followable).
	let animatingSlide = $state(false)

	// Openness 0 (off-screen) → 1 (open). A drag wins; then the external open-drag (horizontal); else the
	// committed open/closing state.
	const openness = $derived(closeDrag ?? (horizontal ? openProgress : null) ?? (closing ? 0 : entered ? 1 : 0))
	// Disable the CSS transition while a finger is driving the panel, so it tracks 1:1 instead of lagging.
	const transitionOn = $derived(closeDrag === null && openProgress == null)
	const dragging = $derived(closeDrag !== null)

	const anchorClass = $derived(
		{
			left: 'inset-y-0 left-0',
			right: 'inset-y-0 right-0',
			top: 'inset-x-0 top-0',
			bottom: 'inset-x-0 bottom-0',
		}[direction]
	)
	const transform = $derived.by(() => {
		const off = (1 - openness) * 100
		return {
			left: `translateX(-${off}%)`,
			right: `translateX(${off}%)`,
			top: `translateY(-${off}%)`,
			bottom: `translateY(${off}%)`,
		}[direction]
	})

	// --- open/close orchestration, driven by the `open` prop -------------------------------------------
	$effect(() => {
		// Mount while open, or while an external open-drag (the shell's edge gesture) is pulling it in — the
		// store `open` only flips true once that gesture commits, so the panel must follow the finger first.
		if (open || (openProgress != null && openProgress > 0)) {
			visible = true
			closing = false
		} else if (visible && !closing) {
			startClose()
		}
	})

	// After mount, slide in on the next frame. A double rAF guarantees the off-screen start (openness 0)
	// paints before we flip `entered`, so the CSS transition actually animates.
	$effect(() => {
		if (visible && !closing && !entered) {
			let raf2 = 0
			const raf1 = requestAnimationFrame(() => (raf2 = requestAnimationFrame(() => (entered = true))))
			return () => {
				cancelAnimationFrame(raf1)
				cancelAnimationFrame(raf2)
			}
		}
	})

	onMount(() => {
		function onKey(e: KeyboardEvent) {
			if (e.key === 'Escape' && visible && !closing) requestClose()
		}
		window.addEventListener('keydown', onKey)
		return () => window.removeEventListener('keydown', onKey)
	})

	// Begin the slide-out (animation only — the parent already knows, e.g. it set `open=false`).
	function startClose() {
		if (closing) return
		closing = true
		// Back up the transitionend so reduced-motion (no transition → no event) still finalizes.
		setTimeout(() => closing && finalizeClose(), DURATION + 20)
	}

	// User-initiated dismissal: animate out AND tell the parent (so it can flip `open` / clear its mount).
	function requestClose() {
		if (closing) return
		startClose()
		onClose()
	}

	function finalizeClose() {
		if (!visible) return
		visible = false
		entered = false
		closing = false
		closeDrag = null
		onClosed?.()
	}

	// Track whether the panel slide is mid-flight (so content can stop scrolling), and finalize a close when
	// it lands. Driven by the transform transition's own events so it stays correct under reduced motion (no
	// transition → no events → never animating). Child transitions (e.g. the player's tempo slide) are
	// ignored via the guards.
	function onTransformStart(e: TransitionEvent) {
		if (e.target === e.currentTarget && e.propertyName === 'transform') animatingSlide = true
	}
	function onTransformEnd(e: TransitionEvent) {
		if (e.target !== e.currentTarget || e.propertyName !== 'transform') return
		animatingSlide = false
		if (closing) finalizeClose()
	}
	function onTransformCancel(e: TransitionEvent) {
		// Interrupted slide (e.g. reopened mid-close): unfreeze; the replacement transition re-freezes.
		if (e.target === e.currentTarget && e.propertyName === 'transform') animatingSlide = false
	}

	// --- gesture wiring ---------------------------------------------------------------------------------
	// Close-gesture option builders, shared by the panel auto-gesture and the exposed `drag` action. Their
	// callbacks read live state (closeDrag, panelH), so a one-shot application stays correct without updates.
	const swipeClose = (): SwipeOptions => ({
		side: direction === 'left' ? 'left' : 'right',
		mode: 'close',
		closeEdgeSize,
		closeEdgeFrom,
		onProgress: (o) => (closeDrag = o),
		onOpen: () => (closeDrag = null),
		onClose: () => {
			closeDrag = null
			requestClose()
		},
	})
	const verticalClose = (): SwipeVerticalOptions => ({
		onProgress: (dy) => {
			if (dy === 0) {
				closeDrag = null
				return
			}
			const d = direction === 'bottom' ? Math.max(0, dy) : Math.max(0, -dy)
			closeDrag = clampUnit(1 - d / (panelH || 1))
		},
		onSwipeDown: direction === 'bottom' ? requestClose : undefined,
		onSwipeUp: direction === 'top' ? requestClose : undefined,
	})

	// One `use:` dispatcher so only the axis-appropriate action is ever attached (so the two never fight over
	// the node's touch-action). `direction` never changes at runtime, so the branch is stable.
	const gesture: Action<HTMLElement, { horizontal: boolean; swipe: SwipeOptions; vertical: SwipeVerticalOptions }> = (
		node,
		p
	) => {
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		const inst: any = p.horizontal ? swipe(node, p.swipe) : swipeVertical(node, p.vertical)
		return {
			update: (np) => inst?.update?.(np.horizontal ? np.swipe : np.vertical),
			destroy: () => inst?.destroy?.(),
		}
	}

	// Panel auto-gesture: enabled only when open and `panelDrag`. A horizontal drawer sets touch-action:pan-y
	// (vertical scroll still works); a vertical sheet sets pan-x, so scrollable sheets pass panelDrag={false}
	// and confine the drag to a handle via `drag` below.
	const panelSwipe = $derived<SwipeOptions>({ ...swipeClose(), enabled: open && panelDrag })
	const panelVertical = $derived<SwipeVerticalOptions>({ ...verticalClose(), enabled: open && panelDrag })

	// Dismiss-drag exposed to content (e.g. a bottom sheet's handle). Always enabled — the element it's
	// applied to exists only while the panel is mounted.
	const drag: Action<HTMLElement> = (node) => {
		const inst = horizontal ? swipe(node, swipeClose()) : swipeVertical(node, verticalClose())
		return { destroy: () => inst?.destroy?.() }
	}
</script>

{#if scrim && visible}
	{#if scrimDismiss}
		<button
			type="button"
			aria-label={$translate('common.close')}
			class="fixed inset-0 bg-black {transitionOn
				? 'ease-fluid transition-opacity duration-500 motion-reduce:transition-none'
				: ''}"
			style="z-index: {effectiveScrimZ}; opacity: {scrimOpacity * openness}"
			onclick={requestClose}
		></button>
	{:else}
		<div
			class="pointer-events-none fixed inset-0 bg-black {transitionOn
				? 'ease-fluid transition-opacity duration-500 motion-reduce:transition-none'
				: ''}"
			style="z-index: {effectiveScrimZ}; opacity: {scrimOpacity * openness}"
		></div>
	{/if}
{/if}

{#if visible}
	<div
		bind:clientHeight={panelH}
		role="dialog"
		aria-modal="true"
		aria-label={ariaLabel}
		class="fixed {anchorClass} {className} {transitionOn
			? 'ease-fluid transition-transform duration-500 motion-reduce:transition-none'
			: ''}"
		style="z-index: {z}; transform: {transform}"
		ontransitionstart={onTransformStart}
		ontransitionend={onTransformEnd}
		ontransitioncancel={onTransformCancel}
		use:gesture={{ horizontal, swipe: panelSwipe, vertical: panelVertical }}
	>
		{@render children({ openness, dragging, drag, animating: animatingSlide && closeDrag === null })}
	</div>
{/if}
