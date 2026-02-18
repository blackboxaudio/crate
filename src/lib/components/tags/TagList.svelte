<script lang="ts">
	import type { Tag, TagCategory, TagSelectionState } from '$lib/types'
	import TagChip from './TagChip.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import { Text, Tooltip } from '$lib/components/common'
	import { translate } from '$lib/i18n'
	import { dragStore, isDraggingTag, hoveredDropTarget } from '$lib/stores'
	import { DRAG_THRESHOLD, getDistance } from '$lib/utils/drag'

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

	// Drag state tracking
	let pointerStartPos: { x: number; y: number } | null = $state(null)
	let dragTagId: string | null = $state(null)
	let dragCategoryId: string | null = $state(null)
	let isDragActive = $state(false)
	let didDrag = false

	function handleTagPointerDown(e: PointerEvent, tag: Tag, categoryId: string) {
		if (isToggleMode || e.button !== 0) return
		pointerStartPos = { x: e.clientX, y: e.clientY }
		dragTagId = tag.id
		dragCategoryId = categoryId
		isDragActive = false
	}

	function handleTagPointerMove(e: PointerEvent) {
		if (!pointerStartPos || !dragTagId || !dragCategoryId) return
		if (isDragActive) return

		const distance = getDistance(pointerStartPos.x, pointerStartPos.y, e.clientX, e.clientY)
		if (distance >= DRAG_THRESHOLD) {
			isDragActive = true
			didDrag = true
			dragStore.startTagDrag(dragTagId, dragCategoryId, e.clientX, e.clientY)
		}
	}

	function handleTagPointerUp() {
		pointerStartPos = null
		dragTagId = null
		dragCategoryId = null
		isDragActive = false
		// Reset didDrag after a microtask — if a click fires synchronously on the
		// same element, it will still see didDrag=true and be suppressed. But if
		// the pointerup was on a different element (e.g. a category header), no
		// click fires and the flag is cleaned up to avoid eating the next click.
		setTimeout(() => {
			didDrag = false
		}, 0)
	}

	function handleContainerContextMenu(e: MouseEvent) {
		const target = e.target as HTMLElement
		// Don't trigger if clicking on a tag, category, or button
		if (target.closest('[data-tag]') || target.closest('[data-category]') || target.closest('button')) return
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
		if (didDrag) {
			didDrag = false
			return
		}
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
		{@const categoryColor = category.color || '#6366f1'}
		<div>
			<div
				class="group mb-2 flex items-center justify-between rounded-sm py-1.5 pr-1.5 pl-3 transition-[color,background-color,box-shadow]"
				style={$isDraggingTag && $hoveredDropTarget === `category-${category.id}`
					? `background-color: ${categoryColor}20; box-shadow: inset 0 0 0 1px ${categoryColor}40;`
					: ''}
				data-category
				data-drop-target="category-{category.id}"
				role="group"
				aria-label="{category.name} category"
				oncontextmenu={(e) => handleCategoryContextMenu(e, category)}
			>
				<Text variant="header-table" as="h3" weight="semibold" class="cursor-default">
					{category.name}
				</Text>
				<Tooltip text={$translate('tags.addTag')} position="left" delay={250}>
					<button
						type="button"
						class="rounded p-0.5 text-text-tertiary transition-colors hover:cursor-pointer hover:bg-surface-2 hover:text-text-secondary"
						onclick={() => onCreateTag?.(category.id)}
					>
						<Icon name="plus" class="h-3.5 w-3.5" />
					</button>
				</Tooltip>
			</div>
			<div
				class="flex flex-wrap gap-1.5 px-3"
				oncontextmenu={(e) => {
					if ((e.target as HTMLElement).closest('[data-tag]')) return
					e.stopPropagation()
					handleCategoryContextMenu(e, category)
				}}
			>
				{#each category.tags.toSorted((a, b) => a.name.localeCompare(b.name)) as tag (tag.id)}
					<span
						onpointerdown={(e) => handleTagPointerDown(e, tag, category.id)}
						onpointermove={handleTagPointerMove}
						onpointerup={handleTagPointerUp}
						onpointercancel={handleTagPointerUp}
					>
						<TagChip
							{tag}
							color={category.color}
							state={isToggleMode ? tagStates?.get(tag.id) : undefined}
							selectionCount={isToggleMode ? tagCounts?.get(tag.id) : undefined}
							selectionTotal={isToggleMode ? selectedTrackCount : undefined}
							onclick={() => handleTagClick(tag)}
							oncontextmenu={(e) => handleTagContextMenu(e, tag, category)}
						/>
					</span>
				{/each}
				{#if category.tags.length === 0}
					<Text variant="caption" italic>{$translate('tags.noTags')}</Text>
				{/if}
			</div>
		</div>
	{/each}

	{#if categories.length === 0}
		<Text variant="caption" as="p" class="py-4 text-center" italic>{$translate('tags.noTagCategoriesYet')}</Text>
	{/if}
</div>
