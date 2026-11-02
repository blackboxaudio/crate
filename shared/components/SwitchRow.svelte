<script lang="ts">
	// Shared on/off switch row for a preference that lives beside the filters ("Apply to playback" in the
	// desktop FilterDropdown and the mobile FilterSheet). Deliberately a switch, not a TriStateControl:
	// it is a setting, not a filter, and must not read as one. Theme tokens + props only, so it stays
	// platform-agnostic.
	type Props = {
		checked: boolean
		onChange: (checked: boolean) => void
		label: string
		hint?: string
		/** `sm` matches the desktop popover's text-xs rows; `md` the mobile sheet's touch targets. */
		size?: 'sm' | 'md'
	}
	let { checked, onChange, label, hint, size = 'sm' }: Props = $props()

	const rowClass = $derived(
		size === 'sm' ? 'rounded px-2 py-1.5 hover:bg-surface-2' : 'rounded-md py-1 active:bg-surface-2'
	)
	const labelClass = $derived(size === 'sm' ? 'text-xs' : 'text-sm font-medium')
	const hintClass = $derived(size === 'sm' ? 'text-[11px]' : 'text-xs')
	const trackClass = $derived(size === 'sm' ? 'h-4 w-7' : 'h-6 w-10')
	const knobClass = $derived(size === 'sm' ? 'h-3 w-3' : 'h-5 w-5')
	const travelClass = $derived(size === 'sm' ? 'translate-x-3' : 'translate-x-4')
</script>

<button
	type="button"
	role="switch"
	aria-checked={checked}
	class="flex w-full items-center justify-between gap-3 text-left transition-colors hover:cursor-pointer {rowClass}"
	onclick={() => onChange(!checked)}
>
	<span class="flex min-w-0 flex-col gap-0.5">
		<span class="text-text-primary {labelClass}">{label}</span>
		{#if hint}
			<span class="text-text-tertiary {hintClass}">{hint}</span>
		{/if}
	</span>
	<span
		class="inline-flex shrink-0 items-center rounded-full p-0.5 transition-colors {trackClass} {checked
			? 'bg-brand-primary'
			: 'bg-stroke'}"
	>
		<span
			class="rounded-full bg-white shadow transition-transform duration-200 ease-out motion-reduce:transition-none {knobClass} {checked
				? travelClass
				: 'translate-x-0'}"
		></span>
	</span>
</button>
