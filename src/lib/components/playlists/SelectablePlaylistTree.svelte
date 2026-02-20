<script lang="ts">
	import type { Playlist } from '$lib/types'
	import { buildPlaylistTree, type PlaylistTreeNode } from '$lib/stores'
	import PlaylistItem from './PlaylistItem.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import { SvelteSet } from 'svelte/reactivity'

	type Props = {
		playlists: Playlist[]
		selectedIds: Set<string>
		onToggle: (playlistId: string, isFolder: boolean) => void
	}

	let { playlists, selectedIds, onToggle }: Props = $props()

	// Session-only expanded state (not persisted to localStorage)
	let expandedIds = $state<Set<string>>(new Set())

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

	function isSelected(playlistId: string): boolean {
		return selectedIds.has(playlistId)
	}
</script>

{#snippet renderNode(node: PlaylistTreeNode, depth: number)}
	<PlaylistItem
		playlist={node.playlist}
		{playlists}
		{depth}
		expanded={expandedIds.has(node.playlist.id)}
		hasChildren={node.children.length > 0}
		onToggle={() => toggleExpanded(node.playlist.id)}
		showCheckbox={true}
		checkboxChecked={isSelected(node.playlist.id)}
		onCheckboxChange={() => onToggle(node.playlist.id, node.playlist.is_folder)}
		disableDrag={true}
		disableContextMenu={true}
	/>

	{#if node.playlist.is_folder && expandedIds.has(node.playlist.id)}
		{#each node.children as child, index (index)}
			{@render renderNode(child, depth + 1)}
		{/each}
	{/if}
{/snippet}

<div role="tree" tabindex="0" class="space-y-0.5">
	{#each tree as node, index (index)}
		{@render renderNode(node, 0)}
	{/each}

	{#if playlists.length === 0}
		<Text variant="caption" class="py-4 text-center">No playlists available</Text>
	{/if}
</div>
