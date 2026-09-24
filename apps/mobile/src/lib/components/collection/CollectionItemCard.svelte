<script lang="ts">
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { translate } from '$shared/i18n'
	import type { CollectionItem } from '$shared/types'
	import { lightTap } from '$lib/utils/haptics'
	import { mobileUIStore } from '$lib/stores/mobileUI'

	// A purchased item that is NOT in the discovery collection: same 72px row geometry as
	// `ReleaseCard` (the Purchased view mixes both under one virtualizer), but with remote-only
	// artwork and external affordances — tap opens the item's Bandcamp page, the trailing "+"
	// prefills the add-release sheet with its URL (the share-intent intake path).
	type Props = { item: CollectionItem }
	let { item }: Props = $props()

	let artworkFailed = $state(false)

	function openPage() {
		void lightTap()
		void openUrl(item.url).catch(() => {})
	}

	function addToDiscovery(e: MouseEvent) {
		e.stopPropagation()
		void lightTap()
		mobileUIStore.openAddReleaseWithUrl(item.url)
	}
</script>

<div class="flex h-[72px] items-center gap-3 px-3">
	<button type="button" class="flex min-w-0 flex-1 items-center gap-3 text-left" onclick={openPage}>
		{#if item.artworkUrl && !artworkFailed}
			<img
				src={item.artworkUrl}
				alt=""
				loading="lazy"
				class="h-12 w-12 flex-shrink-0 rounded object-cover"
				onerror={() => (artworkFailed = true)}
			/>
		{:else}
			<div class="flex h-12 w-12 flex-shrink-0 items-center justify-center rounded bg-surface-2 text-text-tertiary">
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
					<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
				</svg>
			</div>
		{/if}
		<div class="flex min-w-0 flex-1 flex-col leading-tight">
			<span class="truncate text-sm font-medium text-text-primary">
				{item.title ?? $translate('common.untitled')}
			</span>
			<span class="truncate text-xs text-text-secondary">
				{item.artist ?? $translate('common.unknownArtist')}
			</span>
			<!-- Third line mirrors ReleaseCardContent's fixed label line (row-height invariant). -->
			<span class="flex min-w-0 items-center gap-1 text-xs text-text-tertiary">
				<svg
					class="h-3 w-3 flex-shrink-0"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
					aria-hidden="true"
				>
					<path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
					<path d="M15 3h6v6M10 14L21 3" />
				</svg>
				<span class="truncate">{$translate('collection.notInCrate', { values: { appName: 'Crate' } })}</span>
			</span>
		</div>
	</button>
	<button
		type="button"
		aria-label={$translate('collection.addToDiscovery')}
		class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md border border-stroke text-text-secondary active:bg-surface-2"
		onclick={addToDiscovery}
	>
		<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
			<path d="M12 5v14M5 12h14" stroke-linecap="round" stroke-linejoin="round" />
		</svg>
	</button>
</div>
