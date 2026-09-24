<script lang="ts">
	import { translate } from '$shared/i18n'
	import { discoveryStore, facetFilters } from '$shared/stores/discovery'
	import { hasLinkedCollection } from '$shared/stores/collection'
	import type { DiscoverySortField } from '$shared/types'
	import { countActiveFacets } from '$shared/utils/discoveryFilters'
	import { discoveryViewMode, mobileUIStore, tagFilterIds, tagFilterMode } from '$lib/stores/mobileUI'
	import { releaseSortOptions } from '$lib/utils/listControls'
	import MobileSearchInput from '$lib/components/common/MobileSearchInput.svelte'
	import SortSheet from './SortSheet.svelte'
	import FilterSheet from './FilterSheet.svelte'

	// Discovery feed toolbar: search + sort + tag-filter + add. A glass bar the parent overlays on the top of
	// the feed (absolutely positioned), so the release rows scroll behind it and show through the blur
	// (matching the mini-player's material). Search and sort drive the shared discovery store (real-time,
	// client-side); the filter button opens the tag chips; the add button opens the add-release sheet
	// (issue #56's flow, placeholder for now).
	let sortOpen = $state(false)
	let filterOpen = $state(false)

	// The search box is a controlled input over the store's `filter.search` — the store is the single source
	// of truth. (A tag assign/remove reloads the feed and resets `filter.search`; binding to the store keeps
	// the box in lock-step instead of showing a stale query the feed no longer applies.)

	// Active-filter count for the trigger badge: tag filters plus the liked/new/downloaded/purchased toggles
	// (mirrors the desktop FilterDropdown badge) so the button reads as "active" whenever any filter is applied.
	const activeFilterCount = $derived($tagFilterIds.length + countActiveFacets($facetFilters))
	const hasActiveFilters = $derived(activeFilterCount > 0)
</script>

<div class="glass flex items-center gap-2 border-b border-stroke-subtle px-3 py-2">
	<MobileSearchInput
		value={$discoveryStore.filter.search ?? ''}
		oninput={(v) => discoveryStore.setSearch(v)}
		placeholder={$translate('discovery.searchPlaceholder')}
	/>

	<button
		type="button"
		aria-label={$translate('discovery.sortBy')}
		class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md text-text-secondary active:bg-surface-2"
		onclick={() => (sortOpen = true)}
	>
		<!-- Generic (non-directional) sort glyph — the direction is shown per-field inside the sort sheet. -->
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
		<!-- Same filter glyph as desktop (Icon `filter`). -->
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

	<button
		type="button"
		aria-label={$translate($discoveryViewMode === 'grid' ? 'discovery.viewAsList' : 'discovery.viewAsGrid')}
		class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md text-text-secondary active:bg-surface-2"
		onclick={() => mobileUIStore.setDiscoveryViewMode($discoveryViewMode === 'grid' ? 'list' : 'grid')}
	>
		{#if $discoveryViewMode === 'grid'}
			<!-- Currently grid → offer list -->
			<svg
				viewBox="0 0 24 24"
				class="h-5 w-5"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
			>
				<path d="M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01" />
			</svg>
		{:else}
			<!-- Currently list → offer grid -->
			<svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2">
				<rect x="3" y="3" width="7" height="7" rx="1" />
				<rect x="14" y="3" width="7" height="7" rx="1" />
				<rect x="3" y="14" width="7" height="7" rx="1" />
				<rect x="14" y="14" width="7" height="7" rx="1" />
			</svg>
		{/if}
	</button>

	<button
		type="button"
		aria-label={$translate('discovery.addRelease')}
		class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md bg-brand-primary text-white active:opacity-90"
		onclick={mobileUIStore.openAddRelease}
	>
		<svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2">
			<path d="M12 5v14M5 12h14" stroke-linecap="round" stroke-linejoin="round" />
		</svg>
	</button>
</div>

<SortSheet
	open={sortOpen}
	onClose={() => (sortOpen = false)}
	options={releaseSortOptions($facetFilters)}
	current={$discoveryStore.sort}
	onSelect={(field, direction) => discoveryStore.setSort({ field: field as DiscoverySortField, direction })}
/>
<FilterSheet
	open={filterOpen}
	onClose={() => (filterOpen = false)}
	liked={{ value: $facetFilters.liked, onChange: (s) => discoveryStore.setFacetFilter('liked', s) }}
	newReleases={{ value: $facetFilters.new, onChange: (s) => discoveryStore.setFacetFilter('new', s) }}
	downloaded={{ value: $facetFilters.downloaded, onChange: (s) => discoveryStore.setFacetFilter('downloaded', s) }}
	purchased={$hasLinkedCollection
		? { value: $facetFilters.purchased, onChange: (s) => discoveryStore.setFacetFilter('purchased', s) }
		: undefined}
	purchasedSetup={$hasLinkedCollection
		? undefined
		: () => {
				filterOpen = false
				mobileUIStore.openSettings('collection')
			}}
	tags={{
		activeIds: $tagFilterIds,
		mode: $tagFilterMode,
		onToggleTag: (id) => mobileUIStore.toggleTagFilter(id),
		onToggleMode: mobileUIStore.toggleTagFilterMode,
	}}
	onClearAll={() => {
		discoveryStore.clearFacetFilters()
		mobileUIStore.clearTagFilters()
	}}
/>
