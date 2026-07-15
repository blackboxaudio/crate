<script lang="ts">
	import { fade } from 'svelte/transition'
	import { translate } from '$shared/i18n'
	import { tagsStore } from '$shared/stores/tags'
	import type { TagFilterMode } from '$shared/types'
	import { lightTap } from '$lib/utils/haptics'
	import MobileModal from '$lib/components/common/MobileModal.svelte'

	// Generalized filter sheet (liked / downloaded toggles + tag chips with AND/OR matching).
	// Fully controlled: each facet renders only when its prop is provided, so the feed and each
	// detail view opt into exactly the filters that make sense there (the tag detail omits tags,
	// etc.). Mirrors the desktop FilterDropdown's segmented match toggle and liked switch.
	type Props = {
		open: boolean
		onClose: () => void
		liked?: { value: boolean; onToggle: () => void }
		downloaded?: { value: boolean; onToggle: () => void }
		tags?: {
			activeIds: string[]
			mode: TagFilterMode
			onToggleTag: (id: string) => void
			onToggleMode: () => void
		}
		onClearAll: () => void
	}
	let { open, onClose, liked, downloaded, tags, onClearAll }: Props = $props()

	// Lazy-load categories the first time the sheet opens (only when the tags facet is shown).
	let loadedOnce = $state(false)
	$effect(() => {
		if (open && tags && !loadedOnce) {
			loadedOnce = true
			void tagsStore.load()
		}
	})

	const active = $derived(new Set(tags?.activeIds ?? []))
	const hasActiveFilters = $derived(active.size > 0 || !!liked?.value || !!downloaded?.value)

	// Selection tick: filters commit instantly (no confirm step), so acknowledge each toggle the way
	// the app's other sheets do (playlist picker, queue actions, sort options).
	function tick(fn: () => void) {
		void lightTap()
		fn()
	}
</script>

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
		{#if liked}
			<button
				type="button"
				class="flex w-full items-center justify-between rounded-md py-1 active:bg-surface-2"
				aria-pressed={liked.value}
				onclick={() => tick(liked.onToggle)}
			>
				<span class="flex items-center gap-2 text-sm font-medium text-text-primary">
					<svg
						class="h-4 w-4 {liked.value ? 'text-brand-primary' : 'text-text-tertiary'}"
						viewBox="0 0 24 24"
						fill={liked.value ? 'currentColor' : 'none'}
						stroke="currentColor"
						stroke-width="2"
					>
						<path
							d="M20.84 4.61a5.5 5.5 0 00-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 00-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 000-7.78z"
						/>
					</svg>
					{$translate('filters.liked')}
				</span>
				<span
					class="flex h-5 w-9 items-center rounded-full p-0.5 transition-colors {liked.value
						? 'bg-brand-primary'
						: 'bg-stroke'}"
				>
					<span class="h-4 w-4 rounded-full bg-white transition-transform {liked.value ? 'translate-x-4' : ''}"></span>
				</span>
			</button>
		{/if}

		{#if downloaded}
			<!-- Downloaded-only: releases whose audio is fully cached, i.e. playable in airplane mode. -->
			<button
				type="button"
				class="flex w-full items-center justify-between rounded-md py-1 active:bg-surface-2"
				aria-pressed={downloaded.value}
				onclick={() => tick(downloaded.onToggle)}
			>
				<span class="flex items-center gap-2 text-sm font-medium text-text-primary">
					<svg
						class="h-4 w-4 {downloaded.value ? 'text-brand-primary' : 'text-text-tertiary'}"
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
					{$translate('filters.downloaded')}
				</span>
				<span
					class="flex h-5 w-9 items-center rounded-full p-0.5 transition-colors {downloaded.value
						? 'bg-brand-primary'
						: 'bg-stroke'}"
				>
					<span class="h-4 w-4 rounded-full bg-white transition-transform {downloaded.value ? 'translate-x-4' : ''}"
					></span>
				</span>
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
