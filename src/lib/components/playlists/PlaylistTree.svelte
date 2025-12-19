<script lang="ts">
	import type { Playlist } from '$lib/types'
	import { buildPlaylistTree, type PlaylistTreeNode } from '$lib/stores'
	import PlaylistItem from './PlaylistItem.svelte'
	import { SvelteSet } from 'svelte/reactivity'

	type Props = {
		playlists: Playlist[]
		selectedId?: string | null
		onSelect?: (playlist: Playlist) => void
		onContextMenu?: (e: MouseEvent, playlist: Playlist) => void
		onTracksDrop?: (playlistId: string, trackIds: string[]) => void
	}

	let { playlists, selectedId = null, onSelect, onContextMenu, onTracksDrop }: Props = $props()

	let expandedIds = $state<Set<string>>(new Set())

	let tree = $derived(buildPlaylistTree(playlists))

	function toggleExpanded(id: string) {
		const newExpanded = new SvelteSet(expandedIds)
		if (newExpanded.has(id)) {
			newExpanded.delete(id)
		} else {
			newExpanded.add(id)
		}
		expandedIds = newExpanded
	}
</script>

{#snippet renderNode(node: PlaylistTreeNode, depth: number)}
	<PlaylistItem
		playlist={node.playlist}
		selected={selectedId === node.playlist.id}
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
	/>

	{#if node.playlist.is_folder && expandedIds.has(node.playlist.id)}
		{#each node.children as child, index (index)}
			{@render renderNode(child, depth + 1)}
		{/each}
	{/if}
{/snippet}

<div role="tree" class="space-y-0.5">
	{#each tree as node, index (index)}
		{@render renderNode(node, 0)}
	{/each}

	{#if playlists.length === 0}
		<p class="py-4 text-center text-xs text-text-tertiary">No playlists yet</p>
	{/if}
</div>
