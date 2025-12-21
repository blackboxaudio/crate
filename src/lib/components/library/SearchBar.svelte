<script lang="ts">
	import type { Tag, TagFilterMode } from '$lib/types'
	import { libraryStore, uiStore } from '$lib/stores'
	import Button from '$lib/components/common/Button.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import TagChip from '$lib/components/tags/TagChip.svelte'
	import { translate } from '$lib/i18n'

	type Props = {
		activeFilterTags?: Tag[]
		tagColors?: Map<string, string | null>
		tagFilterMode?: TagFilterMode
		onRemoveTagFilter?: (tagId: string) => void
		onClearAllTagFilters?: () => void
		onToggleTagFilterMode?: () => void
	}

	let {
		activeFilterTags = [],
		tagColors,
		tagFilterMode = 'or',
		onRemoveTagFilter,
		onClearAllTagFilters,
		onToggleTagFilterMode,
	}: Props = $props()

	// Compute remaining tags count for "+N" badge
	const remainingTagsCount = $derived(activeFilterTags && activeFilterTags.length > 1 ? activeFilterTags.length - 1 : 0)

	// State for hover popup
	let showTagPopup = $state(false)

	let inputValue = $state('')

	// Debounced search
	let debounceTimer: ReturnType<typeof setTimeout>

	function handleInput(e: Event) {
		const target = e.target as HTMLInputElement
		inputValue = target.value

		clearTimeout(debounceTimer)
		debounceTimer = setTimeout(() => {
			uiStore.setSearchQuery(inputValue)
			libraryStore.setSearch(inputValue)
		}, 300)
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			inputValue = ''
			uiStore.clearSearch()
			libraryStore.setSearch('')
			;(e.target as HTMLInputElement).blur()
		}
	}

	function handleClear() {
		inputValue = ''
		uiStore.clearSearch()
		libraryStore.setSearch('')
	}
</script>

<div class="relative">
	<div class="pointer-events-none absolute inset-y-0 left-0 flex items-center pl-3">
		<Icon name="search" class="h-4 w-4 text-text-tertiary" />
	</div>

	<input
		type="search"
		placeholder={$translate('library.searchPlaceholder')}
		value={inputValue}
		class="w-full rounded-md border border-stroke bg-surface-2 py-1.5 pl-10 text-sm text-text-primary placeholder-text-tertiary focus:border-transparent focus:ring-2 focus:ring-brand-primary focus:outline-none {activeFilterTags &&
		activeFilterTags.length > 0
			? 'pr-[8.5rem]'
			: 'pr-8'}"
		oninput={handleInput}
		onkeydown={handleKeydown}
		onfocus={() => uiStore.setSearchFocused(true)}
		onblur={() => uiStore.setSearchFocused(false)}
	/>

	<!-- Tag filters container (right side, shifts left when clear button is visible) -->
	{#if activeFilterTags && activeFilterTags.length > 0}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="absolute inset-y-0 flex items-center gap-1 {inputValue ? 'right-8' : 'right-0 pr-3'}"
			onmouseleave={() => (showTagPopup = false)}
		>
			<TagChip
				tag={activeFilterTags[0]}
				color={tagColors?.get(activeFilterTags[0].category_id)}
				size="sm"
				removable
				onremove={() => onRemoveTagFilter?.(activeFilterTags[0].id)}
			/>
			{#if remainingTagsCount > 0}
				<!-- Hover container for +N badge and popup -->
				<div class="relative" onmouseenter={() => (showTagPopup = true)}>
					<span
						class="hover:bg-surface-3 flex h-5 min-w-5 cursor-pointer items-center justify-center rounded bg-surface-2 px-1 text-xs text-text-secondary"
					>
						+{remainingTagsCount}
					</span>

					<!-- Hover popup with padding-top to create hoverable bridge -->
					{#if showTagPopup}
						<div class="absolute top-full right-0 z-[200] pt-1">
							<div class="min-w-[180px] rounded-md border border-stroke bg-surface-1 p-2 shadow-lg">
								<!-- Filter mode toggle -->
								<div class="mb-2 flex items-center justify-between border-b border-stroke pb-2 pl-1">
									<span class="text-xs text-text-tertiary">{$translate('library.matching')}</span>
									<button
										type="button"
										class="flex items-center gap-0.5 rounded-full border border-stroke bg-surface-2 p-0.5 text-xs font-medium hover:cursor-pointer"
										onclick={() => onToggleTagFilterMode?.()}
									>
										<span
											class="rounded-full px-2 py-0.5 transition-colors {tagFilterMode === 'or'
												? 'bg-brand-primary text-white'
												: 'text-text-tertiary hover:text-text-secondary'}"
										>
											{$translate('library.matchOr')}
										</span>
										<span
											class="rounded-full px-2 py-0.5 transition-colors {tagFilterMode === 'and'
												? 'bg-brand-primary text-white'
												: 'text-text-tertiary hover:text-text-secondary'}"
										>
											{$translate('library.matchAnd')}
										</span>
									</button>
								</div>
								<div class="flex flex-col gap-1.5">
									{#each activeFilterTags as tag (tag.id)}
										<div
											class="flex w-full items-center justify-between gap-2 rounded px-1.5 py-0.5 text-xs font-medium"
											style="background-color: {tagColors?.get(tag.category_id) ||
												tag.color ||
												'#6366f1'}20; color: {tagColors?.get(tag.category_id) ||
												tag.color ||
												'#6366f1'}; border: 1px solid {tagColors?.get(tag.category_id) || tag.color || '#6366f1'}40;"
										>
											<span>{tag.name}</span>
											<button
												type="button"
												aria-label="Remove tag"
												class="hover:cursor-pointer hover:opacity-70"
												onclick={() => onRemoveTagFilter?.(tag.id)}
											>
												<Icon name="x" class="h-3 w-3" />
											</button>
										</div>
									{/each}
								</div>
								<div class="mt-2 border-t border-stroke pt-2">
									<Button
										variant="ghost-danger"
										size="sm"
										class="w-full justify-start"
										onclick={() => {
											onClearAllTagFilters?.()
											showTagPopup = false
										}}
									>
										{$translate('library.clearAll')}
									</Button>
								</div>
							</div>
						</div>
					{/if}
				</div>
			{/if}
		</div>
	{/if}

	<!-- Clear button -->
	{#if inputValue}
		<button
			type="button"
			aria-label="Clear search"
			class="absolute inset-y-0 right-0 flex items-center pr-3 text-text-tertiary hover:text-text-secondary"
			onclick={handleClear}
		>
			<Icon name="x" />
		</button>
	{/if}
</div>
