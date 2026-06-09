<script lang="ts">
	type Props = {
		value?: number
		min?: number
		max?: number
		step?: number
		disabled?: boolean
		bipolar?: boolean
		snapToCenter?: number
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
		bipolar = false,
		snapToCenter,
		class: className = '',
		oninput,
		onchange,
	}: Props = $props()

	let percentage = $derived(((value - min) / (max - min)) * 100)

	let backgroundStyle = $derived.by(() => {
		if (bipolar) {
			const mid = 50
			const left = Math.min(percentage, mid)
			const right = Math.max(percentage, mid)
			return `background: linear-gradient(to right, var(--stroke) 0%, var(--stroke) ${left}%, var(--brand-primary) ${left}%, var(--brand-primary) ${right}%, var(--stroke) ${right}%, var(--stroke) 100%)`
		}
		return `background: linear-gradient(to right, var(--brand-primary) 0%, var(--brand-primary) ${percentage}%, var(--stroke) ${percentage}%, var(--stroke) 100%)`
	})

	function applySnap(e: Event): void {
		if (snapToCenter !== undefined) {
			const target = e.target as HTMLInputElement
			const raw = parseFloat(target.value)
			const mid = (min + max) / 2
			if (Math.abs(raw - mid) <= snapToCenter) {
				value = mid
				target.value = String(mid)
			}
		}
	}

	function handleInput(e: Event) {
		applySnap(e)
		oninput?.(e)
	}

	function handleChange(e: Event) {
		applySnap(e)
		onchange?.(e)
	}
</script>

<input
	type="range"
	{min}
	{max}
	{step}
	{disabled}
	bind:value
	class="h-1.5 w-full cursor-pointer appearance-none rounded-full bg-surface-2 disabled:cursor-not-allowed disabled:opacity-50 {className}"
	style={backgroundStyle}
	oninput={handleInput}
	onchange={handleChange}
/>

<style>
	input[type='range']::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		width: 12px;
		height: 12px;
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
		width: 12px;
		height: 12px;
		border-radius: 50%;
		background: #ffffff;
		cursor: pointer;
		border: none;
	}
</style>
