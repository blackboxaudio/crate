<script lang="ts">
	import { translate } from '$shared/i18n'
	import type { Playlist } from '$shared/types'
	import { buildPlaylistTree, collectDescendantIds, type PlaylistTreeNode } from '$lib/stores'
	import { slide } from 'svelte/transition'
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
	const expandedIds = new SvelteSet<string>()

	let tree = $derived(buildPlaylistTree(playlists))

	function toggleExpanded(id: string) {
		if (expandedIds.has(id)) {
			expandedIds.delete(id)
			for (const descendantId of collectDescendantIds(playlists, id)) {
				expandedIds.delete(descendantId)
			}
		} else {
			expandedIds.add(id)
		}
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
		<div transition:slide={{ duration: 150 }}>
			{#each node.children as child (child.playlist.id)}
				{@render renderNode(child, depth + 1)}
			{/each}
		</div>
	{/if}
{/snippet}

<div role="tree" tabindex="0" class="space-y-0.5">
	{#each tree as node (node.playlist.id)}
		{@render renderNode(node, 0)}
	{/each}

	{#if playlists.length === 0}
		<Text variant="caption" class="py-4 text-center">{$translate('export.noPlaylistsAvailable')}</Text>
	{/if}
</div>
