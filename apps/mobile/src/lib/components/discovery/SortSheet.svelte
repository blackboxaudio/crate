<script lang="ts">
	import { translate } from '$shared/i18n'
	import type { DiscoverySortField, SortDirection } from '$shared/types'
	import { discoveryStore } from '$shared/stores/discovery'
	import MobileModal from '$lib/components/common/MobileModal.svelte'

	// Sort options for the discovery feed (issue #53 scope: the four fields below). Drives the shared
	// discovery store's `setSort`, which the client-side `sortedReleases` derived applies in-memory. Tapping
	// the already-active field flips its direction; tapping another switches to it with a sensible default.
	type Props = { open: boolean; onClose: () => void }
	let { open, onClose }: Props = $props()

	const options: { field: DiscoverySortField; labelKey: string; defaultDir: SortDirection }[] = [
		{ field: 'date_added', labelKey: 'discovery.columns.dateAdded', defaultDir: 'desc' },
		{ field: 'artist', labelKey: 'discovery.editor.artist', defaultDir: 'asc' },
		{ field: 'title', labelKey: 'discovery.editor.title', defaultDir: 'asc' },
		{ field: 'label', labelKey: 'discovery.editor.label', defaultDir: 'asc' },
	]

	const sort = $derived($discoveryStore.sort)

	function choose(field: DiscoverySortField, defaultDir: SortDirection) {
		if (sort.field === field) {
			discoveryStore.setSort({ field, direction: sort.direction === 'asc' ? 'desc' : 'asc' })
		} else {
			discoveryStore.setSort({ field, direction: defaultDir })
		}
	}
</script>

<MobileModal {open} {onClose} title={$translate('discovery.sortBy')}>
	<div class="flex flex-col">
		{#each options as opt (opt.field)}
			{@const active = sort.field === opt.field}
			<button
				type="button"
				class="flex min-h-[44px] items-center justify-between rounded-md px-2 py-2 text-left active:bg-surface-2 {active
					? 'text-brand-primary'
					: 'text-text-primary'}"
				aria-pressed={active}
				onclick={() => choose(opt.field, opt.defaultDir)}
			>
				<span class="text-sm font-medium">{$translate(opt.labelKey)}</span>
				{#if active}
					<svg
						class="h-4 w-4 {sort.direction === 'asc' ? 'rotate-180' : ''}"
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
