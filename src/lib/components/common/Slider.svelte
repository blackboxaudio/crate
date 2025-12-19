<script lang="ts">
	type Props = {
		value?: number
		min?: number
		max?: number
		step?: number
		disabled?: boolean
		class?: string
		oninput?: (e: Event) => void
		onchange?: (e: Event) => void
	}

	let {
		value = $bindable(0),
		min = 0,
		max = 100,
		step = 1,
		disabled = false,
		class: className = '',
		oninput,
		onchange,
	}: Props = $props()

	let percentage = $derived(((value - min) / (max - min)) * 100)
</script>

<input
	type="range"
	{min}
	{max}
	{step}
	{disabled}
	bind:value
	class="h-1.5 w-full cursor-pointer appearance-none rounded-full bg-surface-2 disabled:cursor-not-allowed disabled:opacity-50 {className}"
	style="background: linear-gradient(to right, var(--brand-primary) 0%, var(--brand-primary) {percentage}%, var(--stroke) {percentage}%, var(--stroke) 100%)"
	{oninput}
	{onchange}
/>

<style>
	input[type='range']::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		width: 12px;
		height: 12px;
		border-radius: 50%;
		background: #ffffff;
		cursor: pointer;
		transition: transform 0.1s;
	}

	input[type='range']::-webkit-slider-thumb:hover {
		transform: scale(1.2);
	}

	input[type='range']::-moz-range-thumb {
		width: 12px;
		height: 12px;
		border-radius: 50%;
		background: #ffffff;
		cursor: pointer;
		border: none;
	}
</style>
