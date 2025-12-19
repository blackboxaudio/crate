<script lang="ts">
	import type { Playlist, TagCategory, Tag, UsbDevice } from '$lib/types'
	import { Button } from '$lib/components/common'
	import { PlaylistTree } from '$lib/components/playlists'
	import { TagList } from '$lib/components/tags'
	import { DeviceList } from '$lib/components/devices'

	type Props = {
		playlists: Playlist[]
		tagCategories: TagCategory[]
		devices: UsbDevice[]
		selectedPlaylistId?: string | null
		selectedTagId?: string | null
		trackCount: number
		onLibraryClick?: () => void
		onPlaylistSelect?: (playlist: Playlist) => void
		onPlaylistContextMenu?: (e: MouseEvent, playlist: Playlist) => void
		onDeviceContextMenu?: (e: MouseEvent, device: UsbDevice) => void
		onTagSelect?: (tagId: string) => void
		onTagContextMenu?: (e: MouseEvent, tag: Tag, category: TagCategory) => void
		onCategoryContextMenu?: (e: MouseEvent, category: TagCategory) => void
		onCreatePlaylist?: () => void
		onCreateFolder?: () => void
		onCreateCategory?: () => void
		onCreateTag?: (categoryId: string) => void
		onTracksDrop?: (playlistId: string, trackIds: string[]) => void
		onPlaylistMove?: (playlistId: string, targetFolderId: string | null) => void
	}

	let {
		playlists,
		tagCategories,
		devices,
		selectedPlaylistId = null,
		selectedTagId = null,
		trackCount,
		onLibraryClick,
		onPlaylistSelect,
		onPlaylistContextMenu,
		onDeviceContextMenu,
		onTagSelect,
		onTagContextMenu,
		onCategoryContextMenu,
		onCreatePlaylist,
		onCreateFolder,
		onCreateCategory,
		onCreateTag,
		onTracksDrop,
		onPlaylistMove,
	}: Props = $props()

	let activeSection = $state<'playlists' | 'tags'>('playlists')
</script>

<div class="flex h-full flex-col" ondragover={(e) => console.log('[Sidebar DragOver]', e.target)}>
	<!-- Devices -->
	<div class="p-2">
		<DeviceList {devices} onContextMenu={onDeviceContextMenu} />
	</div>

	<!-- Library -->
	<div class="p-2">
		<div class="flex items-center px-3 py-1.5">
			<span class="text-xs font-medium tracking-wide text-text-tertiary uppercase">Library</span>
			<span class="ml-auto text-xs text-text-tertiary">{trackCount}</span>
		</div>
	</div>

	<!-- Section tabs -->
	<div class="flex border-b border-stroke">
		<button
			type="button"
			class="flex flex-1 items-center justify-center gap-1.5 px-3 py-2 text-xs font-medium transition-colors {activeSection ===
			'playlists'
				? 'border-b-2 border-brand-primary text-text-primary'
				: 'text-text-tertiary hover:text-text-secondary'}"
			onclick={() => (activeSection = 'playlists')}
		>
			<svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 10h16M4 14h16M4 18h16" />
			</svg>
			Playlists
		</button>
		<button
			type="button"
			class="flex flex-1 items-center justify-center gap-1.5 px-3 py-2 text-xs font-medium transition-colors {activeSection ===
			'tags'
				? 'border-b-2 border-brand-primary text-text-primary'
				: 'text-text-tertiary hover:text-text-secondary'}"
			onclick={() => (activeSection = 'tags')}
		>
			<svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A2 2 0 013 12V7a4 4 0 014-4z"
				/>
			</svg>
			Tags
		</button>
	</div>

	<!-- Content -->
	<div
		class="flex-1 overflow-auto p-2"
		onclick={(e) => {
			if (e.target === e.currentTarget && (selectedPlaylistId || selectedTagId)) {
				onLibraryClick?.()
			}
		}}
		role="region"
	>
		{#if activeSection === 'playlists'}
			<PlaylistTree
				{playlists}
				selectedId={selectedPlaylistId}
				onSelect={onPlaylistSelect}
				onContextMenu={onPlaylistContextMenu}
				{onTracksDrop}
				{onPlaylistMove}
			/>
		{:else}
			<TagList
				categories={tagCategories}
				{selectedTagId}
				onTagClick={onTagSelect}
				{onCreateTag}
				{onTagContextMenu}
				{onCategoryContextMenu}
			/>
		{/if}
	</div>

	<!-- Actions -->
	<div class="space-y-1 border-t border-stroke p-2">
		{#if activeSection === 'playlists'}
			<Button variant="ghost" size="sm" class="w-full justify-start" onclick={onCreatePlaylist}>
				<svg class="mr-2 h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
				</svg>
				New Playlist
			</Button>
			<Button variant="ghost" size="sm" class="w-full justify-start" onclick={onCreateFolder}>
				<svg class="mr-2 h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="2"
						d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
					/>
				</svg>
				New Folder
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
