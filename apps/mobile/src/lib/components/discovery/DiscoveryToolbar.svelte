<script lang="ts">
	import { translate } from '$shared/i18n'
	import { discoveryStore, likedOnly } from '$shared/stores/discovery'
	import { mobileUIStore, tagFilterIds } from '$lib/stores/mobileUI'
	import SortSheet from './SortSheet.svelte'
	import FilterSheet from './FilterSheet.svelte'

	// Discovery feed toolbar: search + sort + tag-filter + add. Sits ABOVE the virtualizer's scroll element
	// (not inside it) so the virtualizer's offset math / scroll restoration stay correct. Search and sort
	// drive the shared discovery store (real-time, client-side); the filter button opens the tag chips; the
	// add button opens the add-release sheet (issue #56's flow, placeholder for now).
	let sortOpen = $state(false)
	let filterOpen = $state(false)

	// The search box is a controlled input over the store's `filter.search` — the store is the single source
	// of truth. (A tag assign/remove reloads the feed and resets `filter.search`; binding to the store keeps
	// the box in lock-step instead of showing a stale query the feed no longer applies.)

	// Active-filter count for the trigger badge: tag filters plus the liked-only toggle (mirrors the desktop
	// FilterDropdown badge) so the button reads as "active" whenever any filter is applied.
	const activeFilterCount = $derived($tagFilterIds.length + ($likedOnly ? 1 : 0))
	const hasActiveFilters = $derived(activeFilterCount > 0)
</script>

<div class="flex items-center gap-2 border-b border-stroke-subtle bg-surface-1 px-3 py-2">
	<div class="relative min-w-0 flex-1">
		<svg
			class="pointer-events-none absolute top-1/2 left-2.5 h-4 w-4 -translate-y-1/2 text-text-tertiary"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
		>
			<circle cx="11" cy="11" r="7" />
			<path d="M21 21l-4.3-4.3" stroke-linecap="round" />
		</svg>
		<input
			type="text"
			value={$discoveryStore.filter.search ?? ''}
			oninput={(e) => discoveryStore.setSearch(e.currentTarget.value)}
			placeholder={$translate('discovery.searchPlaceholder')}
			autocapitalize="off"
			autocomplete="off"
			autocorrect="off"
			spellcheck="false"
			class="h-9 w-full rounded-md border border-stroke bg-surface-2 pr-3 pl-8 text-sm text-text-primary transition-colors placeholder:text-text-tertiary focus:border-brand-primary focus:shadow-[0_0_0_3px_var(--brand-muted)]"
		/>
	</div>

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
		aria-label={$translate('discovery.addRelease')}
		class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md bg-brand-primary text-white active:opacity-90"
		onclick={mobileUIStore.openAddRelease}
	>
		<svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2">
			<path d="M12 5v14M5 12h14" stroke-linecap="round" stroke-linejoin="round" />
		</svg>
	</button>
</div>

<SortSheet open={sortOpen} onClose={() => (sortOpen = false)} />
<FilterSheet open={filterOpen} onClose={() => (filterOpen = false)} />
