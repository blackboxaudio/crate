<script lang="ts">
  import type { Tag } from '$lib/types';

  type Props = {
    tag: Tag;
    size?: 'sm' | 'md';
    removable?: boolean;
    onclick?: () => void;
    onremove?: () => void;
  }

  let {
    tag,
    size = 'md',
    removable = false,
    onclick,
    onremove
  }: Props = $props();

  const sizeStyles = {
    sm: 'px-1.5 py-0.5 text-xs',
    md: 'px-2 py-1 text-xs'
  };

  // Default color if none specified
  let bgColor = $derived(tag.color || '#6366f1');
</script>

<span
  role={onclick ? 'button' : 'presentation'}
  tabindex="-1"
  class="inline-flex items-center gap-1 rounded font-medium {sizeStyles[size]} {onclick ? 'cursor-pointer hover:opacity-80' : ''}"
  style="background-color: {bgColor}20; color: {bgColor}; border: 1px solid {bgColor}40;"
  onclick={onclick}
  onkeydown={(e) => e.key === 'Enter' && onclick?.()}
>
  {tag.name}
  {#if removable && onremove}
    <button
      type="button"
      aria-label="Remove tag"
      class="ml-0.5 hover:opacity-70"
      onclick={(e) => { e.stopPropagation(); onremove(); }}
    >
      <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
      </svg>
    </button>
  {/if}
</span>
