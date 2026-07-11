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
	import ArtworkPlaceholder from '$lib/components/common/ArtworkPlaceholder.svelte'

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
		/** `releaseId:trackIndex` of the CURRENT track — a settle completes only when its own target has
		 *  landed (with rapid queued swipes, an EARLIER swipe's change must not complete it). */
		currentKey: string | null
		prevPick: QueuePick | null
		nextPick: QueuePick | null
		/** Whether paging next/previous is possible (false → rubber-band; never commits). */
		canNext: boolean
		canPrev: boolean
		enabled: boolean
		/** A committed swipe: queue the pending gesture and drive the player next/previous. */
		onRequestPage: (dir: 1 | -1, target: QueuePick | null) => void
		/** The store never confirmed a committed swipe (safety watchdog) — clear pending gestures. */
		onSettleTimeout: () => void
	}
	let {
		artSrc,
		changeFx,
		currentKey,
		prevPick,
		nextPick,
		canNext,
		canPrev,
		enabled,
		onRequestPage,
		onSettleTimeout,
	}: Props = $props()

	const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches

	const GAP = 24 // px between cards in the strip
	const SETTLE_MS = 300
	const STORE_SLIDE_MS = 350
	// Safety valve only: stream resolution routinely takes over a second, and the parked card waiting on
	// it is correct feedback — only a store that never responds at all should bounce the swipe back.
	const SETTLE_WATCHDOG_MS = 3000

	let coverW = $state(0)
	const slotDist = $derived(coverW + GAP)

	// Pager state machine. `dragX` is the strip's live translate; a CSS transition animates it only in
	// the settle/spring/storeSlide phases (finger-follow and resets are instant, Drawer's transitionOn
	// pattern). `strip` freezes the covers at drag start so the store landing mid-slide can't swap the
	// cards under the animation; `storeSlide` renders the outgoing cover for store-driven changes.
	type Phase = 'idle' | 'dragging' | 'settling' | 'spring' | 'storeSlide'
	let phase = $state<Phase>('idle')
	let dragX = $state(0)
	// The strip's frozen cards. The center is `centerSrc` normally; it's `centerPick` when a new drag
	// starts while a prior swipe is still settling — the parked incoming card becomes the center even
	// though its track hasn't technically landed in the store yet (see onDragStart).
	let strip = $state<{
		centerSrc?: string
		centerPick?: QueuePick | null
		prev: QueuePick | null
		next: QueuePick | null
	} | null>(null)
	let storeSlide = $state<{ outgoingSrc: string | undefined; dir: 1 | -1 } | null>(null)
	const transitionOn = $derived(phase === 'settling' || phase === 'spring' || phase === 'storeSlide')

	// A committed settle completes only when BOTH the slide animation ended AND the settle's own target
	// landed in the store (on iOS the native engine confirms asynchronously; on the HTML5 path the
	// stream fetch can take seconds — the parked card just waits it out, still interruptible).
	let settleEnded = false
	let storeLanded = false
	let settleTargetKey: string | null = null
	let lastCommitDir: 1 | -1 = 1
	// Seed with the change already present at mount. The pager remounts every time the player opens
	// (Drawer gates its children on `visible`), but `changeFx` lives in the always-mounted ExpandedPlayer
	// and still holds the last track change — swallowing it here means opening the player never slides the
	// artwork; only prev/next changes that land WHILE it's open (a fresh seq) animate.
	let lastFxSeq = changeFx?.seq ?? 0
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
	// A drag can start in ANY phase — a locked-out pager is what reads as "unresponsive". Interrupting
	// a spring/store-slide just snaps it home and drags fresh; interrupting a settle re-anchors on the
	// parked incoming card (its track may not have landed yet, but the queue already advanced, so the
	// live peeks are its real neighbors) so rapid consecutive swipes keep paging without waiting.
	function onDragStart() {
		clearTimers()
		if (phase === 'settling' && strip) {
			const pendingPick = lastCommitDir === 1 ? strip.next : strip.prev
			strip = pendingPick
				? { centerPick: pendingPick, prev: prevPick, next: nextPick }
				: { centerSrc: artSrc, prev: prevPick, next: nextPick }
		} else {
			strip = { centerSrc: artSrc, prev: prevPick, next: nextPick }
		}
		storeSlide = null
		dragX = 0
		settleEnded = false
		storeLanded = false
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
		settleTargetKey = target ? `${target.release.id}:${target.trackIndex}` : null
		lastCommitDir = dir
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
		// If the store never confirms at all (e.g. the queue emptied concurrently), spring back.
		watchdogTimer = setTimeout(() => {
			if (phase !== 'settling') return // a newer drag already took over
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
		if (phase === 'settling') {
			if (fx.viaGesture && (settleTargetKey === null || currentKey === settleTargetKey)) {
				// Our own target landed — the settle can complete (once the slide animation ends too).
				storeLanded = true
				completeIfReady()
			} else if (fx.viaGesture) {
				// An EARLIER queued swipe landed while we settle toward a later target: nothing to do
				// visually (our strip was re-anchored past it) — keep waiting for our own change.
			} else {
				// A different track raced past the gesture (an UpNext tap / lock-screen skip landed
				// instead): drop the frozen strip and run the normal store slide for what actually played.
				resetIdle()
				runStoreSlide(fx)
			}
		} else if (phase === 'dragging') {
			// Gesture-confirmed changes landing mid-drag are already represented (the strip re-anchored
			// on the pending card); an unrelated store change snaps the drag to the new reality.
			if (!fx.viaGesture) resetIdle()
		} else if (fx.viaGesture) {
			// A gesture's change landing after its settle already wound down (cancel/interrupt): the
			// card is in place, no extra motion.
			resetIdle()
		} else {
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
	// Outgoing fade: the traveling-away card dissolves over the back half of its journey so it's fully
	// gone by the time it parks at the edge — the strip's eventual DOM reset is then invisible instead
	// of a hard pop (its tilted corner otherwise lingers in view while a slow store change resolves).
	// The center card is the outgoing one during a gesture (progress 0 → 1); during a store slide the
	// center is the INCOMING cover (kept fully opaque) and the outgoing sits in a neighbor slot with
	// progress running 1 → 0. Under the drag it's untouched until ~45% travel (rubber-banding never
	// reaches the fade); once a slide animates, the CSS opacity transition carries it smoothly to 0.
	const centerOpacity = $derived(embellish && !storeSlide ? Math.max(0, 1 - Math.max(0, progress - 0.45) / 0.55) : 1)
	const outgoingOpacity = $derived(embellish ? Math.min(1, progress / 0.55) : 1)
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
	const centerPick = $derived(strip?.centerPick ?? null)
	const centerSrc = $derived(strip ? strip.centerSrc : artSrc)
	const prevSlotPick = $derived(storeSlide ? null : strip ? strip.prev : prevPick)
	const nextSlotPick = $derived(storeSlide ? null : strip ? strip.next : nextPick)

	const pagerOptions = $derived({
		// Never gated on the phase: a new drag interrupts whatever motion is in flight (onDragStart).
		enabled: enabled && coverW > 0,
		canPage: (dir: 1 | -1) => (dir === 1 ? canNext : canPrev),
		width: () => coverW,
		onDragStart,
		onDrag,
		onCommit,
		onCancel,
	})
</script>

{#snippet fallbackTile()}
	<ArtworkPlaceholder class="aspect-square w-full rounded-2xl shadow-2xl" />
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
					style="transform: translateX({-slotDist}px) scale({neighborScale}); opacity: {outgoingOpacity}; transition: {slotTransition}"
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

			<!-- Current track's cover (frozen while a gesture strip is active; a pick when a drag was
			     re-anchored on a still-pending swipe target). -->
			<div
				class="absolute inset-0"
				style="transform: {centerTransform}; opacity: {centerOpacity}; transition: {slotTransition}"
			>
				{#if centerPick}
					<ReleaseArtwork
						release={centerPick.release}
						eager
						class="aspect-square w-full rounded-2xl object-cover shadow-2xl"
						fallback={fallbackTile}
					/>
				{:else if centerSrc}
					<img src={centerSrc} alt="" class="aspect-square w-full rounded-2xl object-cover shadow-2xl" />
				{:else}
					{@render fallbackTile()}
				{/if}
			</div>

			<!-- Next slot: the peeked next cover, or the outgoing cover of a store-driven "previous" slide. -->
			{#if storeSlide?.dir === -1}
				<div
					class="absolute inset-0"
					style="transform: translateX({slotDist}px) scale({neighborScale}); opacity: {outgoingOpacity}; transition: {slotTransition}"
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
