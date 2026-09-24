<script lang="ts">
	import type { Snippet } from 'svelte'
	import { onMount } from 'svelte'
	import { translate } from '$shared/i18n'
	import { swipeVertical } from '$lib/actions/swipeVertical'
	import { portalToBody } from '$lib/actions/portal'
	import { registerBackLayer } from '$lib/androidBack'

	// A web recreation of the iOS native context menu. Long-press a row → its rect is captured and passed
	// as `anchorRect`; this overlay dims+blurs the background, lifts a `preview` of the row in place, and
	// springs a menu platter (the `children` rows) in beside it — below the row if there's room, else above.
	// `preview` is optional: omit it for a button-anchored menu (e.g. a ⋯ "more" button) and the platter just
	// springs from the anchor with no lifted row, matching iOS's tap-a-button `UIMenu` presentation.
	// Tap the backdrop, swipe down, tap a row, or press Escape to dismiss. Motion is pure CSS (matching the
	// app's `--ease-fluid` sheet feel, with `--ease-spring` for the platter's pop); the lifecycle mirrors
	// `Drawer` (mount → double-rAF enter → transitionend finalize, backed by a timeout for reduced motion).
	type Rect = { top: number; left: number; width: number; height: number }
	type Props = {
		open: boolean
		/** Viewport rect of the anchor — a long-pressed row, or a tapped ⋯ button (a plain snapshot). */
		anchorRect: Rect | null
		/** Optional lifted preview of the source row (rendered from data, not a DOM clone). Omit for a
		 *  button-anchored menu: the platter then springs from the anchor with no lifted row. */
		preview?: Snippet
		/** The menu rows (`ContextMenuItem`s). */
		children: Snippet
		/** Opened by a discrete tap (e.g. a ⋯ button) rather than a held long-press. The opening press has
		 *  already lifted, so there's no trailing pointerup/click to swallow — dismissal arms immediately. */
		tapTriggered?: boolean
		/** User-initiated dismissal — the caller should flip `open` false in response. */
		onClose: () => void
		/** Fires once the dismiss animation has fully landed (the caller can clear latched state then). */
		onClosed?: () => void
	}
	let { open, anchorRect, preview, children, tapTriggered = false, onClose, onClosed }: Props = $props()

	const GAP = 8 // px between the lifted row and the platter
	const MARGIN = 12 // px the platter/preview keep from the safe-area edges
	const PLATTER_MAX_W = 260 // px — iOS platters are ~250pt
	const DURATION = 250 // ms; keep the inline transitions below in sync

	// --- Environment (measured once / on resize) -------------------------------------------------------
	let reduceMotion = $state(false)
	let vw = $state(0)
	let vh = $state(0)
	let safeTop = $state(0)
	let safeBottom = $state(0)

	onMount(() => {
		const mq = window.matchMedia('(prefers-reduced-motion: reduce)')
		reduceMotion = mq.matches
		const onMq = () => (reduceMotion = mq.matches)
		mq.addEventListener('change', onMq)

		const measure = () => {
			vw = window.innerWidth
			vh = window.innerHeight
		}
		measure()
		window.addEventListener('resize', measure)

		// Read the live safe-area insets off a throwaway probe so the positioning math can keep the menu
		// clear of the notch / home indicator (env() isn't otherwise visible to JS).
		const probe = document.createElement('div')
		probe.style.cssText =
			'position:fixed;visibility:hidden;top:0;left:0;padding-top:env(safe-area-inset-top);padding-bottom:env(safe-area-inset-bottom);'
		document.body.appendChild(probe)
		const cs = getComputedStyle(probe)
		safeTop = parseFloat(cs.paddingTop) || 0
		safeBottom = parseFloat(cs.paddingBottom) || 0
		probe.remove()

		function onKey(e: KeyboardEvent) {
			if (e.key === 'Escape' && visible && !closing) requestClose()
		}
		window.addEventListener('keydown', onKey)

		return () => {
			mq.removeEventListener('change', onMq)
			window.removeEventListener('resize', measure)
			window.removeEventListener('keydown', onKey)
		}
	})

	// --- Lifecycle (mirrors Drawer) --------------------------------------------------------------------
	let visible = $state(false) // mounted
	let entered = $state(false) // animated into place
	let closing = $state(false) // animating out
	let contentH = $state(0) // natural height of the rows (drives placement; measured on the inner div)
	let platterEl = $state<HTMLElement | null>(null)
	let restoreFocus: HTMLElement | null = null
	let armed = $state(false) // whether a release/tap is allowed to dismiss (see the arming effect)

	const shown = $derived(entered && !closing)

	$effect(() => {
		if (open) {
			if (!visible) restoreFocus = (document.activeElement as HTMLElement | null) ?? null
			visible = true
			closing = false
		} else if (visible && !closing) {
			startClose()
		}
	})

	// Android Back dismisses the platter (#62) — same lifecycle window as Drawer's registration:
	// registered while at rest open, unregistered the moment a close starts.
	$effect(() => {
		if (visible && !closing) return registerBackLayer(requestClose)
	})

	// Slide in once mounted AND measured (so the platter springs from the right origin, not from top-left).
	$effect(() => {
		if (visible && !closing && !entered && contentH > 0) {
			let raf2 = 0
			const raf1 = requestAnimationFrame(() => (raf2 = requestAnimationFrame(() => (entered = true))))
			return () => {
				cancelAnimationFrame(raf1)
				cancelAnimationFrame(raf2)
			}
		}
	})

	// Move focus to the first action when shown; restore it on close (see finalize). Programmatic focus
	// doesn't trigger :focus-visible, so no ring appears for a touch open.
	$effect(() => {
		if (shown && platterEl) platterEl.querySelector<HTMLElement>('[role="menuitem"]')?.focus()
	})

	// Ignore the long-press that opened the menu: it fires while the finger is still down, so the pointerup
	// — and the click iOS synthesizes from it on the full-screen backdrop — would otherwise dismiss the menu
	// the instant the user lifts. Arm dismissal only once that opening press has lifted, deferred two frames
	// so the synthesized click lands while still unarmed and is ignored.
	$effect(() => {
		if (!visible) {
			armed = false
			return
		}
		// Tap-triggered (⋯ button): the opening press lifted before we mounted, so there's no trailing
		// pointerup to wait for — arm right away or the first backdrop tap would be swallowed.
		if (tapTriggered) {
			armed = true
			return
		}
		function arm() {
			window.removeEventListener('pointerup', arm, true)
			window.removeEventListener('pointercancel', arm, true)
			requestAnimationFrame(() => requestAnimationFrame(() => (armed = true)))
		}
		window.addEventListener('pointerup', arm, true)
		window.addEventListener('pointercancel', arm, true)
		return () => {
			window.removeEventListener('pointerup', arm, true)
			window.removeEventListener('pointercancel', arm, true)
		}
	})

	// Freeze the list underneath. The menu opens MID-TOUCH — the finger that long-pressed is still down on
	// the row, and the browser's native pan on the scroll container behind is still live, so dragging
	// without lifting would scroll the (supposedly frozen) feed behind the overlay. The backdrop can't stop
	// that: hit-testing was settled at touchstart, and `touch-action` is only read when the gesture starts,
	// so neither has any effect on a gesture already in flight. The one thing that does is cancelling the
	// touch stream itself — a non-passive `touchmove` listener that preventDefaults. It stays for the whole
	// open lifetime, so a fresh drag on the backdrop can't scroll anything either. The platter is exempt:
	// it scrolls internally when the menu is taller than the space beside the row.
	$effect(() => {
		if (!visible) return
		function onTouchMove(e: TouchEvent) {
			if (platterEl?.contains(e.target as Node)) return
			if (e.cancelable) e.preventDefault()
		}
		window.addEventListener('touchmove', onTouchMove, { passive: false, capture: true })
		return () => window.removeEventListener('touchmove', onTouchMove, true)
	})

	function startClose() {
		if (closing) return
		// Fade the developed blur out instead of snapping it off (the effect above drops `blurOn` once
		// `shown` flips; `blurExit` holds the filter through the brief opacity fade).
		if (blurOn) {
			blurExit = true
			if (blurExitTimer) clearTimeout(blurExitTimer)
			blurExitTimer = setTimeout(() => (blurExit = false), 170)
		}
		closing = true
		// Back up transitionend so reduced motion (no transition → no event) still finalizes.
		setTimeout(() => closing && finalize(), DURATION + 40)
	}

	function requestClose() {
		if (closing) return
		startClose()
		onClose()
	}

	function finalize() {
		if (!visible) return
		visible = false
		entered = false
		closing = false
		contentH = 0
		const r = restoreFocus
		restoreFocus = null
		onClosed?.()
		r?.focus?.()
	}

	function onPlatterTransitionEnd(e: TransitionEvent) {
		if (e.target !== e.currentTarget || e.propertyName !== 'transform') return
		if (closing) finalize()
	}

	function onPlatterKeydown(e: KeyboardEvent) {
		if (e.key !== 'Tab' || !platterEl) return
		const items = Array.from(platterEl.querySelectorAll<HTMLElement>('[role="menuitem"]'))
		if (items.length === 0) return
		const first = items[0]
		const last = items[items.length - 1]
		if (e.shiftKey && document.activeElement === first) {
			e.preventDefault()
			last.focus()
		} else if (!e.shiftKey && document.activeElement === last) {
			e.preventDefault()
			first.focus()
		}
	}

	// --- Placement -------------------------------------------------------------------------------------
	const placement = $derived.by(() => {
		if (!anchorRect || !contentH || !vh) return null
		const topLimit = safeTop + MARGIN
		const bottomLimit = vh - safeBottom - MARGIN
		const belowTop = anchorRect.top + anchorRect.height + GAP
		const aboveTop = anchorRect.top - GAP - contentH
		const roomBelow = bottomLimit - belowTop
		const roomAbove = anchorRect.top - GAP - topLimit

		let side: 'below' | 'above' = 'below'
		let top = belowTop
		let maxH = roomBelow
		if (contentH <= roomBelow) {
			side = 'below'
			top = belowTop
			maxH = roomBelow
		} else if (contentH <= roomAbove) {
			side = 'above'
			top = aboveTop
			maxH = roomAbove
		} else if (roomBelow >= roomAbove) {
			// Taller than either side → open on the roomier side and scroll internally.
			side = 'below'
			top = belowTop
			maxH = roomBelow
		} else {
			side = 'above'
			top = topLimit
			maxH = roomAbove
		}

		const width = Math.min(PLATTER_MAX_W, vw - 2 * MARGIN)
		const left = Math.max(MARGIN, Math.min(anchorRect.left, vw - MARGIN - width))
		const originX = Math.max(0, Math.min(width, anchorRect.left + anchorRect.width / 2 - left))
		return { side, top, left, width, maxH, originX }
	})

	// The preview sits exactly over the source row, clamped fully into the safe area if the row was partly
	// off-screen (so a long-press near the header/home-indicator still lifts something visible).
	const previewStyle = $derived.by(() => {
		if (!anchorRect) return ''
		let top = anchorRect.top
		if (vh) top = Math.max(safeTop + MARGIN, Math.min(top, vh - safeBottom - MARGIN - anchorRect.height))
		return `top:${top}px;left:${anchorRect.left}px;width:${anchorRect.width}px;height:${anchorRect.height}px;`
	})

	// --- Animated values -------------------------------------------------------------------------------
	const dur = $derived(reduceMotion ? 130 : DURATION)
	const fade = $derived(shown ? 1 : 0)
	const platterScale = $derived(reduceMotion ? 1 : shown ? 1 : 0.82)
	const previewScale = $derived(reduceMotion ? 1 : shown ? 1.04 : 1)
	const platterTransition = $derived(
		reduceMotion
			? `opacity ${dur}ms ease`
			: `transform ${dur}ms ${shown ? 'var(--ease-spring)' : 'var(--ease-fluid)'}, opacity ${dur}ms ease`
	)
	const previewTransition = $derived(
		reduceMotion ? `opacity ${dur}ms ease` : `transform ${dur}ms var(--ease-fluid), opacity ${dur}ms ease`
	)

	// The backdrop blur is deferred while the menu animates in: opacity-fading a full-viewport
	// backdrop-filter layer makes WebKit re-rasterize the blurred scene every frame, so the heavy pop
	// frames (lift + platter spring) run over the plain dim, and the blur's own fade starts just before
	// the spring lands — overlapping its subtle tail so dim → blur reads as ONE continuous settle, not
	// pop-then-blur. The blur layer sits at the BOTTOM of the stack (below the dim), so its backdrop
	// input is only the frozen feed: nothing it samples changes during its fade. Timeout-based so it
	// fires under reduced motion too. On close it fades back out over a short window (`blurExit` keeps
	// the filter applied so the opacity fade is visible, then removes it) — quicker than the platter's
	// exit so most of the dismissal still runs blur-free.
	let blurOn = $state(false)
	let blurExit = $state(false)
	let blurExitTimer: ReturnType<typeof setTimeout> | null = null
	$effect(() => {
		if (!shown) {
			blurOn = false
			return
		}
		const t = setTimeout(() => (blurOn = true), Math.max(0, dur - 80))
		return () => clearTimeout(t)
	})
</script>

{#if visible}
	<!-- Backdrop: a faint dim that animates with the open/close, over a frosted-blur layer that develops
	     as the menu lands (iOS-style; see `blurOn` above — animating anything on a live backdrop-filter
	     janks WKWebView). The blur layer comes FIRST so the dim fades ABOVE it (dimmed blurred feed ≈
	     blurred dimmed feed, but the dim's fade never invalidates the blur's sampled backdrop). Tap /
	     swipe-down on the dim dismisses; covering everything, it also locks the feed behind from
	     scrolling. -->
	<!-- All layers portal to <body>: an instance mounted inside a detail drawer (playlist/tag/
	     follow rows) would otherwise be capped at that drawer's z-30 stacking context and render
	     under the z-40 mini player. Mount order (blur → dim → preview → platter) keeps their
	     relative layering, since all share z-60. -->
	<div
		aria-hidden="true"
		class="pointer-events-none fixed inset-0 z-[60]"
		style="-webkit-backdrop-filter: {blurOn || blurExit
			? 'blur(12px) saturate(150%)'
			: 'none'}; backdrop-filter: {blurOn || blurExit ? 'blur(12px) saturate(150%)' : 'none'}; opacity: {blurOn
			? 1
			: 0}; transition: {blurOn ? 'opacity 200ms ease-out' : blurExit ? 'opacity 150ms ease' : 'none'};"
		use:portalToBody
	></div>

	<button
		type="button"
		aria-label={$translate('common.close')}
		class="fixed inset-0 z-[60]"
		style="background-color: rgba(0, 0, 0, 0.18); opacity: {fade}; transition: opacity {dur}ms ease;"
		onclick={() => armed && requestClose()}
		use:portalToBody
		use:swipeVertical={{ onSwipeDown: () => armed && requestClose(), enabled: true }}
	></button>

	{#if anchorRect && preview}
		<div
			aria-hidden="true"
			use:portalToBody
			class="pointer-events-none fixed z-[60] flex items-center gap-3 overflow-hidden rounded-xl bg-surface-0 px-4"
			style="{previewStyle} opacity: {fade}; transform: scale({previewScale}); transform-origin: center; box-shadow: 0 10px 40px -8px rgba(0,0,0,0.45); transition: {previewTransition};"
		>
			{@render preview()}
		</div>
	{/if}

	<div
		bind:this={platterEl}
		role="menu"
		tabindex="-1"
		use:portalToBody
		class="fixed z-[60] overflow-y-auto overscroll-contain rounded-2xl border border-stroke-subtle bg-surface-1 shadow-2xl select-none"
		style="top: {placement?.top ?? 0}px; left: {placement?.left ?? 0}px; width: {placement?.width ??
			PLATTER_MAX_W}px; max-height: {placement?.maxH ??
			0}px; opacity: {fade}; transform: scale({platterScale}); transform-origin: {placement?.originX ??
			0}px {placement?.side === 'above' ? '100%' : '0%'}; visibility: {placement
			? 'visible'
			: 'hidden'}; transition: {platterTransition};"
		ontransitionend={onPlatterTransitionEnd}
		onkeydown={onPlatterKeydown}
	>
		<div bind:clientHeight={contentH} class="py-1">
			{@render children()}
		</div>
	</div>
{/if}
