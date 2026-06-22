<script lang="ts">
	import { translate } from '$shared/i18n'
	import { playlistsStore } from '$shared/stores/playlists'
	import { tagsStore } from '$shared/stores/tags'
	import Drawer from '$lib/components/common/Drawer.svelte'
	import MobileList from '$lib/components/common/MobileList.svelte'
	import MobileListItem from '$lib/components/common/MobileListItem.svelte'

	// Left navigation drawer: Playlists + Tags. Slide / scrim / swipe-to-close / edge-grab strip all come
	// from the shared `Drawer` baseline (direction="left"); the shell's left-edge open gesture is forwarded
	// as `dragOpenness` → `openProgress` for finger-follow opening. This component just supplies the content
	// and its lazy data load.
	type Props = {
		open: boolean
		dragOpenness?: number | null
		onClose: () => void
	}

	let { open, dragOpenness = null, onClose }: Props = $props()

	// Lazy-load playlists + tags the first time the drawer starts opening, so app start stays light.
	let loadedOnce = false
	$effect(() => {
		if ((open || (dragOpenness ?? 0) > 0) && !loadedOnce) {
			loadedOnce = true
			playlistsStore.load()
			tagsStore.load()
		}
	})

	// Flat list of non-folder playlists (folder-tree navigation is a later issue).
	const playlists = $derived($playlistsStore.playlists.filter((p) => !p.is_folder))
	const categories = $derived($tagsStore.categories)
</script>

<Drawer
	{open}
	{onClose}
	direction="left"
	z={50}
	scrimZ={40}
	openProgress={dragOpenness}
	ariaLabel={$translate('nav.playlists')}
	class="flex w-[85%] max-w-[320px] flex-col overflow-hidden border-r border-stroke bg-surface-1"
>
	<div class="pt-safe pl-safe flex-1 overflow-y-auto pb-6">
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
</Drawer>

{#snippet emptyPlaylists()}
	{$translate('playlists.noPlaylists')}
{/snippet}
