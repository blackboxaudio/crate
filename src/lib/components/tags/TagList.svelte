<script lang="ts">
  import type { TagCategory } from '$lib/types';
  import TagChip from './TagChip.svelte';

  type Props = {
    categories: TagCategory[];
    selectedTagId?: string | null;
    onTagClick?: (tagId: string) => void;
  }

  let {
    categories,
    selectedTagId = null,
    onTagClick
  }: Props = $props();
</script>

<div class="space-y-4">
  {#each categories as category}
    <div>
      <h3 class="text-xs font-semibold text-zinc-500 uppercase tracking-wider mb-2">
        {category.name}
      </h3>
      <div class="flex flex-wrap gap-1.5">
        {#each category.tags as tag}
          <TagChip
            {tag}
            onclick={() => onTagClick?.(tag.id)}
          />
        {/each}
        {#if category.tags.length === 0}
          <span class="text-xs text-zinc-600 italic">No tags</span>
        {/if}
      </div>
    </div>
  {/each}

  {#if categories.length === 0}
    <p class="text-sm text-zinc-500 text-center py-4">
      No tag categories yet
    </p>
  {/if}
</div>
