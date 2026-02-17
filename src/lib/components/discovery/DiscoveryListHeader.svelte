<script lang="ts">
	import type { DiscoverySortConfig, DiscoverySortField } from '$lib/types'
	import { translate } from '$lib/i18n'

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

	function getSortIndicator(field: ExtendedField): string {
		if (field === 'tags' || sortConfig.field !== field) return ''
		return sortConfig.direction === 'asc' ? ' \u2191' : ' \u2193'
	}

	type Column = {
		field: ExtendedField | null
		labelKey: string
		align: 'left' | 'center' | 'right'
	}

	const columns: Column[] = [
		{ field: null, labelKey: '', align: 'center' },
		{ field: 'artist', labelKey: 'discovery.columns.artistTitle', align: 'left' },
		{ field: 'label', labelKey: 'discovery.columns.label', align: 'left' },
		{ field: 'status', labelKey: 'discovery.columns.status', align: 'center' },
		{ field: 'tags', labelKey: 'discovery.columns.tags', align: 'left' },
		{ field: 'date_added', labelKey: 'discovery.columns.dateAdded', align: 'right' },
	]
</script>

<div
	class="sticky top-0 z-10 grid grid-cols-[40px_1fr_1fr_100px_1fr_100px] gap-2 border-b border-stroke bg-surface-1/50 px-3 py-2 text-xs font-medium tracking-wider text-text-tertiary uppercase backdrop-blur-sm"
>
	{#each columns as column, index (index)}
		{#if column.field}
			<button
				type="button"
				class="text-{column.align} transition-colors hover:text-text-secondary"
				onclick={() => column.field && handleSort(column.field)}
			>
				{column.labelKey ? $translate(column.labelKey) : ''}{getSortIndicator(column.field)}
			</button>
		{:else}
			<div class="text-{column.align}">
				{column.labelKey ? $translate(column.labelKey) : ''}
			</div>
		{/if}
	{/each}
</div>
