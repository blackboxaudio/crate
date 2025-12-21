<script lang="ts">
	import type { SortConfig, TrackSortField } from '$lib/types'
	import { getNextSortConfig } from '$lib/utils'
	import { translate } from '$lib/i18n'

	type Props = {
		sortConfig: SortConfig
		onSort?: (config: SortConfig) => void
	}

	let { sortConfig, onSort }: Props = $props()

	function handleSort(field: ExtendedTrackSortField) {
		if (field !== 'tags') {
			const newConfig = getNextSortConfig(sortConfig, field)
			onSort?.(newConfig)
		}
	}

	function getSortIndicator(field: ExtendedTrackSortField): string {
		if (field === 'tags' || sortConfig.field !== field) return ''
		return sortConfig.direction === 'asc' ? ' ↑' : ' ↓'
	}

	type ExtendedTrackSortField = TrackSortField | 'tags'

	type Column = {
		field: ExtendedTrackSortField | null
		labelKey: string
		align: 'left' | 'center' | 'right'
	}

	const columns: Column[] = [
		{ field: 'color', labelKey: '', align: 'center' }, // Color column (sortable)
		{ field: null, labelKey: '', align: 'center' }, // Artwork column (non-sortable)
		{ field: 'title', labelKey: 'library.columns.title', align: 'left' },
		{ field: 'artist', labelKey: 'library.columns.artist', align: 'left' },
		{ field: 'bpm', labelKey: 'library.columns.bpm', align: 'right' },
		{ field: 'key', labelKey: 'library.columns.key', align: 'center' },
		{ field: 'duration_ms', labelKey: 'library.columns.time', align: 'right' },
		{ field: 'tags', labelKey: 'library.columns.tags', align: 'left' },
	]
</script>

<div
	class="sticky top-0 z-10 grid grid-cols-[24px_40px_1fr_1fr_80px_60px_80px_1fr] gap-2 border-b border-stroke bg-surface-1/50 px-3 py-2 text-xs font-medium tracking-wider text-text-tertiary uppercase backdrop-blur-sm"
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
