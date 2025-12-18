<script lang="ts">
  import '../style.css';
  import type { Snippet } from 'svelte';

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();
</script>

<div class="h-screen w-screen overflow-hidden bg-zinc-950 text-zinc-100 flex flex-col">
  {@render children()}
</div>
