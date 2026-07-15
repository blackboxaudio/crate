<script lang="ts">
	import { translate } from '$shared/i18n'
	import type { SortDirection } from '$shared/types'
	import { countActiveViewFilters, type ReleaseViewFilter, type SortOption } from '$lib/utils/listControls'
	import MobileSearchInput from '$lib/components/common/MobileSearchInput.svelte'
	import SortSheet from './SortSheet.svelte'
	import FilterSheet from './FilterSheet.svelte'

	// Slim glass control bar for the detail views (playlist / tag / follow): search + sort + filter
	// over their in-memory release list — the same controls the feed's DiscoveryToolbar offers,
	// minus the add button. Owns only its sheet-open flags; sort/filter state lives with the caller
	// (session-local per view).
	type Props = {
		sortOptions: SortOption[]
		currentSort: { field: string; direction: SortDirection } | null
		onSelectSort: (field: string, direction: SortDirection) => void
		filter: ReleaseViewFilter
		onFilterChange: (filter: ReleaseViewFilter) => void
		/** Render the tags facet (the tag detail omits it — filtering a tag's list by tags is noise). */
		showTags?: boolean
	}
	let { sortOptions, currentSort, onSelectSort, filter, onFilterChange, showTags = true }: Props = $props()

	let sortOpen = $state(false)
	let filterOpen = $state(false)

	const activeFilterCount = $derived(countActiveViewFilters(filter))
	const hasActiveFilters = $derived(activeFilterCount > 0)

	function toggleTag(id: string) {
		onFilterChange({
			...filter,
			tagIds: filter.tagIds.includes(id) ? filter.tagIds.filter((t) => t !== id) : [...filter.tagIds, id],
		})
	}
</script>

<div class="glass flex items-center gap-2 border-b border-stroke-subtle px-3 py-2">
	<MobileSearchInput
		value={filter.search}
		oninput={(v) => onFilterChange({ ...filter, search: v })}
		placeholder={$translate('discovery.searchPlaceholder')}
	/>

	<button
		type="button"
		aria-label={$translate('discovery.sortBy')}
		class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md text-text-secondary active:bg-surface-2"
		onclick={() => (sortOpen = true)}
	>
		<svg
			viewBox="0 0 24 24"
			class="h-5 w-5"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linecap="round"
			stroke-linejoin="round"
		>
			<path d="M3 8l4-4 4 4M7 4v16M21 16l-4 4-4-4M17 20V4" />
		</svg>
	</button>

	<button
		type="button"
		aria-label={$translate('filters.title')}
		class="relative flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md transition-colors {hasActiveFilters
			? 'bg-brand-muted text-brand-primary'
			: 'text-text-secondary active:bg-surface-2'}"
		onclick={() => (filterOpen = true)}
	>
		<svg
			viewBox="0 0 24 24"
			class="h-5 w-5"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linecap="round"
			stroke-linejoin="round"
		>
			<line x1="3" y1="6" x2="21" y2="6" />
			<line x1="6" y1="12" x2="18" y2="12" />
			<line x1="9" y1="18" x2="15" y2="18" />
		</svg>
		{#if hasActiveFilters}
			<span
				class="absolute -top-0.5 -right-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-brand-primary px-1 text-[10px] leading-none font-semibold text-white"
			>
				{activeFilterCount}
			</span>
		{/if}
	</button>
</div>

<SortSheet
	open={sortOpen}
	onClose={() => (sortOpen = false)}
	options={sortOptions}
	current={currentSort}
	onSelect={onSelectSort}
/>
<FilterSheet
	open={filterOpen}
	onClose={() => (filterOpen = false)}
	liked={{ value: filter.likedOnly, onToggle: () => onFilterChange({ ...filter, likedOnly: !filter.likedOnly }) }}
	downloaded={{
		value: filter.downloadedOnly,
		onToggle: () => onFilterChange({ ...filter, downloadedOnly: !filter.downloadedOnly }),
	}}
	tags={showTags
		? {
				activeIds: filter.tagIds,
				mode: filter.tagMode,
				onToggleTag: toggleTag,
				onToggleMode: () => onFilterChange({ ...filter, tagMode: filter.tagMode === 'or' ? 'and' : 'or' }),
			}
		: undefined}
	onClearAll={() => onFilterChange({ ...filter, likedOnly: false, downloadedOnly: false, tagIds: [] })}
/>
