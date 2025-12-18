<script lang="ts">
  import type { SortConfig, TrackSortField } from '$lib/types';
  import { getNextSortConfig } from '$lib/utils/sorting';

  type Props = {
    sortConfig: SortConfig;
    onSort?: (config: SortConfig) => void;
  };

  let { sortConfig, onSort }: Props = $props();

  function handleSort(field: TrackSortField) {
    const newConfig = getNextSortConfig(sortConfig, field);
    onSort?.(newConfig);
  }

  function getSortIndicator(field: TrackSortField): string {
    if (sortConfig.field !== field) return '';
    return sortConfig.direction === 'asc' ? ' ↑' : ' ↓';
  }

  type Column = {
    field: TrackSortField | null;
    label: string;
    align: 'left' | 'center' | 'right';
  };

  const columns: Column[] = [
    { field: 'title', label: 'Title', align: 'left' },
    { field: 'artist', label: 'Artist', align: 'left' },
    { field: 'bpm', label: 'BPM', align: 'right' },
    { field: 'key', label: 'Key', align: 'center' },
    { field: 'duration_ms', label: 'Time', align: 'right' },
    { field: null, label: 'Tags', align: 'left' }
  ];
</script>

<div class="grid grid-cols-[1fr_1fr_80px_60px_80px_1fr] gap-2 px-3 py-2 text-xs font-medium text-zinc-500 uppercase tracking-wider border-b border-zinc-700 bg-zinc-900/50 sticky top-0 z-10">
  {#each columns as column}
    {#if column.field}
      <button
        type="button"
        class="text-{column.align} hover:text-zinc-300 transition-colors"
        onclick={() => column.field && handleSort(column.field)}
      >
        {column.label}{getSortIndicator(column.field)}
      </button>
    {:else}
      <div class="text-{column.align}">
        {column.label}
      </div>
    {/if}
  {/each}
</div>
