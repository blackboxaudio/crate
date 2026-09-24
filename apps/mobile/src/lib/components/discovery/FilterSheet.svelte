<script lang="ts">
	import type { Snippet } from 'svelte'
	import { fade } from 'svelte/transition'
	import { translate } from '$shared/i18n'
	import { tagsStore } from '$shared/stores/tags'
	import type { DiscoveryFacet, FilterTriState, TagFilterMode } from '$shared/types'
	import { cycleTriState } from '$shared/utils/discoveryFilters'
	import TriStateControl from '$shared/components/TriStateControl.svelte'
	import { lightTap } from '$lib/utils/haptics'
	import MobileModal from '$lib/components/common/MobileModal.svelte'

	// Generalized filter sheet (liked / new / downloaded / purchased tri-state facets + tag chips with
	// AND/OR matching). Fully controlled: each facet renders only when its prop is provided, so the feed
	// and each detail view opt into exactly the filters that make sense there (the tag detail omits
	// tags, etc.). Mirrors the desktop FilterDropdown: the same Off | Only | Not segmented control per
	// facet, with the row label cycling through the states.
	type FacetProp = { value: FilterTriState; onChange: (state: FilterTriState) => void }

	type Props = {
		open: boolean
		onClose: () => void
		liked?: FacetProp
		/** New facet: releases surfaced by a followed source and not yet reviewed. */
		newReleases?: FacetProp
		downloaded?: FacetProp
		/** Purchased facet, shown when a collection account is linked. */
		purchased?: FacetProp
		/** No account linked yet: render a "link your collection" action row instead of the
		 *  toggle (the feature's discoverable entry point — jumps to Settings → Collection). */
		purchasedSetup?: () => void
		tags?: {
			activeIds: string[]
			mode: TagFilterMode
			onToggleTag: (id: string) => void
			onToggleMode: () => void
		}
		onClearAll: () => void
	}
	let { open, onClose, liked, newReleases, downloaded, purchased, purchasedSetup, tags, onClearAll }: Props = $props()

	// Lazy-load categories the first time the sheet opens (only when the tags facet is shown).
	let loadedOnce = $state(false)
	$effect(() => {
		if (open && tags && !loadedOnce) {
			loadedOnce = true
			void tagsStore.load()
		}
	})

	type FacetRow = { key: DiscoveryFacet; labelKey: string; facet: FacetProp; icon: Snippet<[FilterTriState]> }
	const facetRows = $derived(
		(
			[
				liked && { key: 'liked', labelKey: 'filters.liked', facet: liked, icon: heartIcon },
				newReleases && { key: 'new', labelKey: 'filters.new', facet: newReleases, icon: rssIcon },
				downloaded && { key: 'downloaded', labelKey: 'filters.downloaded', facet: downloaded, icon: downloadIcon },
				purchased && { key: 'purchased', labelKey: 'filters.purchased', facet: purchased, icon: bagIcon },
			] as (FacetRow | false | undefined)[]
		).filter((row): row is FacetRow => !!row)
	)

	const active = $derived(new Set(tags?.activeIds ?? []))
	const hasActiveFilters = $derived(active.size > 0 || facetRows.some((row) => row.facet.value !== 'off'))

	// Selection tick: filters commit instantly (no confirm step), so acknowledge each toggle the way
	// the app's other sheets do (playlist picker, queue actions, sort options).
	function tick(fn: () => void) {
		void lightTap()
		fn()
	}
</script>

{#snippet heartIcon(state: FilterTriState)}
	<svg
		class="h-4 w-4 {state === 'include' ? 'text-brand-primary' : 'text-text-tertiary'}"
		viewBox="0 0 24 24"
		fill={state === 'include' ? 'currentColor' : 'none'}
		stroke="currentColor"
		stroke-width="2"
	>
		<path
			d="M20.84 4.61a5.5 5.5 0 00-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 00-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 000-7.78z"
		/>
	</svg>
{/snippet}

<!-- New: releases a followed source surfaced that haven't been reviewed yet (the same `is_new` flag the
     release rows badge). -->
{#snippet rssIcon(state: FilterTriState)}
	<svg
		class="h-4 w-4 {state === 'include' ? 'text-brand-primary' : 'text-text-tertiary'}"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
	>
		<path d="M4 11a9 9 0 0 1 9 9" />
		<path d="M4 4a16 16 0 0 1 16 16" />
		<circle cx="5" cy="19" r="1" fill="currentColor" stroke="none" />
	</svg>
{/snippet}

<!-- Downloaded: releases whose audio is fully cached, i.e. playable in airplane mode. -->
{#snippet downloadIcon(state: FilterTriState)}
	<svg
		class="h-4 w-4 {state === 'include' ? 'text-brand-primary' : 'text-text-tertiary'}"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
	>
		<circle cx="12" cy="12" r="9" />
		<path d="M12 8v7M8.5 12l3.5 3.5L15.5 12" />
	</svg>
{/snippet}

<!-- Purchased: releases owned in the linked Bandcamp collection(s). -->
{#snippet bagIcon(state: FilterTriState)}
	<svg
		class="h-4 w-4 {state === 'include' ? 'text-brand-primary' : 'text-text-tertiary'}"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
	>
		<path d="M6 8h12l-1.2 12H7.2L6 8z" />
		<path d="M9 8V6a3 3 0 0 1 6 0v2" />
	</svg>
{/snippet}

<MobileModal {open} {onClose} title={$translate('filters.title')}>
	{#snippet headerAction()}
		{#if hasActiveFilters}
			<button
				type="button"
				class="text-sm font-medium text-danger active:opacity-70"
				transition:fade={{ duration: 120 }}
				onclick={onClearAll}
			>
				{$translate('library.clearAll')}
			</button>
		{/if}
	{/snippet}

	<div class="flex flex-col gap-4">
		<!-- Facet rows: tapping anywhere on the row cycles off → include → exclude; the segmented control inside
		     sets a state directly (and stops the tap so it doesn't also cycle). The row is a role="button" div
		     rather than a <button> because it contains the control's own buttons. -->
		{#each facetRows as row (row.key)}
			{@const state = row.facet.value}
			{@const cycle = () => tick(() => row.facet.onChange(cycleTriState(state)))}
			<div
				role="button"
				tabindex="0"
				class="flex w-full items-center justify-between gap-3 rounded-md py-1 hover:cursor-pointer active:bg-surface-2"
				onclick={cycle}
				onkeydown={(e) => {
					if (e.key === 'Enter' || e.key === ' ') {
						e.preventDefault()
						cycle()
					}
				}}
			>
				<span
					class="flex min-w-0 items-center gap-2 text-sm font-medium {state === 'include'
						? 'text-brand-primary'
						: state === 'exclude'
							? 'text-text-secondary line-through'
							: 'text-text-primary'}"
				>
					{@render row.icon(state)}
					{$translate(row.labelKey)}
				</span>
				<TriStateControl
					value={state}
					onChange={(s) => tick(() => row.facet.onChange(s))}
					label={$translate(row.labelKey)}
					size="md"
				/>
			</div>
		{/each}

		{#if purchasedSetup}
			<!-- Not linked yet: the Purchased slot doubles as the feature's front door. -->
			<button
				type="button"
				class="flex w-full items-center justify-between rounded-md py-1 active:bg-surface-2"
				onclick={() => tick(purchasedSetup)}
			>
				<span class="flex items-center gap-2 text-sm font-medium text-text-primary">
					<svg
						class="h-4 w-4 text-text-tertiary"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
					>
						<path d="M6 8h12l-1.2 12H7.2L6 8z" />
						<path d="M9 8V6a3 3 0 0 1 6 0v2" />
					</svg>
					{$translate('settings.collection.linkAccount')}
				</span>
				<svg
					class="h-3.5 w-3.5 text-text-tertiary"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2.5"
				>
					<path d="M9 18l6-6-6-6" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			</button>
		{/if}

		{#if tags}
			{#if $tagsStore.loading && $tagsStore.categories.length === 0}
				<p class="py-2 text-center text-sm text-text-secondary">{$translate('common.loading')}</p>
			{:else if $tagsStore.categories.length > 0}
				<div class="border-t border-stroke-subtle"></div>

				<!-- Tag matching mode: a segmented OR | AND control (the desktop FilterDropdown pattern), disabled
				     until 2+ tags are active since the mode only matters with multiple tags. -->
				<div class="flex items-center justify-between">
					<span class="flex items-center gap-2 text-sm font-medium text-text-primary">
						<svg
							class="h-4 w-4 text-text-tertiary"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="2"
							stroke-linecap="round"
							stroke-linejoin="round"
						>
							<path
								d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A2 2 0 013 12V7a4 4 0 014-4z"
							/>
						</svg>
						{$translate('library.matching')}
					</span>
					<button
						type="button"
						class="relative grid grid-cols-2 rounded-full border border-stroke bg-surface-2 p-0.5 text-xs font-medium transition-opacity {active.size <
						2
							? 'pointer-events-none opacity-40'
							: ''}"
						disabled={active.size < 2}
						onclick={() => tick(tags.onToggleMode)}
					>
						<span
							class="absolute inset-y-0.5 rounded-full bg-brand-primary transition-all duration-200 ease-out"
							style="left: {tags.mode === 'or' ? '2px' : '50%'}; right: {tags.mode === 'or' ? '50%' : '2px'}"
						></span>
						<span
							class="relative z-10 rounded-full px-3 py-1 text-center transition-colors {tags.mode === 'or'
								? 'text-white'
								: 'text-text-tertiary'}"
						>
							{$translate('library.matchOr')}
						</span>
						<span
							class="relative z-10 rounded-full px-3 py-1 text-center transition-colors {tags.mode === 'and'
								? 'text-white'
								: 'text-text-tertiary'}"
						>
							{$translate('library.matchAnd')}
						</span>
					</button>
				</div>

				<!-- Tag chips by category. -->
				<div class="flex flex-col gap-5">
					{#each $tagsStore.categories as category (category.id)}
						<div>
							<h3 class="mb-2 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
								{category.name}
							</h3>
							<div class="flex flex-wrap gap-2">
								{#each category.tags as tag (tag.id)}
									{@const color = tag.color ?? category.color ?? '#888888'}
									{@const on = active.has(tag.id)}
									<button
										type="button"
										class="inline-flex items-center gap-1 rounded-md px-3 py-2 text-sm font-medium transition-colors {on
											? ''
											: 'border border-stroke bg-surface-2 text-text-secondary'}"
										style={on ? `background-color: ${color}20; color: ${color}; border: 1px solid ${color}40;` : ''}
										aria-pressed={on}
										onclick={() => tick(() => tags.onToggleTag(tag.id))}
									>
										{#if on}
											<svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
												<path d="M20 6L9 17l-5-5" stroke-linecap="round" stroke-linejoin="round" />
											</svg>
										{/if}
										{tag.name}
									</button>
								{/each}
							</div>
						</div>
					{/each}
				</div>
			{/if}
		{/if}
	</div>
</MobileModal>
