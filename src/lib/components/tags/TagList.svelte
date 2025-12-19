<script lang="ts">
	import type { Tag, TagCategory, TagSelectionState } from '$lib/types'
	import TagChip from './TagChip.svelte'
	import Icon from '$lib/components/common/Icon.svelte'

	type Props = {
		categories: TagCategory[]
		selectedTagId?: string | null
		isToggleMode?: boolean
		tagStates?: Map<string, TagSelectionState>
		tagCounts?: Map<string, number>
		selectedTrackCount?: number
		onTagClick?: (tagId: string) => void
		onTagToggle?: (tagId: string, state: TagSelectionState) => void
		onCreateTag?: (categoryId: string) => void
		onTagContextMenu?: (e: MouseEvent, tag: Tag, category: TagCategory) => void
		onCategoryContextMenu?: (e: MouseEvent, category: TagCategory) => void
	}

	let {
		categories,
		selectedTagId = null,
		isToggleMode = false,
		tagStates,
		tagCounts,
		selectedTrackCount = 0,
		onTagClick,
		onTagToggle,
		onCreateTag,
		onTagContextMenu,
		onCategoryContextMenu,
	}: Props = $props()

	function handleCategoryContextMenu(e: MouseEvent, category: TagCategory) {
		e.preventDefault()
		onCategoryContextMenu?.(e, category)
	}

	function handleTagContextMenu(e: MouseEvent, tag: Tag, category: TagCategory) {
		e.preventDefault()
		onTagContextMenu?.(e, tag, category)
	}

	function handleTagClick(tag: Tag) {
		if (isToggleMode && onTagToggle && tagStates) {
			const state = tagStates.get(tag.id) || 'inactive'
			onTagToggle(tag.id, state)
		} else {
			onTagClick?.(tag.id)
		}
	}
</script>

<div class="space-y-4">
	{#each categories as category (category.id)}
		<div>
			<div
				class="group mb-2 flex items-center justify-between"
				role="group"
				aria-label="{category.name} category"
				oncontextmenu={(e) => handleCategoryContextMenu(e, category)}
			>
				<h3 class="cursor-default text-xs font-semibold tracking-wider text-text-tertiary uppercase">
					{category.name}
				</h3>
				<button
					type="button"
					class="rounded p-0.5 text-text-tertiary transition-colors hover:bg-surface-2 hover:text-text-secondary"
					onclick={() => onCreateTag?.(category.id)}
					title="Add tag to {category.name}"
				>
					<Icon name="plus" class="h-3.5 w-3.5" />
				</button>
			</div>
			<div class="flex flex-wrap gap-1.5">
				{#each category.tags as tag (tag.id)}
					<TagChip
						{tag}
						color={category.color}
						state={isToggleMode ? tagStates?.get(tag.id) : undefined}
						selectionCount={isToggleMode ? tagCounts?.get(tag.id) : undefined}
						selectionTotal={isToggleMode ? selectedTrackCount : undefined}
						onclick={() => handleTagClick(tag)}
						oncontextmenu={(e) => handleTagContextMenu(e, tag, category)}
					/>
				{/each}
				{#if category.tags.length === 0}
					<span class="text-xs text-text-tertiary italic">No tags</span>
				{/if}
			</div>
		</div>
	{/each}

	{#if categories.length === 0}
		<p class="py-4 text-center text-sm text-text-tertiary">No tag categories yet</p>
	{/if}
</div>
