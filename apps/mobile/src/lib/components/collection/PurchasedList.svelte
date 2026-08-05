<script lang="ts">
	import { translate } from '$shared/i18n'
	import type { CollectionItem, DiscoveryRelease } from '$shared/types'
	import { collectionItems, collectionStore, hasLinkedCollection } from '$shared/stores/collection'
	import { discoveryStore } from '$shared/stores/discovery'
	import { mobileUIStore, DISCOVERY_ROW_HEIGHT } from '$lib/stores/mobileUI'
	import ReleaseFeedList from '$lib/components/discovery/ReleaseFeedList.svelte'
	import ReleaseCard from '$lib/components/discovery/ReleaseCard.svelte'
	import CollectionItemCard from './CollectionItemCard.svelte'

	// The Purchased view: the whole linked Bandcamp collection as one virtualized list, newest
	// purchases first (the backend's order). Items matched to a discovery release render the normal
	// `ReleaseCard` (badges, swipe actions, detail on tap); unmatched items render the lightweight
	// `CollectionItemCard` (open on Bandcamp / add to discovery). Shown when the toolbar's Purchased
	// filter is active — the discovery feed swaps to this list, and the toolbar's search box keeps
	// working (client-side, over artist/title).
	type Row = { id: string; item: CollectionItem; release: DiscoveryRelease | undefined }

	const releaseById = $derived(new Map($discoveryStore.releases.map((r) => [r.id, r])))
	const rows = $derived.by(() => {
		const search = ($discoveryStore.filter.search ?? '').trim().toLowerCase()
		const all: Row[] = $collectionItems.map((item) => ({
			id: item.id,
			item,
			release: item.matchedReleaseId ? releaseById.get(item.matchedReleaseId) : undefined,
		}))
		if (!search) return all
		return all.filter(({ item, release }) => {
			const artist = release?.artist ?? item.artist
			const title = release?.title ?? item.title
			return artist?.toLowerCase().includes(search) || title?.toLowerCase().includes(search)
		})
	})

	async function refresh() {
		await collectionStore.refreshAllAccounts()
	}
</script>

<ReleaseFeedList
	releases={rows}
	rowHeight={DISCOVERY_ROW_HEIGHT}
	onRefresh={refresh}
	empty={emptyState}
	row={itemRow}
/>

{#snippet itemRow({ release: row }: { release: Row })}
	{#if row.release}
		<ReleaseCard release={row.release} />
	{:else}
		<CollectionItemCard item={row.item} />
	{/if}
{/snippet}

{#snippet emptyState()}
	<div class="flex h-full flex-col items-center justify-center gap-5 px-8 text-center">
		<div class="flex h-16 w-16 items-center justify-center rounded-full bg-surface-2 text-text-tertiary">
			<svg
				viewBox="0 0 24 24"
				class="h-8 w-8"
				fill="none"
				stroke="currentColor"
				stroke-width="1.5"
				stroke-linecap="round"
				stroke-linejoin="round"
			>
				<path d="M6 8h12l-1.2 12H7.2L6 8z" />
				<path d="M9 8V6a3 3 0 0 1 6 0v2" />
			</svg>
		</div>
		{#if !$hasLinkedCollection}
			<div class="space-y-1">
				<p class="text-base font-semibold text-text-primary">{$translate('collection.emptyTitle')}</p>
				<p class="text-sm text-text-secondary">{$translate('collection.emptyHint')}</p>
			</div>
			<button
				type="button"
				class="inline-flex items-center gap-1.5 rounded-lg bg-brand-primary px-4 py-2.5 text-sm font-semibold text-white active:opacity-90"
				onclick={() => mobileUIStore.openSettings('collection')}
			>
				{$translate('settings.collection.linkAccount')}
			</button>
		{:else}
			<p class="text-sm text-text-secondary">{$translate('collection.noItems')}</p>
		{/if}
	</div>
{/snippet}
