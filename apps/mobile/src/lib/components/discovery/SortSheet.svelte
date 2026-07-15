<script lang="ts">
	import { translate } from '$shared/i18n'
	import type { SortDirection } from '$shared/types'
	import type { SortOption } from '$lib/utils/listControls'
	import { lightTap } from '$lib/utils/haptics'
	import MobileModal from '$lib/components/common/MobileModal.svelte'

	// Generalized sort sheet shared by the discovery feed and the detail views (playlist / tag /
	// follow), the Playlists folder listing, and the Following roster: options + the current
	// selection come in as props. Tapping the active field flips its direction; tapping another
	// switches to it with its default direction. `directionless` options (follow sorts, "Playlist
	// order") render a check instead of the direction arrow and never flip.
	type Props = {
		open: boolean
		onClose: () => void
		options: SortOption[]
		current: { field: string; direction: SortDirection } | null
		onSelect: (field: string, direction: SortDirection) => void
		titleKey?: string
	}
	let { open, onClose, options, current, onSelect, titleKey = 'discovery.sortBy' }: Props = $props()

	function choose(opt: SortOption) {
		void lightTap()
		if (!opt.directionless && current?.field === opt.field) {
			onSelect(opt.field, current.direction === 'asc' ? 'desc' : 'asc')
		} else {
			onSelect(opt.field, opt.defaultDir)
		}
	}
</script>

<MobileModal {open} {onClose} title={$translate(titleKey)}>
	<div class="flex flex-col">
		{#each options as opt (opt.field)}
			{@const active = current?.field === opt.field}
			<button
				type="button"
				class="flex min-h-[44px] items-center justify-between rounded-md px-2 py-2 text-left active:bg-surface-2 {active
					? 'text-brand-primary'
					: 'text-text-primary'}"
				aria-pressed={active}
				onclick={() => choose(opt)}
			>
				<span class="text-sm font-medium">{$translate(opt.labelKey)}</span>
				{#if active && opt.directionless}
					<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M20 6L9 17l-5-5" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
				{:else if active}
					<svg
						class="h-4 w-4 {current?.direction === 'asc' ? 'rotate-180' : ''}"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
					>
						<path d="M12 5v14M19 12l-7 7-7-7" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
				{/if}
			</button>
		{/each}
	</div>
</MobileModal>
