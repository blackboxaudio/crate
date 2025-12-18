<script lang="ts">
  import { Button } from '$lib/components/common';
  import { SearchBar } from '$lib/components/library';

  type Props = {
    onImport?: () => void;
  }

  let { onImport }: Props = $props();
</script>

<div class="flex items-center gap-4 px-4 py-2 bg-zinc-900 border-b border-zinc-800">
  <!-- Logo/Title -->
  <div class="flex items-center gap-2">
    <svg class="w-6 h-6 text-blue-500" fill="currentColor" viewBox="0 0 24 24">
      <path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z" />
    </svg>
    <span class="text-lg font-bold text-zinc-100">Crate</span>
  </div>

  <!-- Search -->
  <div class="flex-1 max-w-md">
    <SearchBar />
  </div>

  <!-- Spacer -->
  <div class="flex-1"></div>

  <!-- Actions -->
  <div class="flex items-center gap-2">
    <Button variant="primary" size="sm" onclick={onImport}>
      <svg class="w-4 h-4 mr-1.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
      </svg>
      Import
    </Button>
  </div>
</div>
