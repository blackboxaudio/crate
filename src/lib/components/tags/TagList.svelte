<script lang="ts">
	import type { TagCategory } from '$lib/types'
	import TagChip from './TagChip.svelte'

	type Props = {
		categories: TagCategory[]
		selectedTagId?: string | null
		onTagClick?: (tagId: string) => void
	}

	let { categories, selectedTagId = null, onTagClick }: Props = $props()
</script>

<div class="space-y-4">
	{#each categories as category (category.id)}
		<div>
			<h3 class="mb-2 text-xs font-semibold tracking-wider text-zinc-500 uppercase">
				{category.name}
			</h3>
			<div class="flex flex-wrap gap-1.5">
				{#each category.tags as tag (tag.id)}
					<TagChip {tag} onclick={() => onTagClick?.(tag.id)} />
				{/each}
				{#if category.tags.length === 0}
					<span class="text-xs text-zinc-600 italic">No tags</span>
				{/if}
			</div>
		</div>
	{/each}

	{#if categories.length === 0}
		<p class="py-4 text-center text-sm text-zinc-500">No tag categories yet</p>
	{/if}
</div>
