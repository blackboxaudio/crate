<script lang="ts">
	type Props = {
		checked?: boolean
		onchange?: () => void
		label?: string
		disabled?: boolean
	}

	let { checked = $bindable(false), onchange, label, disabled = false }: Props = $props()

	function handleChange(event: MouseEvent) {
		event.stopPropagation()
		if (onchange) {
			// Let parent control state via callback
			onchange()
		} else {
			// Only toggle internally when using bind:checked
			checked = !checked
		}
	}
</script>

<label
	class="inline-flex cursor-pointer items-center gap-2"
	class:opacity-50={disabled}
	class:cursor-not-allowed={disabled}
>
	<button
		type="button"
		role="checkbox"
		aria-checked={checked}
		{disabled}
		onclick={handleChange}
		class="flex h-4 w-4 items-center justify-center rounded border transition-colors
			{checked ? 'border-brand-primary bg-brand-primary' : 'border-stroke bg-surface-2 hover:border-text-tertiary'}"
	>
		{#if checked}
			<svg class="h-3 w-3 text-white" viewBox="0 0 12 12" fill="none">
				<path d="M2 6L5 9L10 3" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
			</svg>
		{/if}
	</button>
	{#if label}
		<span class="text-sm text-text-primary select-none">{label}</span>
	{/if}
</label>
