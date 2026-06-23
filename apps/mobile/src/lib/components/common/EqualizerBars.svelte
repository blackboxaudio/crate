<script lang="ts">
	// Equalizer bars for the currently-playing track row. Decorative for now: the motion is a CSS
	// keyframe animation, NOT real frequency data. The `levels` prop is the hook for making it accurate
	// later — pass real per-bar magnitudes (e.g. from a spectrum analyser) and the bars render those
	// heights instead, with no call-site restructuring. When `playing` is false the bars freeze in place
	// (paused track); reduced-motion users get static mid-height bars.
	type Props = {
		class?: string
		/** Animate the bars (true) or freeze them in place (false — e.g. the track is paused). */
		playing?: boolean
		/** Optional real per-bar levels in [0,1]; when set, heights reflect these instead of the animation. */
		levels?: number[]
	}
	let { class: className = 'h-3.5 w-3.5', playing = true, levels }: Props = $props()

	// Per-bar timing — varied durations/offsets make the motion read as organic rather than a single
	// synchronized bounce. Four bars fit the ~14px track-number column.
	const timings = [
		{ dur: '520ms', delay: '0ms' },
		{ dur: '700ms', delay: '180ms' },
		{ dur: '430ms', delay: '90ms' },
		{ dur: '610ms', delay: '300ms' },
	]

	// Keep a small floor so a bar never fully collapses (a zero-height bar reads as "broken").
	const pct = (level: number | undefined) => Math.round(Math.max(0.12, Math.min(1, level ?? 0)) * 100)
</script>

<div class="flex items-end justify-center gap-[2px] {className}" aria-hidden="true">
	{#each timings as t, i (i)}
		<span
			class="eq-bar w-[2px] flex-none rounded-full bg-brand-primary"
			class:eq-static={levels != null}
			style="animation-duration:{t.dur};animation-delay:{t.delay};animation-play-state:{playing
				? 'running'
				: 'paused'};{levels != null ? `height:${pct(levels[i])}%;` : ''}"
		></span>
	{/each}
</div>

<style>
	.eq-bar {
		height: 100%;
		transform-origin: bottom;
		animation-name: eq-bounce;
		animation-iteration-count: infinite;
		animation-direction: alternate;
		animation-timing-function: ease-in-out;
	}
	/* Real-data mode: height is driven by the inline style; drop the keyframe animation and just ease
	   between successive values. */
	.eq-bar.eq-static {
		animation: none;
		transform: none;
		transition: height 90ms linear;
	}
	@keyframes eq-bounce {
		from {
			transform: scaleY(0.18);
		}
		to {
			transform: scaleY(1);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.eq-bar:not(.eq-static) {
			animation: none;
			transform: scaleY(0.55);
		}
	}
</style>
