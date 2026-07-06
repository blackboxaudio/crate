<script lang="ts">
	import { DRAG_THRESHOLD } from '$shared/utils/drag'
	import Spinner from './Spinner.svelte'

	// iOS-style pull-to-refresh. Rendered as an overlay INSIDE a `relative` wrapper that also holds the
	// scroll element; it attaches its own touch handling to that element (passed in via `scrollEl`) rather
	// than owning the scroll container, because the discovery feed's scroll element belongs to the
	// virtualizer. The gesture only engages at the very top of the list, pulling DOWN — so normal
	// scrolling and the rows' horizontal swipe-to-delete are never hijacked (we claim only once vertical
	// intent dominates and preventDefault only after claiming). On release past the threshold it spins
	// until `onRefresh` settles.
	type Props = {
		scrollEl: HTMLElement | null
		onRefresh: () => Promise<void> | void
		enabled?: boolean
	}
	let { scrollEl, onRefresh, enabled = true }: Props = $props()

	const THRESHOLD = 64 // px pulled (after resistance) that commits the refresh on release
	const MAX = 96 // px the indicator can travel while dragging
	const RESIST = 0.5 // drag-distance → indicator-travel damping (rubber-band feel)

	const FADE_MS = 200 // spinner fade-out after the refresh completes, before the gap collapses

	let distance = $state(0)
	let refreshing = $state(false)
	// Brief phase after the work finishes: the gap is held open while the spinner fades out in place, then
	// the gap collapses. Splitting the two keeps the exit a clean fade rather than the spinner being yanked
	// up as the list snaps back.
	let finishing = $state(false)

	// Gap height: held open at the threshold while refreshing or fading out, else follows the finger.
	const active = $derived(refreshing || finishing ? THRESHOLD : distance)
	// Faded fully out while finishing; otherwise tracks pull progress (full once past the threshold).
	const opacity = $derived(finishing ? 0 : Math.min((refreshing ? THRESHOLD : distance) / THRESHOLD, 1))
	// Ease things on settle (refresh/fade/snap-back); follow the finger 1:1 while actively dragging.
	const settling = $derived(refreshing || finishing || distance === 0)

	// Push the list DOWN by the pull distance so the spinner sits in the revealed gap above the content
	// (rather than overlaying the rows). The scroll element is owned by the host, so drive it here off the
	// reactive `active`. We use padding-top rather than a transform on purpose: a transform would make the
	// scroll element the containing block for the long-press ContextMenu's `position: fixed` overlay and
	// clip it during a refresh. padding-top pushes content the same way with no such side effect, and the
	// extra height just becomes scrollable — nothing overflows the container.
	let baseEl: HTMLElement | null = null
	let basePadTop = 0
	$effect(() => {
		const el = scrollEl
		if (!el) return
		if (el !== baseEl) {
			// Capture the element's resting top padding (from its classes) once, before we start overriding
			// it, so the pull offset stacks on top rather than replacing it (some hosts have their own pt-*).
			baseEl = el
			basePadTop = parseFloat(getComputedStyle(el).paddingTop) || 0
		}
		el.style.paddingTop = active > 0 ? `${basePadTop + active}px` : ''
		el.style.transition = settling ? 'padding-top 0.2s ease' : 'none'
		return () => {
			el.style.paddingTop = ''
			el.style.transition = ''
		}
	})

	let fadeTimer = 0

	async function trigger() {
		refreshing = true
		try {
			await onRefresh()
		} finally {
			// Fade the spinner out in place (gap held), then collapse the gap.
			refreshing = false
			finishing = true
			clearTimeout(fadeTimer)
			fadeTimer = window.setTimeout(() => {
				finishing = false
				distance = 0
			}, FADE_MS)
		}
	}

	// Cancel a pending fade if the list unmounts mid-refresh.
	$effect(() => () => clearTimeout(fadeTimer))

	$effect(() => {
		const el = scrollEl
		if (!el || !enabled) return

		let startX = 0
		let startY = 0
		let tracking = false
		let claimed = false

		function onTouchStart(e: TouchEvent) {
			if (refreshing || e.touches.length !== 1 || el!.scrollTop > 0) return
			startX = e.touches[0].clientX
			startY = e.touches[0].clientY
			tracking = true
			claimed = false
		}

		function onTouchMove(e: TouchEvent) {
			if (!tracking || refreshing) return
			const dx = e.touches[0].clientX - startX
			const dy = e.touches[0].clientY - startY

			if (!claimed) {
				// Content scrolled off the top, or the finger is heading up / sideways → this isn't a
				// pull-to-refresh. Bail so the native scroll and the row swipe-to-delete run untouched.
				if (el!.scrollTop > 0 || dy <= 0) {
					tracking = false
					return
				}
				if (dy < DRAG_THRESHOLD) return
				if (Math.abs(dx) > dy) {
					tracking = false
					return
				}
				claimed = true
			}

			// Own the gesture now: stop the scroll container from also panning.
			if (e.cancelable) e.preventDefault()
			distance = Math.min(dy * RESIST, MAX)
		}

		function onTouchEnd() {
			if (!tracking) return
			tracking = false
			if (!claimed) return
			if (distance >= THRESHOLD) void trigger()
			else distance = 0
		}

		el.addEventListener('touchstart', onTouchStart, { passive: true })
		el.addEventListener('touchmove', onTouchMove, { passive: false })
		el.addEventListener('touchend', onTouchEnd)
		el.addEventListener('touchcancel', onTouchEnd)
		return () => {
			el.removeEventListener('touchstart', onTouchStart)
			el.removeEventListener('touchmove', onTouchMove)
			el.removeEventListener('touchend', onTouchEnd)
			el.removeEventListener('touchcancel', onTouchEnd)
		}
	})
</script>

<!-- Spinner sits flat in the gap opened above the pushed-down list: an overlay whose height tracks the
     pull, with the bare spinner centered so it's evenly spaced between the container top and the list. -->
<div
	class="pointer-events-none absolute inset-x-0 top-0 z-20 flex items-center justify-center overflow-hidden text-text-tertiary"
	style="height: {active}px; opacity: {opacity}; transition: {settling
		? `height 0.2s ease, opacity ${FADE_MS}ms ease`
		: 'none'}"
>
	<Spinner class="h-5 w-5" />
</div>
