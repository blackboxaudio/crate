<script lang="ts">
	import { onMount } from 'svelte'
	import { translate } from '$shared/i18n'
	import { playlistsStore } from '$shared/stores/playlists'
	import { tagsStore } from '$shared/stores/tags'
	import MobileList from '$lib/components/common/MobileList.svelte'
	import MobileListItem from '$lib/components/common/MobileListItem.svelte'

	// Playlists tab: the user's playlists plus tag categories. Owns its own scroll container (the shell
	// frame is overflow-hidden and reserves the bottom inset for the tab bar / mini-player). Folder-tree
	// navigation and tag editing are later issues; this is a flat read-only listing for now.
	onMount(() => {
		playlistsStore.load()
		tagsStore.load()
	})

	// Flat list of non-folder playlists (folder-tree navigation is a later issue).
	const playlists = $derived($playlistsStore.playlists.filter((p) => !p.is_folder))
	const categories = $derived($tagsStore.categories)
</script>

<div class="h-full overflow-y-auto" style="padding-bottom: var(--mini-player-inset, 0px)">
	<MobileList title={$translate('nav.playlists')} isEmpty={playlists.length === 0} empty={emptyPlaylists}>
		{#each playlists as playlist (playlist.id)}
			<MobileListItem>
				{#snippet trailing()}
					<span class="text-xs tabular-nums">{playlist.track_count}</span>
				{/snippet}
				<span class="truncate">{playlist.name}</span>
			</MobileListItem>
		{/each}
	</MobileList>

	<section class="flex flex-col">
		<h2 class="px-4 pt-4 pb-1 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
			{$translate('nav.tags')}
		</h2>
		{#if categories.length === 0}
			<div class="px-4 py-6 text-sm text-text-secondary">{$translate('tags.noTags')}</div>
		{:else}
			{#each categories as category (category.id)}
				<div class="px-4 py-2">
					<h3 class="mb-1.5 text-sm font-medium text-text-secondary">{category.name}</h3>
					<div class="flex flex-wrap gap-1.5">
						{#each category.tags as tag (tag.id)}
							{@const color = tag.color ?? category.color ?? '#6366f1'}
							<span
								class="rounded px-2 py-1 text-xs font-medium"
								style="background-color: {color}20; color: {color}; border: 1px solid {color}40;"
							>
								{tag.name}
							</span>
						{/each}
					</div>
				</div>
			{/each}
		{/if}
	</section>
</div>

{#snippet emptyPlaylists()}
	{$translate('playlists.noPlaylists')}
{/snippet}
