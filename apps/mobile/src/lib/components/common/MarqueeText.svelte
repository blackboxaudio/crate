<script lang="ts">
	import { onMount } from 'svelte'

	// Single-line text that truncates with an ellipsis when it fits the available width, and — once it has
	// been on screen for `delay` ms — seamlessly loops if (and only if) the text overflows: it scrolls left
	// at a constant, gentle speed while a duplicate copy follows, so the text wraps around with no visible
	// jump and no confusing reverse "slide-back". A short dwell at the start of each loop lets the name be
	// read first (mirrors the iOS now-playing marquee). Honors prefers-reduced-motion (ellipsis only).
	//
	// Layout (flex sizing, color, font-size) is passed via `class` and applied to the clipping wrapper; the
	// inner text inherits those typographic styles, so callers style it exactly like a plain <span>.
	type Props = {
		text: string
		/** ms to wait after mount before the marquee starts (the "sensible pause" on open). */
		delay?: number
		class?: string
	}
	let { text, delay = 2500, class: className = '' }: Props = $props()

	// Gap (px) between a copy and its repeat; scroll speed (px/s, gentle); and the fraction of each cycle
	// spent moving (the rest is the dwell at the start of the loop).
	const GAP = 32
	const SPEED = 30
	const MOVING_FRACTION = 0.84

	let viewportEl: HTMLDivElement | undefined = $state()
	let textEl: HTMLSpanElement | undefined = $state()
	let textWidthPx = $state(0)
	let overflowPx = $state(0)
	let started = $state(false)
	let reducedMotion = $state(false)

	const shouldAnimate = $derived(started && overflowPx > 4 && !reducedMotion)
	// One loop travels textWidth + GAP, so the trailing copy lands exactly where the first began.
	const distancePx = $derived(textWidthPx + GAP)
	const durationMs = $derived(
		Math.round(Math.min(24000, Math.max(6000, ((distancePx / SPEED) * 1000) / MOVING_FRACTION)))
	)

	// scrollWidth is the full content width and clientWidth the visible width; both ignore the CSS
	// transform and the trailing margin, so they stay correct in either render state.
	function measure() {
		if (!textEl || !viewportEl) return
		textWidthPx = textEl.scrollWidth
		const o = textWidthPx - viewportEl.clientWidth
		overflowPx = o > 1 ? o : 0
	}

	onMount(() => {
		reducedMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false

		const ro = new ResizeObserver(measure)
		if (viewportEl) ro.observe(viewportEl)

		const timer = setTimeout(() => (started = true), delay)

		return () => {
			ro.disconnect()
			clearTimeout(timer)
		}
	})

	// Re-measure when the text changes (handles list reuse / live edits).
	$effect(() => {
		void text
		measure()
	})
</script>

<div bind:this={viewportEl} class="overflow-hidden {className}">
	{#if shouldAnimate}
		<div class="marquee-track" style="--marquee-distance: {distancePx}px; --marquee-duration: {durationMs}ms">
			<span bind:this={textEl} class="shrink-0 whitespace-nowrap" style="margin-right: {GAP}px">{text}</span>
			<span class="shrink-0 whitespace-nowrap" aria-hidden="true">{text}</span>
		</div>
	{:else}
		<span bind:this={textEl} class="block truncate whitespace-nowrap">{text}</span>
	{/if}
</div>

<style>
	.marquee-track {
		display: flex;
		width: max-content;
		animation: marquee var(--marquee-duration) linear infinite;
		will-change: transform;
	}

	/* Dwell at the start, then scroll one (text + gap) at a constant speed. The trailing copy lands
	   exactly where the first began, so the loop point is invisible — no reverse slide-back, no hard
	   reset; the text just flows continuously past. */
	@keyframes marquee {
		0%,
		16% {
			transform: translateX(0);
		}
		100% {
			transform: translateX(calc(-1 * var(--marquee-distance)));
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.marquee-track {
			animation: none;
		}
	}
</style>
