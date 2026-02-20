<script lang="ts">
	import type { Playlist } from '$lib/types'
	import { buildPlaylistTree, type PlaylistTreeNode } from '$lib/stores'
	import { getStoredSet, setStoredSet } from '$lib/utils'
	import { translate } from '$lib/i18n'
	import PlaylistItem from './PlaylistItem.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import { SvelteSet } from 'svelte/reactivity'

	const EXPANDED_STORAGE_KEY = 'expandedPlaylistIds'

	type Props = {
		playlists: Playlist[]
		selectedId?: string | null
		contextMenuItemId?: string | null
		onSelect?: (playlist: Playlist) => void
		onContextMenu?: (e: MouseEvent, playlist: Playlist) => void
		onWhitespaceContextMenu?: (e: MouseEvent) => void
		onWhitespaceClick?: () => void
		onTracksDrop?: (playlistId: string, trackIds: string[]) => void
		onPlaylistMove?: (playlistId: string, targetFolderId: string | null) => void
	}

	let {
		playlists,
		selectedId = null,
		contextMenuItemId = null,
		onSelect,
		onContextMenu,
		onWhitespaceContextMenu,
		onWhitespaceClick,
		onTracksDrop,
		onPlaylistMove,
	}: Props = $props()

	function handleContainerContextMenu(e: MouseEvent) {
		const target = e.target as HTMLElement
		if (target.closest('[role="treeitem"]')) return
		if (onWhitespaceContextMenu) {
			e.preventDefault()
			onWhitespaceContextMenu(e)
		}
	}

	function handleContainerClick(e: MouseEvent) {
		const target = e.target as HTMLElement
		if (target.closest('[role="treeitem"]')) return
		onWhitespaceClick?.()
	}

	function handleContainerKeyDown(e: KeyboardEvent) {
		if (e.key !== 'Enter' && e.key !== ' ') return
		const target = e.target as HTMLElement
		if (target.closest('[role="treeitem"]')) return
		e.preventDefault()
		onWhitespaceClick?.()
	}

	let expandedIds = $state<Set<string>>(getStoredSet(EXPANDED_STORAGE_KEY))

	$effect(() => {
		setStoredSet(EXPANDED_STORAGE_KEY, expandedIds)
	})

	let tree = $derived(buildPlaylistTree(playlists))

	function getDescendantIds(parentId: string): string[] {
		const children = playlists.filter((p) => p.parent_id === parentId)
		return children.flatMap((child) => [child.id, ...getDescendantIds(child.id)])
	}

	function toggleExpanded(id: string) {
		const newExpanded = new SvelteSet(expandedIds)
		if (newExpanded.has(id)) {
			newExpanded.delete(id)
			for (const descendantId of getDescendantIds(id)) {
				newExpanded.delete(descendantId)
			}
		} else {
			newExpanded.add(id)
		}
		expandedIds = newExpanded
	}
</script>

{#snippet renderNode(node: PlaylistTreeNode, depth: number)}
	<PlaylistItem
		playlist={node.playlist}
		{playlists}
		selected={selectedId === node.playlist.id}
		isContextMenuActive={contextMenuItemId === node.playlist.id}
		{depth}
		expanded={expandedIds.has(node.playlist.id)}
		hasChildren={node.children.length > 0}
		onclick={() => onSelect?.(node.playlist)}
		onToggle={() => toggleExpanded(node.playlist.id)}
		oncontextmenu={(e) => {
			e.preventDefault()
			onContextMenu?.(e, node.playlist)
		}}
		onTracksDrop={(trackIds) => onTracksDrop?.(node.playlist.id, trackIds)}
		onPlaylistDrop={(droppedId) => onPlaylistMove?.(droppedId, node.playlist.id)}
	/>

	{#if node.playlist.is_folder && expandedIds.has(node.playlist.id)}
		{#each node.children as child, index (index)}
			{@render renderNode(child, depth + 1)}
		{/each}
	{/if}
{/snippet}

<div
	role="tree"
	tabindex="0"
	class="h-full space-y-0.5"
	onclick={handleContainerClick}
	onkeydown={handleContainerKeyDown}
	oncontextmenu={handleContainerContextMenu}
>
	{#each tree as node, index (index)}
		{@render renderNode(node, 0)}
	{/each}

	{#if playlists.length === 0}
		<Text variant="caption" as="p" italic class="py-4 text-center">{$translate('playlists.noPlaylistsYet')}</Text>
	{/if}
</div>
