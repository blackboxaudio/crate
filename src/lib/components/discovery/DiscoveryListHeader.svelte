<script lang="ts">
	import type { DiscoverySortConfig, DiscoverySortField } from '$lib/types'
	import { translate } from '$lib/i18n'
	import Icon from '$lib/components/common/Icon.svelte'
	import { fade } from 'svelte/transition'

	type Props = {
		sortConfig: DiscoverySortConfig
		onSort?: (config: DiscoverySortConfig) => void
	}

	let { sortConfig, onSort }: Props = $props()

	type ExtendedField = DiscoverySortField | 'tags'

	function handleSort(field: ExtendedField) {
		if (field === 'tags') return
		const newConfig: DiscoverySortConfig =
			sortConfig.field === field
				? { field, direction: sortConfig.direction === 'asc' ? 'desc' : 'asc' }
				: { field, direction: 'asc' }
		onSort?.(newConfig)
	}

	type Column = {
		field: ExtendedField | null
		labelKey: string
		align: 'left' | 'center' | 'right'
	}

	const columns: Column[] = [
		{ field: null, labelKey: '', align: 'center' },
		{ field: null, labelKey: '', align: 'center' },
		{ field: 'artist', labelKey: 'discovery.columns.artistTitle', align: 'left' },
		{ field: 'label', labelKey: 'discovery.columns.label', align: 'left' },
		{ field: 'tags', labelKey: 'discovery.columns.tags', align: 'left' },
		{ field: 'source_type', labelKey: 'discovery.source', align: 'left' },
		{ field: 'release_date', labelKey: 'discovery.columns.released', align: 'left' },
		{ field: 'date_added', labelKey: 'discovery.columns.added', align: 'left' },
		{ field: null, labelKey: '', align: 'center' },
	]
</script>

<div
	class="sticky top-0 z-10 grid grid-cols-[24px_40px_1.25fr_0.6fr_1fr_90px_110px_100px_64px] justify-items-start gap-2 border-b border-stroke bg-surface-1/50 px-3 py-2 text-xs font-medium tracking-wider text-text-tertiary uppercase backdrop-blur-sm"
>
	{#each columns as column, index (index)}
		{#if column.field}
			<button
				type="button"
				class="w-full text-left transition-colors hover:text-text-secondary"
				onclick={() => column.field && handleSort(column.field)}
			>
				{column.labelKey ? $translate(column.labelKey) : ''}
				{#if column.field && column.field !== 'tags' && sortConfig.field === column.field}
					<span class="inline-block" transition:fade={{ duration: 50 }}>
						<Icon
							name="chevron-down"
							class="ml-1 inline-block h-3 w-3 align-middle transition-transform {sortConfig.direction === 'asc'
								? 'rotate-180'
								: ''}"
						/>
					</span>
				{/if}
			</button>
		{:else}
			<div></div>
		{/if}
	{/each}
</div>
