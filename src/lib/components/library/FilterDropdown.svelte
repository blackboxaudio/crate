<script lang="ts">
	import type { Tag, TagFilterMode } from '$lib/types'
	import Icon from '$lib/components/common/Icon.svelte'
	import Button from '$lib/components/common/Button.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import { translate } from '$lib/i18n'
	import { scale } from 'svelte/transition'

	type Props = {
		activeFilterTags: Tag[]
		tagColors: Map<string, string | null>
		tagFilterMode: TagFilterMode
		onRemoveTagFilter: (tagId: string) => void
		onClearAll: () => void
		onToggleTagFilterMode: () => void
		showLikedFilter?: boolean
		likedOnly?: boolean
		onToggleLikedFilter?: () => void
	}

	let {
		activeFilterTags,
		tagColors,
		tagFilterMode,
		onRemoveTagFilter,
		onClearAll,
		onToggleTagFilterMode,
		showLikedFilter = false,
		likedOnly = false,
		onToggleLikedFilter,
	}: Props = $props()

	let open = $state(false)
	let triggerEl: HTMLButtonElement | undefined = $state()
	let popoverEl: HTMLDivElement | undefined = $state()

	const badgeCount = $derived(activeFilterTags.length + (showLikedFilter && likedOnly ? 1 : 0))
	const hasActiveFilters = $derived(badgeCount > 0)

	// Position the popover using fixed positioning, portaled to escape overflow
	const GAP = 4
	const VIEWPORT_PADDING = 8
	let popoverStyle = $state('')
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

		// Align right edge of popover with right edge of trigger, clamped to viewport
		let left: number
		if (popoverWidth > 0) {
			left = triggerRect.right - popoverWidth
		} else {
			// Fallback before popover is measured: align left edge with trigger left
			left = triggerRect.left
		}
		left = Math.max(VIEWPORT_PADDING, Math.min(left, viewportWidth - popoverWidth - VIEWPORT_PADDING))

		popoverStyle = `position:fixed;top:${top}px;left:${left}px;`
	}

	function onScroll() {
		computePosition()
	}

	$effect(() => {
		if (open && triggerEl && popoverEl) {
			if (rafId) cancelAnimationFrame(rafId)
			// Compute immediately for initial placement, then again after layout settles
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

	function handleClickOutside(e: MouseEvent) {
		const target = e.target as Node
		if (triggerEl?.contains(target) || popoverEl?.contains(target)) return
		open = false
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
			triggerEl?.focus()
		}
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
			class="z-50 min-w-[220px] rounded-md border border-stroke bg-surface-1 shadow-lg"
			style={popoverStyle}
			transition:scale={{ start: 0.95, duration: 200 }}
		>
			<div class="p-2">
				<!-- Liked filter -->
				{#if showLikedFilter}
					<button
						type="button"
						class="flex w-full items-center justify-between rounded px-2 py-1.5 text-sm transition-colors hover:cursor-pointer hover:bg-surface-2"
						onclick={() => onToggleLikedFilter?.()}
					>
						<div class="flex items-center gap-2">
							<Icon name="heart" class="h-3.5 w-3.5" fill={likedOnly} />
							<span class="text-xs text-text-tertiary">{$translate('filters.liked')}</span>
						</div>
						<div
							class="flex h-4 w-7 items-center rounded-full p-0.5 transition-colors {likedOnly
								? 'bg-brand-primary'
								: 'bg-stroke'}"
						>
							<div class="h-3 w-3 rounded-full bg-white transition-transform {likedOnly ? 'translate-x-3' : ''}"></div>
						</div>
					</button>
					{#if activeFilterTags.length > 0}
						<div class="my-1.5 border-t border-stroke"></div>
					{/if}
				{/if}

				<!-- AND/OR toggle (only when tags are active) -->
				{#if activeFilterTags.length > 0}
					<div class="mb-1.5 flex items-center justify-between px-2 py-1">
						<span class="text-xs text-text-tertiary">{$translate('library.matching')}</span>
						<button
							type="button"
							class="relative grid grid-cols-2 rounded-full border border-stroke bg-surface-2 p-0.5 text-xs font-medium hover:cursor-pointer"
							onclick={() => onToggleTagFilterMode()}
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

					<!-- Active tag list -->
					<div class="flex flex-col gap-1">
						{#each activeFilterTags as tag (tag.id)}
							<div
								class="flex w-full items-center justify-between gap-2 rounded px-2 py-1 text-xs font-medium"
								style="background-color: {tagColors.get(tag.category_id) ||
									tag.color ||
									'#6366f1'}20; color: {tagColors.get(tag.category_id) ||
									tag.color ||
									'#6366f1'}; border: 1px solid {tagColors.get(tag.category_id) || tag.color || '#6366f1'}40;"
							>
								<span>{tag.name}</span>
								<button
									type="button"
									aria-label="Remove tag"
									class="hover:cursor-pointer hover:opacity-70"
									onclick={() => onRemoveTagFilter(tag.id)}
								>
									<Icon name="x" class="h-3 w-3" />
								</button>
							</div>
						{/each}
					</div>

					<!-- Clear all button -->
					<div class="mt-1.5 border-t border-stroke pt-1.5">
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
				{/if}

				<!-- Empty state when no filters and no liked option -->
				{#if !showLikedFilter && activeFilterTags.length === 0}
					<div class="px-2 py-3 text-center text-xs text-text-tertiary">
						{$translate('filters.title')}
					</div>
				{/if}
			</div>
		</div>
	{/if}
</div>
