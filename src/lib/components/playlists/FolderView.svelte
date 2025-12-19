<script lang="ts">
	import type { Playlist } from '$lib/types'
	import { getPlaylistChildren } from '$lib/stores/playlists'
	import FolderCard from './FolderCard.svelte'

	type Props = {
		folderId: string
		playlists: Playlist[]
		onSelect: (playlist: Playlist) => void
	}

	let { folderId, playlists, onSelect }: Props = $props()

	// Get children of this folder
	let children = $derived(getPlaylistChildren(playlists, folderId))

	// Sort: folders first, then playlists
	let sortedChildren = $derived(
		[...children].sort((a, b) => {
			if (a.is_folder && !b.is_folder) return -1
			if (!a.is_folder && b.is_folder) return 1
			return a.name.localeCompare(b.name)
		})
	)

	// Get the folder name
	let folder = $derived(playlists.find((p) => p.id === folderId))

	// Count children for each subfolder
	function getChildCount(playlist: Playlist): number {
		if (!playlist.is_folder) return 0
		return getPlaylistChildren(playlists, playlist.id).length
	}
</script>

<div class="flex h-full flex-col overflow-hidden bg-zinc-950">
	<!-- Header -->
	<div class="flex items-center gap-3 border-b border-zinc-800 px-6 py-4">
		<svg class="h-5 w-5 text-zinc-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
			<path
				stroke-linecap="round"
				stroke-linejoin="round"
				stroke-width="2"
				d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
			/>
		</svg>
		<h2 class="text-lg font-medium text-zinc-100">{folder?.name ?? 'Folder'}</h2>
		<span class="text-sm text-zinc-500">
			{children.length}
			{children.length === 1 ? 'item' : 'items'}
		</span>
	</div>

	<!-- Content -->
	<div class="flex-1 overflow-auto p-6">
		{#if sortedChildren.length === 0}
			<div class="flex h-full flex-col items-center justify-center text-zinc-500">
				<svg class="mb-3 h-12 w-12" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="1.5"
						d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
					/>
				</svg>
				<p class="text-sm">This folder is empty</p>
			</div>
		{:else}
			<div class="grid grid-cols-2 gap-4 lg:grid-cols-3 xl:grid-cols-4">
				{#each sortedChildren as child (child.id)}
					<FolderCard playlist={child} childCount={getChildCount(child)} onclick={() => onSelect(child)} />
				{/each}
			</div>
		{/if}
	</div>
</div>
