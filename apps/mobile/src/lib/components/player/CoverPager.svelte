<script lang="ts" module>
	// One track change, as detected by ExpandedPlayer's previewInfo watcher. Drives every layer of the
	// change animation (this pager, the title roll, the background crossfade): `kind` is whether the
	// release changed, `dir` the spatial direction (1 = next / cards travel left), `viaGesture` whether
	// a committed swipe requested exactly this change (the pager then completes its in-flight settle
	// instead of starting a fresh slide), and `outgoingSrc` the previous track's rendered cover src.
	// `seq` makes each change a fresh event even when the fields repeat.
	export interface TrackChangeFx {
		kind: 'same' | 'cross'
		dir: 1 | -1
		viaGesture: boolean
		outgoingSrc?: string
		seq: number
	}
</script>

<script lang="ts">
	import type { Pick as QueuePick } from '$shared/stores/playbackQueue'
	import { swipePager } from '$lib/actions/swipePager'
	import { lightTap } from '$lib/utils/haptics'
	import ReleaseArtwork from '$lib/components/common/ReleaseArtwork.svelte'

	// The expanded player's swipeable cover: a 3-slot strip (previous | current | next) that follows the
	// finger during a horizontal drag with the neighboring tracks' real covers peeking in, rubber-bands
	// where no neighbor exists, and commits a page (next/previous track) on release. The actual content
	// swap is always store-driven (`changeFx` from ExpandedPlayer's previewInfo watcher) so lock-screen
	// skips, auto-advance, and the transport buttons animate through the same slide; a committed gesture
	// freezes the strip it was dragging and hands off to the store change without a flash.
	type Props = {
		/** Resolved cover src of the CURRENT track (cache-first, owned by ExpandedPlayer). */
		artSrc: string | undefined
		/** The latest track change, store-driven. The pager slides (or completes a settle) on each one. */
		changeFx: TrackChangeFx | null
		prevPick: QueuePick | null
		nextPick: QueuePick | null
		/** Whether paging next/previous is possible (false → rubber-band; never commits). */
		canNext: boolean
		canPrev: boolean
		enabled: boolean
		/** A committed swipe: set the pending gesture and drive the player next/previous. */
		onRequestPage: (dir: 1 | -1, target: QueuePick | null) => void
		/** The store never confirmed a committed swipe (watchdog) — clear the pending gesture. */
		onSettleTimeout: () => void
	}
	let { artSrc, changeFx, prevPick, nextPick, canNext, canPrev, enabled, onRequestPage, onSettleTimeout }: Props =
		$props()

	const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches

	const GAP = 24 // px between cards in the strip
	const SETTLE_MS = 300
	const STORE_SLIDE_MS = 350
	const SETTLE_WATCHDOG_MS = 800

	let coverW = $state(0)
	const slotDist = $derived(coverW + GAP)

	// Pager state machine. `dragX` is the strip's live translate; a CSS transition animates it only in
	// the settle/spring/storeSlide phases (finger-follow and resets are instant, Drawer's transitionOn
	// pattern). `strip` freezes the covers at drag start so the store landing mid-slide can't swap the
	// cards under the animation; `storeSlide` renders the outgoing cover for store-driven changes.
	type Phase = 'idle' | 'dragging' | 'settling' | 'spring' | 'storeSlide'
	let phase = $state<Phase>('idle')
	let dragX = $state(0)
	let strip = $state<{ centerSrc: string | undefined; prev: QueuePick | null; next: QueuePick | null } | null>(null)
	let storeSlide = $state<{ outgoingSrc: string | undefined; dir: 1 | -1 } | null>(null)
	const transitionOn = $derived(phase === 'settling' || phase === 'spring' || phase === 'storeSlide')

	// A committed settle completes only when BOTH the slide animation ended AND the store change landed
	// (on iOS the native engine confirms a "next" asynchronously, often after the card has parked).
	let settleEnded = false
	let storeLanded = false
	let lastFxSeq = 0
	let fallbackTimer: ReturnType<typeof setTimeout> | null = null
	let watchdogTimer: ReturnType<typeof setTimeout> | null = null
	let slideRaf = 0

	function clearTimers() {
		if (fallbackTimer) clearTimeout(fallbackTimer)
		if (watchdogTimer) clearTimeout(watchdogTimer)
		cancelAnimationFrame(slideRaf)
		fallbackTimer = null
		watchdogTimer = null
	}

	// Back to rest: the live (post-change) cover centered, no transition (phase 'idle' → instant), so a
	// completed settle swaps frozen-strip-at-parked-position → fresh-centered-strip pixel-identically.
	function resetIdle() {
		clearTimers()
		phase = 'idle'
		dragX = 0
		strip = null
		storeSlide = null
		settleEnded = false
		storeLanded = false
	}

	function completeIfReady() {
		if (settleEnded && storeLanded) resetIdle()
	}

	// --- gesture callbacks ------------------------------------------------------------------------
	function onDragStart() {
		clearTimers()
		storeSlide = null
		strip = { centerSrc: artSrc, prev: prevPick, next: nextPick }
		phase = 'dragging'
	}

	function onDrag(dx: number) {
		dragX = dx
	}

	function onCancel() {
		if (phase !== 'dragging') return
		if (reducedMotion) {
			resetIdle()
			return
		}
		phase = 'spring'
		dragX = 0
		fallbackTimer = setTimeout(resetIdle, SETTLE_MS + 100) // transitionend backup
	}

	function onCommit(dir: 1 | -1) {
		if (phase !== 'dragging') return
		void lightTap()
		const target = dir === 1 ? (strip?.next ?? null) : (strip?.prev ?? null)
		settleEnded = false
		storeLanded = false
		phase = 'settling'
		dragX = -dir * slotDist
		if (reducedMotion) {
			settleEnded = true // no transition runs — the card parks instantly
		} else {
			fallbackTimer = setTimeout(() => {
				settleEnded = true
				completeIfReady()
			}, SETTLE_MS + 100)
		}
		// If the store never confirms (e.g. the queue emptied concurrently), spring back.
		watchdogTimer = setTimeout(() => {
			onSettleTimeout()
			if (reducedMotion) resetIdle()
			else {
				phase = 'spring'
				dragX = 0
				fallbackTimer = setTimeout(resetIdle, SETTLE_MS + 100)
			}
		}, SETTLE_WATCHDOG_MS)
		onRequestPage(dir, target)
	}

	function onTransitionEnd(e: TransitionEvent) {
		if (e.target !== e.currentTarget || e.propertyName !== 'transform') return
		if (phase === 'spring' || phase === 'storeSlide') resetIdle()
		else if (phase === 'settling') {
			settleEnded = true
			completeIfReady()
		}
	}

	// --- store-driven changes -----------------------------------------------------------------------
	// Slide the new (already-current) cover in from the change's direction: park the strip one slot
	// toward `dir` with the OUTGOING cover occupying the slot now at center (no transition), then
	// animate back to rest. Serves buttons, auto-advance, lock-screen skips — and same-release changes,
	// where the identical card visibly travels.
	function runStoreSlide(fx: TrackChangeFx) {
		if (reducedMotion || coverW === 0) return // instant swap
		storeSlide = { outgoingSrc: fx.outgoingSrc, dir: fx.dir }
		phase = 'idle' // transition off for the initial park
		dragX = fx.dir * slotDist
		slideRaf = requestAnimationFrame(() => {
			slideRaf = requestAnimationFrame(() => {
				phase = 'storeSlide'
				dragX = 0
				fallbackTimer = setTimeout(resetIdle, STORE_SLIDE_MS + 100)
			})
		})
	}

	$effect(() => {
		const fx = changeFx
		if (!fx || fx.seq === lastFxSeq) return
		lastFxSeq = fx.seq
		if (watchdogTimer) clearTimeout(watchdogTimer)
		watchdogTimer = null
		if (phase === 'settling' && fx.viaGesture) {
			storeLanded = true
			completeIfReady()
		} else if (phase === 'dragging') {
			// The store changed under an active drag (rare — e.g. a lock-screen skip): drop the frozen
			// strip and snap to the new track; the finger keeps dragging the fresh card.
			resetIdle()
		} else {
			// Normal store-driven change — or a settling gesture that a different track raced past
			// (an UpNext tap / lock-screen skip landed instead of the gesture's target).
			resetIdle()
			runStoreSlide(fx)
		}
	})

	$effect(() => () => clearTimers())

	// --- render derivations ---------------------------------------------------------------------------
	// Progress 0→1 of the current card's travel toward a neighbor slot; drives the card-deck feel
	// (incoming cover grows/brightens 0.92/0.7 → 1/1, current card tilts up to ±3° and shrinks to 0.95).
	const progress = $derived(slotDist > 0 ? Math.min(1, Math.abs(dragX) / slotDist) : 0)
	const embellish = $derived(!reducedMotion)
	const centerTransform = $derived(
		embellish && dragX !== 0
			? `rotate(${((dragX / Math.max(1, slotDist)) * -3).toFixed(3)}deg) scale(${(1 - progress * 0.05).toFixed(4)})`
			: 'none'
	)
	const neighborScale = $derived(embellish ? 0.92 + 0.08 * progress : 1)
	const neighborOpacity = $derived(embellish ? 0.7 + 0.3 * progress : 1)
	const slotTransition = $derived(
		transitionOn && !reducedMotion
			? `transform ${phase === 'storeSlide' ? STORE_SLIDE_MS : SETTLE_MS}ms var(--ease-fluid), opacity ${
					phase === 'storeSlide' ? STORE_SLIDE_MS : SETTLE_MS
				}ms var(--ease-fluid)`
			: 'none'
	)

	// What each slot shows. While a frozen strip exists (drag/settle) it wins; during a store slide the
	// outgoing cover occupies the slot the strip parked on and the other neighbor is hidden (its peeked
	// pick may already be stale); at rest the live peeked neighbors pre-mount so their art is decoded
	// before a drag starts.
	const centerSrc = $derived(strip ? strip.centerSrc : artSrc)
	const prevSlotPick = $derived(storeSlide ? null : strip ? strip.prev : prevPick)
	const nextSlotPick = $derived(storeSlide ? null : strip ? strip.next : nextPick)

	const pagerOptions = $derived({
		enabled: enabled && coverW > 0 && (phase === 'idle' || phase === 'dragging'),
		canPage: (dir: 1 | -1) => (dir === 1 ? canNext : canPrev),
		width: () => coverW,
		onDragStart,
		onDrag,
		onCommit,
		onCancel,
	})
</script>

{#snippet fallbackTile()}
	<div class="flex aspect-square w-full items-center justify-center rounded-2xl bg-surface-2 text-text-tertiary">
		<svg viewBox="0 0 24 24" class="h-16 w-16" fill="currentColor">
			<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
		</svg>
	</div>
{/snippet}

<div class="flex flex-1 items-center justify-center overflow-hidden px-4 pt-3" use:swipePager={pagerOptions}>
	<div class="relative aspect-square w-full max-w-sm" bind:clientWidth={coverW}>
		<div
			class="absolute inset-0"
			style="transform: translateX({dragX}px); transition: {transitionOn && !reducedMotion
				? `transform ${phase === 'storeSlide' ? STORE_SLIDE_MS : SETTLE_MS}ms var(--ease-fluid)`
				: 'none'}"
			ontransitionend={onTransitionEnd}
		>
			<!-- Previous slot: the peeked previous cover, or the outgoing cover of a store-driven "previous ← next" slide. -->
			{#if storeSlide?.dir === 1}
				<div
					class="absolute inset-0"
					style="transform: translateX({-slotDist}px) scale({neighborScale}); opacity: {neighborOpacity}; transition: {slotTransition}"
				>
					{#if storeSlide.outgoingSrc}
						<img src={storeSlide.outgoingSrc} alt="" class="aspect-square w-full rounded-2xl object-cover shadow-2xl" />
					{:else}
						{@render fallbackTile()}
					{/if}
				</div>
			{:else if prevSlotPick}
				<div
					class="absolute inset-0"
					style="transform: translateX({-slotDist}px) scale({neighborScale}); opacity: {neighborOpacity}; transition: {slotTransition}"
				>
					<ReleaseArtwork
						release={prevSlotPick.release}
						eager
						class="aspect-square w-full rounded-2xl object-cover shadow-2xl"
						fallback={fallbackTile}
					/>
				</div>
			{/if}

			<!-- Current track's cover (frozen while a gesture strip is active). -->
			<div class="absolute inset-0" style="transform: {centerTransform}; transition: {slotTransition}">
				{#if centerSrc}
					<img src={centerSrc} alt="" class="aspect-square w-full rounded-2xl object-cover shadow-2xl" />
				{:else}
					{@render fallbackTile()}
				{/if}
			</div>

			<!-- Next slot: the peeked next cover, or the outgoing cover of a store-driven "previous" slide. -->
			{#if storeSlide?.dir === -1}
				<div
					class="absolute inset-0"
					style="transform: translateX({slotDist}px) scale({neighborScale}); opacity: {neighborOpacity}; transition: {slotTransition}"
				>
					{#if storeSlide.outgoingSrc}
						<img src={storeSlide.outgoingSrc} alt="" class="aspect-square w-full rounded-2xl object-cover shadow-2xl" />
					{:else}
						{@render fallbackTile()}
					{/if}
				</div>
			{:else if nextSlotPick}
				<div
					class="absolute inset-0"
					style="transform: translateX({slotDist}px) scale({neighborScale}); opacity: {neighborOpacity}; transition: {slotTransition}"
				>
					<ReleaseArtwork
						release={nextSlotPick.release}
						eager
						class="aspect-square w-full rounded-2xl object-cover shadow-2xl"
						fallback={fallbackTile}
					/>
				</div>
			{/if}
		</div>
	</div>
</div>
