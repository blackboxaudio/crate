<script lang="ts">
  import { uiStore, searchQuery } from '$lib/stores';
  import { libraryStore } from '$lib/stores/library';

  let inputValue = $state('');

  // Debounced search
  let debounceTimer: ReturnType<typeof setTimeout>;

  function handleInput(e: Event) {
    const target = e.target as HTMLInputElement;
    inputValue = target.value;

    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      uiStore.setSearchQuery(inputValue);
      libraryStore.setSearch(inputValue);
    }, 300);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      inputValue = '';
      uiStore.clearSearch();
      libraryStore.setSearch('');
      (e.target as HTMLInputElement).blur();
    }
  }

  function handleClear() {
    inputValue = '';
    uiStore.clearSearch();
    libraryStore.setSearch('');
  }
</script>

<div class="relative">
  <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
    <svg
      class="w-4 h-4 text-zinc-500"
      fill="none"
      stroke="currentColor"
      viewBox="0 0 24 24"
    >
      <path
        stroke-linecap="round"
        stroke-linejoin="round"
        stroke-width="2"
        d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
      />
    </svg>
  </div>

  <input
    type="search"
    placeholder="Search tracks..."
    value={inputValue}
    class="w-full pl-10 pr-8 py-1.5 bg-zinc-800 border border-zinc-700 rounded-md text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
    oninput={handleInput}
    onkeydown={handleKeydown}
    onfocus={() => uiStore.setSearchFocused(true)}
    onblur={() => uiStore.setSearchFocused(false)}
  />

  {#if inputValue}
    <button
      type="button"
      aria-label="Clear search"
      class="absolute inset-y-0 right-0 pr-3 flex items-center text-zinc-500 hover:text-zinc-300"
      onclick={handleClear}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M6 18L18 6M6 6l12 12"
        />
      </svg>
    </button>
  {/if}
</div>
