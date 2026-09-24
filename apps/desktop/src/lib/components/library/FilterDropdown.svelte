<script lang="ts">
	import type { DiscoveryFacet, FilterTriState, Tag, TagCategory, TagFilterMode } from '$shared/types'
	import { cycleTriState } from '$shared/utils/discoveryFilters'
	import TriStateControl from '$shared/components/TriStateControl.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import Button from '$lib/components/common/Button.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import { translate } from '$shared/i18n'
	import { scale, slide, fade } from 'svelte/transition'

	/** A discovery facet (Liked / New / …): its tri-state and the setter. A facet renders only when
	 *  its prop is provided, so each host opts into exactly the filters that make sense there. */
	type FacetProp = { value: FilterTriState; onChange: (state: FilterTriState) => void }

	type Props = {
		activeFilterTags: Tag[]
		tagCategories: TagCategory[]
		tagColors: Map<string, string | null>
		tagFilterMode: TagFilterMode
		onToggleTagFilter: (tagId: string) => void
		onClearAll: () => void
		onToggleTagFilterMode: () => void
		liked?: FacetProp
		/** Releases surfaced by a followed source and not yet reviewed. */
		newReleases?: FacetProp
		/** Owned in the linked purchase collection(s). */
		purchased?: FacetProp
		/** No collection account linked yet: render a "link your collection" action row
		 *  instead of the control (the feature's discoverable entry point). */
		onSetupPurchased?: () => void
		/** Every track's audio cached on disk. */
		downloaded?: FacetProp
	}

	let {
		activeFilterTags,
		tagCategories,
		tagColors,
		tagFilterMode,
		onToggleTagFilter,
		onClearAll,
		onToggleTagFilterMode,
		liked,
		newReleases,
		purchased,
		onSetupPurchased,
		downloaded,
	}: Props = $props()

	type FacetRow = { key: DiscoveryFacet; icon: string; labelKey: string; facet: FacetProp }
	// The unlinked-collection CTA replaces the Purchased control, so that facet drops out of the rows.
	const facetRows = $derived(
		(
			[
				liked && { key: 'liked', icon: 'heart', labelKey: 'filters.liked', facet: liked },
				newReleases && { key: 'new', icon: 'rss', labelKey: 'filters.new', facet: newReleases },
				purchased &&
					!onSetupPurchased && {
						key: 'purchased',
						icon: 'shopping-bag',
						labelKey: 'filters.purchased',
						facet: purchased,
					},
				downloaded && { key: 'downloaded', icon: 'download', labelKey: 'filters.downloaded', facet: downloaded },
			] as (FacetRow | false | undefined)[]
		).filter((row): row is FacetRow => !!row)
	)
	const hasFacetRows = $derived(facetRows.length > 0 || !!onSetupPurchased)

	const allTags = $derived(tagCategories.flatMap((c) => c.tags))
	const activeTagIds = $derived(new Set(activeFilterTags.map((t) => t.id)))

	// Count active tags per category
	const activeCounts = $derived(
		new Map(tagCategories.map((c) => [c.id, c.tags.filter((t) => activeTagIds.has(t.id)).length]))
	)

	let open = $state(false)
	let triggerEl: HTMLButtonElement | undefined = $state()
	let popoverEl: HTMLDivElement | undefined = $state()
	let flyoutEl: HTMLDivElement | undefined = $state()

	const badgeCount = $derived(activeFilterTags.length + facetRows.filter((row) => row.facet.value !== 'off').length)
	const hasActiveFilters = $derived(badgeCount > 0)

	// =========================================================================
	// Main popover positioning
	// =========================================================================

	const GAP = 4
	const FLYOUT_GAP = 2
	const VIEWPORT_PADDING = 8
	let popoverStyle = $state('')
	let flyoutStyle = $state('')
	let rafId: number | undefined

	function portal(node: HTMLElement) {
		const dialog = triggerEl?.closest('dialog')
		const target = dialog ?? document.body
		target.appendChild(node)
		return {
			destroy() {
				node.remove()
			},
		}
	}

	function computePosition() {
		if (!triggerEl || !popoverEl) return

		const triggerRect = triggerEl.getBoundingClientRect()
		const popoverHeight = popoverEl.offsetHeight
		const popoverWidth = popoverEl.offsetWidth
		const viewportHeight = window.innerHeight
		const viewportWidth = window.innerWidth
		const spaceBelow = viewportHeight - triggerRect.bottom
		const openUpward = spaceBelow < popoverHeight + GAP && triggerRect.top > popoverHeight + GAP

		let top: number
		if (openUpward) {
			top = triggerRect.top - popoverHeight - GAP
		} else {
			top = triggerRect.bottom + GAP
		}
		top = Math.max(VIEWPORT_PADDING, Math.min(top, viewportHeight - popoverHeight - VIEWPORT_PADDING))

		let left: number
		if (popoverWidth > 0) {
			left = triggerRect.right - popoverWidth
		} else {
			left = triggerRect.left
		}
		left = Math.max(VIEWPORT_PADDING, Math.min(left, viewportWidth - popoverWidth - VIEWPORT_PADDING))

		popoverStyle = `position:fixed;top:${top}px;left:${left}px;`
	}

	function onScroll() {
		computePosition()
		if (openCategoryId) computeFlyoutPosition()
	}

	$effect(() => {
		if (open && triggerEl && popoverEl) {
			if (rafId) cancelAnimationFrame(rafId)
			computePosition()
			rafId = requestAnimationFrame(() => {
				computePosition()
			})
			window.addEventListener('scroll', onScroll, true)
		} else {
			window.removeEventListener('scroll', onScroll, true)
		}

		return () => {
			if (rafId) cancelAnimationFrame(rafId)
			window.removeEventListener('scroll', onScroll, true)
		}
	})

	// =========================================================================
	// Flyout sub-menu state & positioning
	// =========================================================================

	let openCategoryId: string | null = $state(null)
	let rowEls: Record<string, HTMLElement> = {}
	let openTimer: ReturnType<typeof setTimeout> | undefined
	let closeTimer: ReturnType<typeof setTimeout> | undefined

	function computeFlyoutPosition() {
		if (!openCategoryId || !popoverEl || !flyoutEl) return

		const rowEl = rowEls[openCategoryId]
		if (!rowEl) return

		const popoverRect = popoverEl.getBoundingClientRect()
		const rowRect = rowEl.getBoundingClientRect()
		const flyoutWidth = flyoutEl.offsetWidth
		const flyoutHeight = flyoutEl.offsetHeight
		const viewportWidth = window.innerWidth
		const viewportHeight = window.innerHeight

		// Position to the right of the main popover
		let left = popoverRect.right + FLYOUT_GAP
		// If not enough space on right, flip to the left
		if (left + flyoutWidth + VIEWPORT_PADDING > viewportWidth) {
			left = popoverRect.left - flyoutWidth - FLYOUT_GAP
		}
		left = Math.max(VIEWPORT_PADDING, Math.min(left, viewportWidth - flyoutWidth - VIEWPORT_PADDING))

		// Align top with the hovered row
		let top = rowRect.top
		top = Math.max(VIEWPORT_PADDING, Math.min(top, viewportHeight - flyoutHeight - VIEWPORT_PADDING))

		flyoutStyle = `position:fixed;top:${top}px;left:${left}px;`
	}

	// Recompute flyout position when category changes
	$effect(() => {
		if (openCategoryId && flyoutEl && popoverEl) {
			requestAnimationFrame(() => {
				computeFlyoutPosition()
			})
		}
	})

	function handleCategoryEnter(categoryId: string) {
		clearTimeout(closeTimer)
		if (openCategoryId && openCategoryId !== categoryId) {
			// Instant switch when another flyout is already open
			openCategoryId = categoryId
		} else if (!openCategoryId) {
			openTimer = setTimeout(() => {
				openCategoryId = categoryId
			}, 100)
		}
	}

	function handleCategoryLeave() {
		clearTimeout(openTimer)
		closeTimer = setTimeout(() => {
			openCategoryId = null
		}, 150)
	}

	function handleFlyoutEnter() {
		clearTimeout(closeTimer)
	}

	function handleFlyoutLeave() {
		closeTimer = setTimeout(() => {
			openCategoryId = null
		}, 150)
	}

	// =========================================================================
	// Click outside & keyboard
	// =========================================================================

	function handleClickOutside(e: MouseEvent) {
		const target = e.target as Node
		if (triggerEl?.contains(target) || popoverEl?.contains(target) || flyoutEl?.contains(target)) return
		open = false
		openCategoryId = null
	}

	$effect(() => {
		if (open) {
			document.addEventListener('click', handleClickOutside)
		} else {
			document.removeEventListener('click', handleClickOutside)
		}

		return () => {
			document.removeEventListener('click', handleClickOutside)
		}
	})

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape' && open) {
			e.preventDefault()
			open = false
			openCategoryId = null
			triggerEl?.focus()
		}
	}

	// Clean up timers when dropdown closes
	$effect(() => {
		if (!open) {
			clearTimeout(openTimer)
			clearTimeout(closeTimer)
			openCategoryId = null
		}
	})

	// =========================================================================
	// Flyout tag chip hover
	// =========================================================================

	let hoveredTagId: string | null = $state(null)

	function chipStyle(color: string, isActive: boolean, isHovered: boolean): string {
		if (isActive && isHovered) {
			return `background-color: ${color}30; color: ${color}; border: 1px solid ${color}50;`
		}
		if (isActive) {
			return `background-color: ${color}20; color: ${color}; border: 1px solid ${color}40;`
		}
		if (isHovered) {
			return `background-color: ${color}15; color: ${color}; border: 1px solid ${color}30;`
		}
		return `background-color: transparent; color: var(--text-tertiary); border: 1px solid var(--stroke);`
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="relative">
	<Tooltip text={$translate('filters.title')} position="bottom" delay={250}>
		<button
			bind:this={triggerEl}
			type="button"
			class="relative inline-flex h-6 w-6 items-center justify-center rounded-md transition-colors focus:ring-2 focus:ring-brand-primary focus:outline-none {hasActiveFilters
				? 'bg-brand-muted text-brand-primary hover:cursor-pointer'
				: 'text-text-secondary hover:cursor-pointer hover:bg-surface-2 hover:text-text-primary'}"
			onclick={() => (open = !open)}
			aria-haspopup="true"
			aria-expanded={open}
		>
			<Icon name="filter" />
			{#if hasActiveFilters}
				<span
					class="absolute -top-1.5 -right-1.5 flex h-3.5 min-w-3.5 items-center justify-center rounded-full bg-brand-primary px-0.5 text-[9px] leading-none font-bold text-white"
				>
					{badgeCount}
				</span>
			{/if}
		</button>
	</Tooltip>

	{#if open}
		<div
			bind:this={popoverEl}
			use:portal
			class="z-50 min-w-[200px] rounded-md border border-stroke bg-surface-1 shadow-lg"
			style={popoverStyle}
			transition:scale={{ start: 0.95, duration: 200 }}
		>
			<div class="p-1.5">
				<!-- Facet rows: clicking anywhere on the row cycles off → include → exclude; the segmented control
				     inside sets a state directly (and stops the click so it doesn't also cycle). The row is a
				     role="button" div rather than a <button> because it contains the control's own buttons. -->
				{#each facetRows as row (row.key)}
					{@const state = row.facet.value}
					{@const cycle = () => row.facet.onChange(cycleTriState(state))}
					<div
						role="button"
						tabindex="0"
						class="flex w-full items-center justify-between gap-3 rounded px-2 py-1.5 text-sm transition-colors hover:cursor-pointer hover:bg-surface-2 focus-visible:ring-2 focus-visible:ring-brand-primary focus-visible:outline-none"
						onclick={cycle}
						onkeydown={(e) => {
							if (e.key === 'Enter' || e.key === ' ') {
								e.preventDefault()
								cycle()
							}
						}}
					>
						<div class="flex min-w-0 items-center gap-2">
							<Icon
								name={row.icon}
								class="h-3.5 w-3.5 {state === 'include'
									? 'text-brand-primary'
									: state === 'exclude'
										? 'text-text-secondary'
										: ''}"
								fill={row.key === 'liked' && state === 'include'}
							/>
							<span
								class="text-xs {state === 'include'
									? 'text-brand-primary'
									: state === 'exclude'
										? 'text-text-secondary line-through'
										: 'text-text-tertiary'}"
							>
								{$translate(row.labelKey)}
							</span>
						</div>
						<TriStateControl value={state} onChange={row.facet.onChange} label={$translate(row.labelKey)} size="sm" />
					</div>
				{/each}

				<!-- Purchased, unlinked: the row doubles as the feature's front door (opens Settings → Discovery). -->
				{#if onSetupPurchased}
					<button
						type="button"
						class="flex w-full items-center justify-between rounded px-2 py-1.5 text-sm transition-colors hover:cursor-pointer hover:bg-surface-2"
						onclick={() => {
							open = false
							onSetupPurchased()
						}}
					>
						<div class="flex items-center gap-2">
							<Icon name="shopping-bag" class="h-3.5 w-3.5" />
							<span class="text-xs text-text-tertiary">{$translate('settings.collection.linkAccount')}</span>
						</div>
						<svg
							class="h-3 w-3 text-text-tertiary"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="2.5"
						>
							<path d="M9 18l6-6-6-6" stroke-linecap="round" stroke-linejoin="round" />
						</svg>
					</button>
				{/if}

				{#if allTags.length > 0}
					{#if hasFacetRows}
						<div class="my-1 border-t border-stroke"></div>
					{/if}

					<!-- AND/OR toggle — always visible, disabled when < 2 tags active -->
					<div class="mb-1 flex items-center justify-between px-2 py-1">
						<div class="flex items-center gap-2">
							<Icon name="tag" class="h-3.5 w-3.5" />
							<span class="text-xs text-text-tertiary">{$translate('library.matching')}</span>
						</div>
						<button
							type="button"
							class="relative grid grid-cols-2 rounded-full border border-stroke bg-surface-2 p-0.5 text-xs font-medium transition-opacity {activeFilterTags.length <
							2
								? 'pointer-events-none opacity-40'
								: 'hover:cursor-pointer'}"
							onclick={() => onToggleTagFilterMode()}
							disabled={activeFilterTags.length < 2}
						>
							<div
								class="absolute inset-y-0.5 rounded-full bg-brand-primary transition-all duration-200 ease-out motion-reduce:transition-none"
								style="left: {tagFilterMode === 'or' ? '2px' : '50%'}; right: {tagFilterMode === 'or' ? '50%' : '2px'}"
							></div>
							<span
								class="relative z-10 rounded-full px-2 py-0.5 text-center transition-colors {tagFilterMode === 'or'
									? 'text-white'
									: 'text-text-tertiary hover:text-text-secondary'}"
							>
								{$translate('library.matchOr')}
							</span>
							<span
								class="relative z-10 rounded-full px-2 py-0.5 text-center transition-colors {tagFilterMode === 'and'
									? 'text-white'
									: 'text-text-tertiary hover:text-text-secondary'}"
							>
								{$translate('library.matchAnd')}
							</span>
						</button>
					</div>

					<!-- Category rows -->
					{#each tagCategories as category (category.id)}
						{@const color = category.color || '#6366f1'}
						{@const count = activeCounts.get(category.id) ?? 0}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div
							bind:this={rowEls[category.id]}
							class="flex items-center gap-2 rounded px-2 py-1.5 transition-colors hover:cursor-pointer hover:bg-surface-2 {openCategoryId ===
							category.id
								? 'bg-surface-2'
								: ''}"
							onmouseenter={() => handleCategoryEnter(category.id)}
							onmouseleave={() => handleCategoryLeave()}
						>
							<span class="h-2 w-2 shrink-0 rounded-full" style="background-color: {color};"></span>
							<span class="max-w-[120px] truncate text-xs text-text-primary">{category.name}</span>
							<div class="flex-1"></div>
							{#if count > 0}
								<span
									class="flex h-4 min-w-4 items-center justify-center rounded-full px-1 text-[10px] leading-none font-semibold text-white"
									style="background-color: {color};"
								>
									{count}
								</span>
							{/if}
							<Icon name="chevron-right" class="h-3 w-3 shrink-0 text-text-tertiary" />
						</div>
					{/each}
				{/if}

				<!-- Clear all button — outside the tag block so the toggle-only facets can still be
				     cleared in a workspace with no tag categories -->
				{#if hasActiveFilters}
					<div class="mt-1 border-t border-stroke pt-1" transition:slide={{ duration: 150 }}>
						<div in:fade={{ duration: 100, delay: 50 }} out:fade={{ duration: 75 }}>
							<Button
								variant="ghost-danger"
								size="sm"
								class="w-full justify-start"
								onclick={() => {
									onClearAll()
									open = false
								}}
							>
								{$translate('library.clearAll')}
							</Button>
						</div>
					</div>
				{/if}
			</div>
		</div>

		<!-- Flyout sub-menu for the hovered category -->
		{#if openCategoryId}
			{@const flyoutCategory = tagCategories.find((c) => c.id === openCategoryId)}
			{#if flyoutCategory && flyoutCategory.tags.length > 0}
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<div
					bind:this={flyoutEl}
					use:portal
					class="z-50 max-h-[280px] max-w-[240px] min-w-[140px] overflow-y-auto rounded-md border border-stroke bg-surface-1 shadow-lg"
					style={flyoutStyle}
					onmouseenter={handleFlyoutEnter}
					onmouseleave={handleFlyoutLeave}
					transition:scale={{ start: 0.97, duration: 120 }}
				>
					<div class="flex flex-wrap gap-1 p-2">
						{#each flyoutCategory.tags as tag (tag.id)}
							{@const color = tagColors.get(tag.category_id) || tag.color || '#6366f1'}
							{@const isActive = activeTagIds.has(tag.id)}
							<button
								type="button"
								class="rounded px-1.5 py-0.5 text-[11px] leading-snug font-medium transition-colors hover:cursor-pointer"
								style={chipStyle(color, isActive, hoveredTagId === tag.id)}
								onmouseenter={() => (hoveredTagId = tag.id)}
								onmouseleave={() => (hoveredTagId = null)}
								onclick={() => onToggleTagFilter(tag.id)}
							>
								{tag.name}
							</button>
						{/each}
					</div>
				</div>
			{/if}
		{/if}
	{/if}
</div>
