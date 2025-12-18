<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = {
    title?: string;
    disabled?: boolean;
    active?: boolean;
    size?: 'sm' | 'md' | 'lg';
    class?: string;
    onclick?: (e: MouseEvent) => void;
    children: Snippet;
  }

  let {
    title = '',
    disabled = false,
    active = false,
    size = 'md',
    class: className = '',
    onclick,
    children
  }: Props = $props();

  const sizeStyles = {
    sm: 'w-6 h-6 text-sm',
    md: 'w-8 h-8 text-base',
    lg: 'w-10 h-10 text-lg'
  };
</script>

<button
  type="button"
  {title}
  {disabled}
  class="inline-flex items-center justify-center rounded-md transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed {active ? 'bg-zinc-700 text-zinc-100' : 'text-zinc-400 hover:text-zinc-100 hover:bg-zinc-800'} {sizeStyles[size]} {className}"
  onclick={onclick}
>
  {@render children()}
</button>
