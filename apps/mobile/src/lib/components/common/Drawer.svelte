<script lang="ts">
	import type { Snippet } from 'svelte'
	import type { Action } from 'svelte/action'
	import { onMount } from 'svelte'
	import { translate } from '$shared/i18n'
	import { swipe, type SwipeOptions } from '$lib/actions/swipe'
	import { swipeVertical, type SwipeVerticalOptions } from '$lib/actions/swipeVertical'
	import { registerBackLayer } from '$lib/androidBack'

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
		/** Cross-fade the panel's opacity with its slide, so a translucent sheet dissolves instead of traveling
		 *  as a visible slab over the content behind it (the clash a glass bottom-sheet otherwise makes on
		 *  dismiss). Leave off for fullscreen pushes (release detail, player), which must stay opaque mid-slide
		 *  so the content behind never shows through. */
		fade?: boolean
		/** Bottom sheets only: slide by animating the panel's `bottom` position instead of `transform:
		 *  translateY`. It still slides up from the bottom, but the panel carries NO transform — so a field
		 *  focused on open isn't parented by a transformed element (the iOS caret-offset trigger). The native
		 *  caret tracks a layout-position move but not a composited transform, so it stays glued to the field as
		 *  the sheet slides. Use for keyboard-first sheets (e.g. the add-release form). */
		positionSlide?: boolean
		/** External finger-follow OPEN progress 0→1 (the shell's edge-open gesture). Horizontal only. */
		openProgress?: number | null
		/** Apply the dismiss gesture to the whole panel (default). A bottom sheet with scrollable content sets
		 *  this false and confines the drag to a handle via the exposed `drag` action — otherwise the panel's
		 *  `pan-x` touch-action would block the content's vertical scroll. */
		panelDrag?: boolean
		/** iOS-style back-swipe: restrict the close drag to start within this many px of an edge. Horizontal only. */
		closeEdgeSize?: number
		closeEdgeFrom?: 'left' | 'right'
		/** Skip the slide-in and appear already in place (boot-restored overlays reopen where the user left
		 *  off, not as a fresh navigation). Close still animates. Applies to every open of this instance —
		 *  fine for surfaces mounted via `{#if}` per open, which is how the detail views use it. */
		enterInstant?: boolean
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
		fade = false,
		positionSlide = false,
		openProgress = null,
		panelDrag = true,
		closeEdgeSize,
		closeEdgeFrom,
		enterInstant = false,
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
	// True once a bottom sheet has finished sliding open and is at rest. We then drop its `transform` to `none`
	// (identity `translateY(0%)` is visually the same) so a focused input inside it isn't parented by a
	// transformed element — iOS mispositions the text caret inside transformed elements and only lazily
	// corrects it, which showed up as the caret sitting well below the field for ~1s after the sheet opened.
	let settled = $state(false)

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
	// `positionSlide` bottom sheets animate `bottom` (see below), never `transform`, so this is unused for them.
	const posSlide = $derived(direction === 'bottom' && positionSlide)
	const transform = $derived.by(() => {
		// Bottom sheet at rest (fully open, no drag, not closing): no transform at all, so an input's caret sits
		// correctly. During the open/close slide or a dismiss drag it keeps the translate so the motion works.
		if (direction === 'bottom' && settled && closeDrag === null && !closing) return 'none'
		const off = (1 - openness) * 100
		return {
			left: `translateX(-${off}%)`,
			right: `translateX(${off}%)`,
			top: `translateY(-${off}%)`,
			bottom: `translateY(${off}%)`,
		}[direction]
	})
	// When `fade` is on, the panel's opacity rides `openness` so a translucent sheet dissolves as it slides.
	// A `posSlide` sheet instead slides by its `bottom` offset (a layout move, so a focused field's caret follows
	// it): `bottom: -100%` parks it fully below the viewport, `0%` is open. Both literal class strings are spelled
	// out so Tailwind can see them.
	const panelTransitionClass = $derived(
		!transitionOn
			? ''
			: posSlide
				? 'ease-fluid transition-[bottom] duration-500 motion-reduce:transition-none'
				: fade
					? 'ease-fluid transition-[transform,opacity] duration-500 motion-reduce:transition-none'
					: 'ease-fluid transition-transform duration-500 motion-reduce:transition-none'
	)
	const panelStyle = $derived(
		posSlide
			? `z-index: ${z}; bottom: ${-(1 - openness) * 100}%`
			: `z-index: ${z}; transform: ${transform}${fade ? `; opacity: ${openness}` : ''}`
	)

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
	// paints before we flip `entered`, so the CSS transition actually animates. `enterInstant` flips it
	// before the first paint instead, so the panel appears already in place (no slide).
	$effect(() => {
		if (visible && !closing && !entered) {
			if (enterInstant) {
				entered = true
				return
			}
			let raf2 = 0
			const raf1 = requestAnimationFrame(() => (raf2 = requestAnimationFrame(() => (entered = true))))
			return () => {
				cancelAnimationFrame(raf1)
				cancelAnimationFrame(raf2)
			}
		}
	})

	// Mark the sheet settled once the open slide has had time to land (the `transitionend` below sets it sooner
	// when a transition actually runs; this is the fallback for reduced motion, where no transition fires).
	$effect(() => {
		if (entered && !closing) {
			const t = setTimeout(() => {
				if (entered && !closing) settled = true
			}, DURATION + 20)
			return () => clearTimeout(t)
		}
	})

	onMount(() => {
		function onKey(e: KeyboardEvent) {
			if (e.key === 'Escape' && visible && !closing) requestClose()
		}
		window.addEventListener('keydown', onKey)
		return () => window.removeEventListener('keydown', onKey)
	})

	// Android Back closes the topmost open surface (#62). Registered while at rest open — the
	// effect's cleanup unregisters the moment a close starts (`closing` flips), so a rapid second
	// Back falls through to the next layer instead of re-closing this one.
	$effect(() => {
		if (visible && !closing) return registerBackLayer(requestClose)
	})

	// Begin the slide-out (animation only — the parent already knows, e.g. it set `open=false`).
	function startClose() {
		if (closing) return
		closing = true
		// Restore the transform for the slide-out (it was dropped to `none` at rest for the caret fix).
		settled = false
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
		settled = false
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
		else if (entered) settled = true
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
		class="fixed {anchorClass} {className} {panelTransitionClass}"
		style={panelStyle}
		ontransitionstart={onTransformStart}
		ontransitionend={onTransformEnd}
		ontransitioncancel={onTransformCancel}
		use:gesture={{ horizontal, swipe: panelSwipe, vertical: panelVertical }}
	>
		{@render children({ openness, dragging, drag, animating: animatingSlide && closeDrag === null })}
	</div>
{/if}
