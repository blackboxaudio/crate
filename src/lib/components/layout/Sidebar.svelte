<script lang="ts">
	import type { Playlist, TagCategory, Tag, TagSelectionState, UsbDevice } from '$lib/types'
	import { Button, Text } from '$lib/components/common'
	import { PlaylistTree } from '$lib/components/playlists'
	import { TagList } from '$lib/components/tags'
	import { DeviceList } from '$lib/components/devices'
	import Icon from '$lib/components/common/Icon.svelte'
	import { isDev } from '$lib/stores'
	import { translate } from '$lib/i18n'

	type Props = {
		playlists: Playlist[]
		tagCategories: TagCategory[]
		devices: UsbDevice[]
		selectedPlaylistId?: string | null
		selectedFolderId?: string | null
		contextMenuPlaylistId?: string | null
		selectedTagIds?: string[]
		selectedTrackIds?: Set<string>
		selectedTreeIds?: Set<string>
		tagStates?: Map<string, TagSelectionState>
		tagCounts?: Map<string, number>
		trackCount: number
		showHeader?: boolean
		onLibraryClick?: () => void
		onPlaylistSelect?: (playlist: Playlist) => void
		onPlaylistItemClick?: (playlist: Playlist, selectedIds: Set<string>, isModifierClick: boolean) => void
		onPlaylistContextMenu?: (e: MouseEvent, playlist: Playlist) => void
		onPlaylistMultiContextMenu?: (e: MouseEvent, playlists: Playlist[]) => void
		onPlaylistTreeContextMenu?: (e: MouseEvent) => void
		onDeviceContextMenu?: (e: MouseEvent, device: UsbDevice) => void
		onCancelExport?: () => void
		onTagSelect?: (tagId: string) => void
		onTagToggle?: (tagId: string, state: TagSelectionState) => void
		onTagContextMenu?: (e: MouseEvent, tag: Tag, category: TagCategory) => void
		onCategoryContextMenu?: (e: MouseEvent, category: TagCategory) => void
		onCreatePlaylist?: () => void
		onCreateSmartPlaylist?: () => void
		onCreateFolder?: () => void
		onCreateCategory?: () => void
		onCreateTag?: (categoryId: string) => void
		onTagsWhitespaceContextMenu?: (e: MouseEvent) => void
		onTracksDrop?: (playlistId: string, trackIds: string[]) => void
		onPlaylistMove?: (playlistId: string, targetFolderId: string | null) => void
	}

	let {
		playlists,
		tagCategories,
		devices,
		selectedPlaylistId = null,
		selectedFolderId = null,
		contextMenuPlaylistId = null,
		selectedTagIds = [],
		selectedTrackIds,
		selectedTreeIds = new Set<string>(),
		tagStates,
		tagCounts,
		trackCount,
		showHeader = true,
		onLibraryClick,
		onPlaylistSelect,
		onPlaylistItemClick,
		onPlaylistContextMenu,
		onPlaylistMultiContextMenu,
		onPlaylistTreeContextMenu,
		onDeviceContextMenu,
		onCancelExport,
		onTagSelect,
		onTagToggle,
		onTagContextMenu,
		onCategoryContextMenu,
		onCreatePlaylist,
		onCreateSmartPlaylist,
		onCreateFolder,
		onCreateCategory,
		onCreateTag,
		onTagsWhitespaceContextMenu,
		onTracksDrop,
		onPlaylistMove,
	}: Props = $props()

	let activeSection = $state<'playlists' | 'tags'>('playlists')

	// When tracks are selected and we're on the Tags tab, enable toggle mode
	let isTagToggleMode = $derived(activeSection === 'tags' && (selectedTrackIds?.size ?? 0) > 0)
</script>

<div class="flex h-full flex-col rounded-tr-md bg-surface-1">
	{#if showHeader}
		<!-- Logo section -->
		<div class="flex items-center justify-center gap-2 py-4">
			<Icon name="logo" class="h-6 w-6 text-brand-primary" fill />
			<Text variant="header-1" as="span" weight="bold">Crate</Text>
			{#if $isDev}
				<span class="rounded bg-amber-500/20 px-1.5 py-0.5 text-xs font-medium text-amber-500"> DEV </span>
			{/if}
		</div>
	{/if}

	<DeviceList {devices} onContextMenu={onDeviceContextMenu} {onCancelExport} />

	<!-- Library -->
	<div class="mx-0 border-t border-stroke px-2 pt-6">
		<div class="-mx-0 flex items-center px-3 py-1.5">
			<Text variant="header-4">{$translate('nav.library')}</Text>
			<Text variant="caption" class="mr-1 ml-auto">{trackCount}</Text>
		</div>
	</div>

	<!-- Section tabs -->
	<div class="relative mx-0 mt-1 flex border-b border-stroke">
		<!-- Sliding indicator -->
		<div
			class="absolute bottom-0 h-0.5 w-1/2 bg-brand-primary transition-transform duration-200 ease-out motion-reduce:transition-none"
			style="transform: translateX({activeSection === 'playlists' ? '0%' : '100%'})"
		></div>
		<button
			type="button"
			class="flex flex-1 items-center justify-center gap-1.5 px-3 py-2 text-xs font-medium transition-colors {activeSection ===
			'playlists'
				? 'text-text-primary'
				: 'text-text-tertiary hover:cursor-pointer hover:text-text-secondary'}"
			onclick={() => (activeSection = 'playlists')}
		>
			<Icon name="grid" class="h-3.5 w-3.5" />
			{$translate('nav.playlists')}
		</button>
		<button
			type="button"
			class="flex flex-1 items-center justify-center gap-1.5 px-3 py-2 text-xs font-medium transition-colors {activeSection ===
			'tags'
				? 'text-text-primary'
				: 'text-text-tertiary hover:cursor-pointer hover:text-text-secondary'}"
			onclick={() => (activeSection = 'tags')}
		>
			<Icon name="tag" class="h-3.5 w-3.5" />
			{$translate('nav.tags')}
		</button>
	</div>

	<!-- Content -->
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div
		class="flex-1 overflow-auto p-2"
		onclick={(e) => {
			if (e.target === e.currentTarget && (selectedPlaylistId || selectedTagIds.length > 0)) {
				onLibraryClick?.()
			}
		}}
		onkeydown={(e) => {
			if (e.key === 'Escape' && (selectedPlaylistId || selectedTagIds.length > 0)) {
				onLibraryClick?.()
			}
		}}
		ondragenter={(e) =>
			console.log('[Sidebar] dragenter', { types: e.dataTransfer?.types ? Array.from(e.dataTransfer.types) : [] })}
		role="region"
		tabindex="-1"
	>
		{#if activeSection === 'playlists'}
			<PlaylistTree
				{playlists}
				selectedId={selectedPlaylistId ?? selectedFolderId}
				selectedIds={selectedTreeIds}
				contextMenuItemId={contextMenuPlaylistId}
				onSelect={onPlaylistSelect}
				onItemClick={onPlaylistItemClick}
				onContextMenu={onPlaylistContextMenu}
				onMultiContextMenu={onPlaylistMultiContextMenu}
				onWhitespaceContextMenu={onPlaylistTreeContextMenu}
				onWhitespaceClick={onLibraryClick}
				{onTracksDrop}
				{onPlaylistMove}
			/>
		{:else}
			<TagList
				categories={tagCategories}
				selectedTagId={isTagToggleMode ? null : selectedTagIds.length > 0 ? selectedTagIds[0] : null}
				isToggleMode={isTagToggleMode}
				{tagStates}
				{tagCounts}
				selectedTrackCount={selectedTrackIds?.size ?? 0}
				onTagClick={onTagSelect}
				{onTagToggle}
				{onCreateTag}
				{onTagContextMenu}
				{onCategoryContextMenu}
				onWhitespaceContextMenu={onTagsWhitespaceContextMenu}
			/>
		{/if}
	</div>

	<!-- Actions -->
	<div class="space-y-1 border-t border-stroke p-2">
		{#if activeSection === 'playlists'}
			<Button variant="ghost" size="sm" class="w-full justify-start" onclick={onCreateFolder}>
				<Icon name="folder" class="mr-2 h-4 w-4" />
				{$translate('playlists.newFolder')}
			</Button>
			<Button variant="ghost" size="sm" class="w-full justify-start" onclick={onCreatePlaylist}>
				<Icon name="music-note" class="mr-2 h-4 w-4" />
				{$translate('playlists.newPlaylist')}
			</Button>
			<Button variant="ghost" size="sm" class="w-full justify-start" onclick={onCreateSmartPlaylist}>
				<Icon name="bolt" class="mr-2 h-4 w-4" />
				{$translate('playlists.newSmartPlaylist')}
			</Button>
		{:else}
			<Button
				variant="ghost"
				size="sm"
				class="w-full justify-start"
				onclick={onCreateCategory}
				disabled={tagCategories.length >= 4}
			>
				<Icon name="plus" class="mr-2 h-4 w-4" />
				{$translate('tags.newCategory')}
			</Button>
		{/if}
	</div>
</div>
