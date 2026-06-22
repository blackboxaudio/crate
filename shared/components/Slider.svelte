<script lang="ts">
	import { onDestroy } from 'svelte'

	// Shared slider primitive used by both apps (desktop player controls, mobile player sheet). A styled
	// range input with two fill modes: unipolar — the brand colour fills from the left edge to the thumb,
	// like a volume control or a scrubber — and bipolar, where it fills outward from the centre to the
	// thumb, like a pitch fader. `snapToCenter` adds a detent at the midpoint; `onpointerdown` is forwarded
	// so a host can stop the event reaching an ancestor gesture (e.g. the mobile player's swipe-to-dismiss).
	type Props = {
		value?: number
		min?: number
		max?: number
		step?: number
		disabled?: boolean
		bipolar?: boolean
		snapToCenter?: number
		/** Diameter of the visible white thumb in px. Defaults to 12. */
		thumbSize?: number
		/** Touch-target diameter in px. When larger than `thumbSize`, a transparent ring enlarges the grab
		 *  area without enlarging the visible dot — handy on touch. Omit to make the hit area equal the dot. */
		hitSize?: number
		/** Scale the thumb to this factor while the pointer is held (a grab cue), easing back on release.
		 *  Omit for no grow. */
		activeScale?: number
		ariaLabel?: string
		class?: string
		oninput?: (e: Event) => void
		onchange?: (e: Event) => void
		onpointerdown?: (e: PointerEvent) => void
	}

	let {
		value = $bindable(0),
		min = 0,
		max = 100,
		step = 1,
		disabled = false,
		bipolar = false,
		snapToCenter,
		thumbSize = 12,
		hitSize,
		activeScale,
		ariaLabel,
		class: className = '',
		oninput,
		onchange,
		onpointerdown,
	}: Props = $props()

	// Guard a zero-width range (e.g. a scrubber before its duration is known) so the fill stops never go
	// NaN, and keep the thumb position clamped to the track.
	let percentage = $derived.by(() => {
		const range = max - min
		if (range <= 0) return 0
		return Math.max(0, Math.min(100, ((value - min) / range) * 100))
	})

	// Align the fill's moving edge with the thumb's *centre*, not with `percentage%` of the track. A native
	// range thumb can't overflow the track, so the browser insets its travel by half the thumb on each end:
	// the centre runs from thumb/2 (at min) to trackWidth − thumb/2 (at max). A plain `percentage%` fill stop
	// only meets the thumb at the midpoint and drifts up to half a thumb away elsewhere (the visible gap
	// between the fill and the thumb). Adding `thumb·(0.5 − percentage/100)px` to `percentage%` reproduces the
	// thumb centre: +thumb/2 at 0%, 0 at 50%, −thumb/2 at 100%. The travel-determining width is the rendered
	// thumb box — `hitSize` when the transparent hit ring is in play (it's border-box), otherwise `thumbSize`.
	let fillStop = $derived.by(() => {
		const thumb = hitSize ?? thumbSize
		const offset = thumb * (0.5 - percentage / 100)
		return `calc(${percentage}% ${offset < 0 ? '-' : '+'} ${Math.abs(offset)}px)`
	})

	let backgroundStyle = $derived.by(() => {
		if (bipolar) {
			// Fill outward from the fixed centre (50%) to the thumb-aligned moving edge on whichever side the
			// value sits; the 50% anchor needs no offset because the thumb centre is exactly mid-track there.
			if (percentage <= 50) {
				return `background: linear-gradient(to right, var(--stroke) 0%, var(--stroke) ${fillStop}, var(--brand-primary) ${fillStop}, var(--brand-primary) 50%, var(--stroke) 50%, var(--stroke) 100%)`
			}
			return `background: linear-gradient(to right, var(--stroke) 0%, var(--stroke) 50%, var(--brand-primary) 50%, var(--brand-primary) ${fillStop}, var(--stroke) ${fillStop}, var(--stroke) 100%)`
		}
		return `background: linear-gradient(to right, var(--brand-primary) 0%, var(--brand-primary) ${fillStop}, var(--stroke) ${fillStop}, var(--stroke) 100%)`
	})

	// Thumb sizing handed to the stylesheet as CSS vars. Passing `hitSize` adds the `thumb-hit` class, which
	// swaps in the transparent-ring rules below so the grab area can exceed the painted dot.
	let thumbVars = $derived(
		`--thumb-size: ${thumbSize}px` +
			(hitSize != null ? `; --thumb-hit: ${hitSize}px` : '') +
			(activeScale != null ? `; --thumb-active-scale: ${activeScale}` : '')
	)

	// Flush travel: when the touch ring makes the thumb box (hitSize) larger than the painted dot (thumbSize),
	// the native thumb's travel is inset by hitSize/2 — so the *visible* dot stops short of each track end by
	// (hitSize − thumbSize)/2, leaving the dot floating off the edge and a sliver of fill poking out past it at
	// the extremes. Widen the input by twice that gap and pull it back with a negative margin so the painted
	// dot travels edge-to-edge of the wrapper; the wrapper clips the horizontal overflow (the extra track plus
	// the transparent ring) while leaving the thumb free to overflow vertically. Zero without a hit ring, so
	// the desktop sliders render unchanged.
	let edgeInset = $derived(hitSize != null ? Math.max(0, (hitSize - thumbSize) / 2) : 0)
	let trackStyle = $derived(
		edgeInset > 0 ? `width: calc(100% + ${edgeInset * 2}px); margin-inline: -${edgeInset}px` : ''
	)

	// Grow-on-interaction: while the pointer is held, toggle `thumb-active` so the thumb scales up (see the
	// `thumb-grow` rules below), easing back on release. Driven from JS, not CSS `:active`, because iOS
	// WebKit doesn't reliably apply `:active` to form controls; the window listeners catch a release anywhere.
	let active = $state(false)
	function releaseActive() {
		active = false
		window.removeEventListener('pointerup', releaseActive)
		window.removeEventListener('pointercancel', releaseActive)
	}
	function handlePointerDown(e: PointerEvent) {
		if (activeScale != null) {
			active = true
			window.addEventListener('pointerup', releaseActive)
			window.addEventListener('pointercancel', releaseActive)
		}
		onpointerdown?.(e)
	}
	onDestroy(releaseActive)

	function handleInput(e: Event) {
		if (snapToCenter !== undefined) {
			const mid = (min + max) / 2
			if (Math.abs(value - mid) <= snapToCenter) {
				value = mid
				;(e.target as HTMLInputElement).value = String(mid)
			}
		}
		oninput?.(e)
	}
</script>

<!-- Wrapper clips the horizontal track overflow from the flush-travel widening (see `trackStyle`) while
     leaving the thumb free to overflow vertically out of the thin track. -->
<div class="w-full overflow-x-clip overflow-y-visible">
	<input
		type="range"
		{min}
		{max}
		{step}
		{disabled}
		aria-label={ariaLabel}
		bind:value
		class="h-1.5 w-full cursor-pointer appearance-none rounded-full bg-surface-2 disabled:cursor-not-allowed disabled:opacity-50 {hitSize !=
		null
			? 'thumb-hit'
			: ''} {activeScale != null ? 'thumb-grow' : ''} {active ? 'thumb-active' : ''} {className}"
		style="{backgroundStyle}; {thumbVars}; {trackStyle}"
		oninput={handleInput}
		{onchange}
		onpointerdown={handlePointerDown}
	/>
</div>

<style>
	input[type='range']::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		box-sizing: border-box;
		width: var(--thumb-size, 12px);
		height: var(--thumb-size, 12px);
		border: 1px solid var(--stroke);
		border-radius: 50%;
		background: #ffffff;
		cursor: pointer;
		transition: transform 0.1s;
	}

	input[type='range']::-webkit-slider-thumb:hover {
		transform: scale(1.2);
	}

	input[type='range']::-moz-range-thumb {
		box-sizing: border-box;
		width: var(--thumb-size, 12px);
		height: var(--thumb-size, 12px);
		border-radius: 50%;
		background: #ffffff;
		cursor: pointer;
		border: none;
	}

	/* Touch-target expansion (opt-in via `hitSize`): grow the thumb box to --thumb-hit while a
	   transparent border insets the painted dot back to --thumb-size, so the grab area exceeds the
	   visible thumb. The hairline outline moves to an inset shadow since the outer border is now the
	   transparent ring. Without the `thumb-hit` class the rules above render the default (desktop) thumb. */
	input[type='range'].thumb-hit::-webkit-slider-thumb {
		box-sizing: border-box;
		width: var(--thumb-hit, var(--thumb-size, 12px));
		height: var(--thumb-hit, var(--thumb-size, 12px));
		border-style: solid;
		border-color: transparent;
		border-width: calc((var(--thumb-hit, var(--thumb-size, 12px)) - var(--thumb-size, 12px)) / 2);
		background-clip: padding-box;
		box-shadow: inset 0 0 0 1px var(--stroke);
	}

	input[type='range'].thumb-hit::-moz-range-thumb {
		box-sizing: border-box;
		width: var(--thumb-hit, var(--thumb-size, 12px));
		height: var(--thumb-hit, var(--thumb-size, 12px));
		border-style: solid;
		border-color: transparent;
		border-width: calc((var(--thumb-hit, var(--thumb-size, 12px)) - var(--thumb-size, 12px)) / 2);
		background-clip: padding-box;
		box-shadow: inset 0 0 0 1px var(--stroke);
	}

	/* Grow-on-interaction (opt-in via `activeScale`): the thumb scales up while the pointer is held — the
	   `thumb-active` class is toggled in script — and eases back on release along the app's --ease-fluid curve. */
	input[type='range'].thumb-grow::-webkit-slider-thumb {
		transition: transform 0.15s var(--ease-fluid, cubic-bezier(0.32, 0.72, 0, 1));
	}

	input[type='range'].thumb-grow.thumb-active::-webkit-slider-thumb {
		transform: scale(var(--thumb-active-scale, 1.4));
	}

	input[type='range'].thumb-grow::-moz-range-thumb {
		transition: transform 0.15s var(--ease-fluid, cubic-bezier(0.32, 0.72, 0, 1));
	}

	input[type='range'].thumb-grow.thumb-active::-moz-range-thumb {
		transform: scale(var(--thumb-active-scale, 1.4));
	}
</style>
