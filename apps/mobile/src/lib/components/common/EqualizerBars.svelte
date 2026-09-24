<script lang="ts">
	// Equalizer bars for the currently-playing track row. Decorative for now: the motion is a CSS
	// keyframe animation, NOT real frequency data. The `levels` prop is the hook for making it accurate
	// later — pass real per-bar magnitudes (e.g. from a spectrum analyser) and the bars render those
	// heights instead, with no call-site restructuring.
	//
	// Every bar has a fixed *rest* height, and the two states blend by amplitude rather than by
	// starting/stopping the animation: `--eq-amp` scales each bar's keyframe range around its rest
	// height, so amp 1 is full bounce and amp 0 is the resting shape at every point in the cycle.
	// Transitioning the amplitude means pausing settles the bars into a deliberate symmetric arch
	// instead of freezing them at whatever random phase they happened to be in (which is what made a
	// single bar read as a collapsed nub). Reduced-motion users get the rest shape, static.
	type Props = {
		class?: string
		/** Animate the bars (true) or settle them into the resting shape (false — e.g. the track is paused). */
		playing?: boolean
		/** Optional real per-bar levels in [0,1]; when set, heights reflect these instead of the animation. */
		levels?: number[]
	}
	let { class: className = 'h-3.5 w-3.5', playing = true, levels }: Props = $props()

	// Per-bar shape and timing. `rest` forms a symmetric arch that reads as an intentional icon when
	// idle; `min`/`max` bound the bounce and never dip low enough for a bar to look broken. Varied
	// durations plus negative delays make the motion organic and already mid-cycle on mount.
	const bars = [
		{ rest: 0.45, min: 0.35, max: 0.85, dur: '520ms', delay: '-140ms' },
		{ rest: 0.85, min: 0.5, max: 1, dur: '700ms', delay: '-380ms' },
		{ rest: 0.85, min: 0.45, max: 1, dur: '430ms', delay: '-90ms' },
		{ rest: 0.45, min: 0.35, max: 0.9, dur: '610ms', delay: '-260ms' },
	]

	// Keep the amplitude transition and the settle timeout in sync.
	const SETTLE_MS = 280

	// Once the bars have finished settling they hold the rest shape at every phase, so the animation
	// can be paused outright — no per-frame keyframe re-resolution while a track sits paused. The
	// keyframes only need to keep running while playing or while a fresh pause eases into the rest
	// arch (`settling` covers the transition window).
	let settling = $state(false)
	$effect(() => {
		if (playing) return
		settling = true
		const timer = setTimeout(() => (settling = false), SETTLE_MS)
		return () => {
			clearTimeout(timer)
			settling = false
		}
	})
	const animating = $derived(playing || settling)

	// Keep a small floor so a bar never fully collapses (a zero-height bar reads as "broken").
	const pct = (level: number | undefined) => Math.round(Math.max(0.12, Math.min(1, level ?? 0)) * 100)
</script>

<div
	class="eq flex items-end justify-center gap-[2px] {className}"
	style="--eq-amp:{playing && levels == null ? 1 : 0};--eq-settle:{SETTLE_MS}ms"
	aria-hidden="true"
>
	{#each bars as bar, i (i)}
		<span
			class="eq-bar w-[2px] flex-none rounded-full bg-brand-primary"
			class:eq-static={levels != null}
			class:eq-paused={!animating}
			style="--eq-rest:{bar.rest};--eq-min:{bar.min};--eq-max:{bar.max};animation-duration:{bar.dur};animation-delay:{bar.delay};{levels !=
			null
				? `height:${pct(levels[i])}%;`
				: ''}"
		></span>
	{/each}
</div>

<style>
	/* Registering the amplitude makes it interpolable, so the bars ease between motion and rest.
	   Without @property support it simply snaps between the two states — still the right shape. */
	@property --eq-amp {
		syntax: '<number>';
		inherits: true;
		initial-value: 0;
	}

	.eq {
		transition: --eq-amp var(--eq-settle) ease;
	}
	.eq-bar {
		height: 100%;
		transform-origin: bottom;
		transform: scaleY(var(--eq-rest));
		animation-name: eq-bounce;
		animation-iteration-count: infinite;
		animation-direction: alternate;
		animation-timing-function: ease-in-out;
	}
	.eq-bar.eq-paused {
		animation-play-state: paused;
	}
	/* Real-data mode: height is driven by the inline style; drop the keyframe animation and just ease
	   between successive values. */
	.eq-bar.eq-static {
		animation: none;
		transform: none;
		transition: height 90ms linear;
	}
	/* Both endpoints collapse onto the rest height as --eq-amp reaches 0, so the bounce fades out
	   into the resting shape rather than stopping wherever it was. */
	@keyframes eq-bounce {
		from {
			transform: scaleY(calc(var(--eq-rest) + (var(--eq-min) - var(--eq-rest)) * var(--eq-amp)));
		}
		to {
			transform: scaleY(calc(var(--eq-rest) + (var(--eq-max) - var(--eq-rest)) * var(--eq-amp)));
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.eq-bar:not(.eq-static) {
			animation: none;
			transform: scaleY(var(--eq-rest));
		}
	}
</style>
