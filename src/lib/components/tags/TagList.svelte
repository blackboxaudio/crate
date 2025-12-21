<script lang="ts">
	import type { Tag, TagCategory, TagSelectionState } from '$lib/types'
	import TagChip from './TagChip.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import { Text } from '$lib/components/common'
	import { translate } from '$lib/i18n'

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
		onWhitespaceContextMenu?: (e: MouseEvent) => void
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
		onWhitespaceContextMenu,
	}: Props = $props()

	function handleContainerContextMenu(e: MouseEvent) {
		const target = e.target as HTMLElement
		// Don't trigger if clicking on a tag, category, or button
		if (target.closest('[data-tag]') || target.closest('[role="group"]') || target.closest('button')) return
		if (onWhitespaceContextMenu) {
			e.preventDefault()
			onWhitespaceContextMenu(e)
		}
	}

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

<div class="h-full space-y-4" oncontextmenu={handleContainerContextMenu} role="group">
	{#each categories as category (category.id)}
		<div>
			<div
				class="group mb-2 flex items-center justify-between py-1.5 pr-1.5 pl-3"
				role="group"
				aria-label="{category.name} category"
				oncontextmenu={(e) => handleCategoryContextMenu(e, category)}
			>
				<Text variant="header-table" as="h3" weight="semibold" class="cursor-default">
					{category.name}
				</Text>
				<button
					type="button"
					class="rounded p-0.5 text-text-tertiary transition-colors hover:cursor-pointer hover:bg-surface-2 hover:text-text-secondary"
					onclick={() => onCreateTag?.(category.id)}
					title={$translate('tags.addTag')}
				>
					<Icon name="plus" class="h-3.5 w-3.5" />
				</button>
			</div>
			<div class="flex flex-wrap gap-1.5 px-3">
				{#each category.tags.toSorted((a, b) => a.name.localeCompare(b.name)) as tag (tag.id)}
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
					<Text variant="caption" italic>No tags</Text>
				{/if}
			</div>
		</div>
	{/each}

	{#if categories.length === 0}
		<Text variant="caption" as="p" class="py-4 text-center">No tag categories yet</Text>
	{/if}
</div>
