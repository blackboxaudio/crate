<script lang="ts">
	import { onMount } from 'svelte'
	import { translate } from '$shared/i18n'
	import { playlistsStore } from '$shared/stores/playlists'
	import MobileList from '$lib/components/common/MobileList.svelte'
	import MobileListItem from '$lib/components/common/MobileListItem.svelte'

	// Playlists tab: the user's playlists. Owns its own scroll container (the shell frame is
	// overflow-hidden and reserves the bottom inset for the tab bar / mini-player). Folder-tree navigation
	// is a later issue; this is a flat read-only listing for now. (Tags are their own tab — see TagsView.)
	onMount(() => {
		playlistsStore.load()
	})

	// Flat list of non-folder playlists (folder-tree navigation is a later issue).
	const playlists = $derived($playlistsStore.playlists.filter((p) => !p.is_folder))
</script>

<div class="h-full overflow-y-auto pt-2" style="padding-bottom: var(--mini-player-inset, 0px)">
	<MobileList isEmpty={playlists.length === 0} empty={emptyPlaylists}>
		{#each playlists as playlist (playlist.id)}
			<MobileListItem>
				{#snippet trailing()}
					<span class="text-xs tabular-nums">{playlist.track_count}</span>
				{/snippet}
				<span class="truncate">{playlist.name}</span>
			</MobileListItem>
		{/each}
	</MobileList>
</div>

{#snippet emptyPlaylists()}
	{$translate('playlists.noPlaylists')}
{/snippet}
