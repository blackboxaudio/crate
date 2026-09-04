<script lang="ts">
	import type { FilterTriState } from '../types'
	import { translate } from '../i18n'

	// Shared three-way segmented control (Off | Only | Not) for the discovery facet filters, used by the
	// desktop FilterDropdown and the mobile FilterSheet. It extends the OR | AND segmented pattern both
	// already use, so a filter shows all three of its states at a glance — the third state is discoverable
	// without a tap-cycle or a long-press. Each segment sets its state explicitly and swallows the click,
	// because the host row around it cycles on click. Theme tokens + props only, so it stays
	// platform-agnostic.
	type Props = {
		value: FilterTriState
		onChange: (state: FilterTriState) => void
		/** Accessible group name — the facet's label (e.g. "Liked"). */
		label: string
		/** `sm` matches the desktop popover's text-xs rows; `md` the mobile sheet's touch targets. */
		size?: 'sm' | 'md'
		disabled?: boolean
	}
	let { value, onChange, label, size = 'sm', disabled = false }: Props = $props()

	const ORDER: FilterTriState[] = ['off', 'include', 'exclude']
	const LABEL_KEYS: Record<FilterTriState, string> = {
		off: 'filters.off',
		include: 'filters.only',
		exclude: 'filters.not',
	}
	const index = $derived(ORDER.indexOf(value))
	const segmentPadding = $derived(size === 'sm' ? 'px-2 py-0.5' : 'px-3 py-1')
	// The pill sits inside the p-0.5 (2px) inset, so each third is measured against the inner width.
	const pillColor = $derived(
		value === 'off' ? 'bg-stroke' : value === 'include' ? 'bg-brand-primary' : 'bg-text-secondary'
	)
</script>

<div
	role="radiogroup"
	aria-label={label}
	class="relative grid shrink-0 grid-cols-3 rounded-full border border-stroke bg-surface-2 p-0.5 text-xs font-medium transition-opacity {disabled
		? 'pointer-events-none opacity-40'
		: ''}"
>
	<span
		class="absolute inset-y-0.5 rounded-full transition-all duration-200 ease-out motion-reduce:transition-none {pillColor}"
		style="left: calc(2px + {index} * (100% - 4px) / 3); width: calc((100% - 4px) / 3)"
	></span>
	{#each ORDER as state (state)}
		<button
			type="button"
			role="radio"
			aria-checked={value === state}
			{disabled}
			class="relative z-10 rounded-full text-center transition-colors hover:cursor-pointer {segmentPadding} {value ===
			state
				? state === 'off'
					? 'text-text-primary'
					: 'text-white'
				: 'text-text-tertiary hover:text-text-secondary'}"
			onclick={(e) => {
				e.stopPropagation()
				if (value !== state) onChange(state)
			}}
		>
			{$translate(LABEL_KEYS[state])}
		</button>
	{/each}
</div>
