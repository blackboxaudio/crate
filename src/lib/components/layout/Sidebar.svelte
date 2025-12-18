<script lang="ts">
	import type { Playlist, TagCategory } from '$lib/types'
	import { Button } from '$lib/components/common'
	import { PlaylistTree } from '$lib/components/playlists'
	import { TagList } from '$lib/components/tags'

	type Props = {
		playlists: Playlist[]
		tagCategories: TagCategory[]
		selectedPlaylistId?: string | null
		selectedTagId?: string | null
		trackCount: number
		onLibraryClick?: () => void
		onPlaylistSelect?: (playlist: Playlist) => void
		onTagSelect?: (tagId: string) => void
		onCreatePlaylist?: () => void
		onCreateCategory?: () => void
	}

	let {
		playlists,
		tagCategories,
		selectedPlaylistId = null,
		selectedTagId = null,
		trackCount,
		onLibraryClick,
		onPlaylistSelect,
		onTagSelect,
		onCreatePlaylist,
		onCreateCategory,
	}: Props = $props()

	let activeSection = $state<'playlists' | 'tags'>('playlists')
</script>

<div class="flex h-full flex-col border-r border-zinc-800 bg-zinc-900">
	<!-- Library -->
	<div class="p-2">
		<button
			type="button"
			class="flex w-full items-center gap-2 rounded px-3 py-2 transition-colors {!selectedPlaylistId && !selectedTagId
				? 'bg-zinc-800 text-zinc-100'
				: 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-200'}"
			onclick={onLibraryClick}
		>
			<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"
				/>
			</svg>
			<span class="flex-1 text-left text-sm font-medium">Library</span>
			<span class="text-xs text-zinc-500">{trackCount}</span>
		</button>
	</div>

	<!-- Section tabs -->
	<div class="flex border-b border-zinc-800">
		<button
			type="button"
			class="flex-1 px-3 py-2 text-xs font-medium transition-colors {activeSection === 'playlists'
				? 'border-b-2 border-blue-500 text-zinc-100'
				: 'text-zinc-500 hover:text-zinc-300'}"
			onclick={() => (activeSection = 'playlists')}
		>
			Playlists
		</button>
		<button
			type="button"
			class="flex-1 px-3 py-2 text-xs font-medium transition-colors {activeSection === 'tags'
				? 'border-b-2 border-blue-500 text-zinc-100'
				: 'text-zinc-500 hover:text-zinc-300'}"
			onclick={() => (activeSection = 'tags')}
		>
			Tags
		</button>
	</div>

	<!-- Content -->
	<div class="flex-1 overflow-auto p-2">
		{#if activeSection === 'playlists'}
			<PlaylistTree {playlists} selectedId={selectedPlaylistId} onSelect={onPlaylistSelect} />
		{:else}
			<TagList categories={tagCategories} {selectedTagId} onTagClick={onTagSelect} />
		{/if}
	</div>

	<!-- Actions -->
	<div class="border-t border-zinc-800 p-2">
		{#if activeSection === 'playlists'}
			<Button variant="ghost" size="sm" class="w-full justify-start" onclick={onCreatePlaylist}>
				<svg class="mr-2 h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
				</svg>
				New Playlist
			</Button>
		{:else}
			<Button variant="ghost" size="sm" class="w-full justify-start" onclick={onCreateCategory}>
				<svg class="mr-2 h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
				</svg>
				New Category
			</Button>
		{/if}
	</div>
</div>
