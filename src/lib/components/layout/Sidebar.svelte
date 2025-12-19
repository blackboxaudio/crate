<script lang="ts">
	import type { Playlist, TagCategory, Tag, UsbDevice } from '$lib/types'
	import { Button } from '$lib/components/common'
	import { PlaylistTree } from '$lib/components/playlists'
	import { TagList } from '$lib/components/tags'
	import { DeviceList } from '$lib/components/devices'
	import Icon from '$lib/components/common/Icon.svelte'

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

<div class="flex h-full flex-col">
	<DeviceList {devices} onContextMenu={onDeviceContextMenu} />

	<!-- Library -->
	<div class="mt-2 p-2">
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
				: 'border-b-2 border-[#00000000] text-text-tertiary hover:cursor-pointer hover:text-text-secondary'}"
			onclick={() => (activeSection = 'playlists')}
		>
			<Icon name="grid" class="h-3.5 w-3.5" />
			Playlists
		</button>
		<button
			type="button"
			class="flex flex-1 items-center justify-center gap-1.5 px-3 py-2 text-xs font-medium transition-colors {activeSection ===
			'tags'
				? 'border-b-2 border-brand-primary text-text-primary'
				: 'border-b-2 border-[#00000000] text-text-tertiary hover:cursor-pointer hover:text-text-secondary'}"
			onclick={() => (activeSection = 'tags')}
		>
			<Icon name="tag" class="h-3.5 w-3.5" />
			Tags
		</button>
	</div>

	<!-- Content -->
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div
		class="flex-1 overflow-auto p-2"
		onclick={(e) => {
			if (e.target === e.currentTarget && (selectedPlaylistId || selectedTagId)) {
				onLibraryClick?.()
			}
		}}
		onkeydown={(e) => {
			if (e.key === 'Escape' && (selectedPlaylistId || selectedTagId)) {
				onLibraryClick?.()
			}
		}}
		role="region"
		tabindex="-1"
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
				<Icon name="plus" class="mr-2 h-4 w-4" />
				New Playlist
			</Button>
			<Button variant="ghost" size="sm" class="w-full justify-start" onclick={onCreateFolder}>
				<Icon name="folder" class="mr-2 h-4 w-4" />
				New Folder
			</Button>
		{:else}
			<Button variant="ghost" size="sm" class="w-full justify-start" onclick={onCreateCategory}>
				<Icon name="plus" class="mr-2 h-4 w-4" />
				New Category
			</Button>
		{/if}
	</div>
</div>
