<script lang="ts">
	import { openUrl } from '@tauri-apps/plugin-opener'
	import type { CollectionGapItem } from '$shared/types'
	import { translate } from '$shared/i18n'
	import * as collectionApi from '$shared/api/collection'
	import { Modal, Button, Checkbox, Spinner, Text } from '$lib/components/common'

	// "Purchased but not in library": cross-references the whole collection against the
	// track library (fuzzy artist + album/title, computed backend-side on open) so you can
	// see which Bandcamp purchases you haven't downloaded/imported yet. Advisory only —
	// nothing is stored; re-opening recomputes.
	type Props = {
		open: boolean
		onClose: () => void
	}
	let { open, onClose }: Props = $props()

	let items = $state<CollectionGapItem[]>([])
	let loading = $state(false)
	let missingOnly = $state(true)

	$effect(() => {
		if (!open) return
		loading = true
		collectionApi
			.getCollectionLibraryGap()
			.then((result) => (items = result))
			.catch(() => (items = []))
			.finally(() => (loading = false))
	})

	const displayed = $derived(missingOnly ? items.filter((g) => !g.inLibrary) : items)
	const missingCount = $derived(items.filter((g) => !g.inLibrary).length)
</script>

<Modal {open} {onClose} title={$translate('collection.gap.title')} size="lg">
	<div class="flex max-h-[60vh] min-h-48 flex-col gap-3">
		{#if loading}
			<div class="flex flex-1 items-center justify-center py-10"><Spinner /></div>
		{:else if items.length === 0}
			<div class="flex flex-1 items-center justify-center py-10">
				<Text variant="caption">{$translate('collection.noItems')}</Text>
			</div>
		{:else}
			<div class="flex items-center justify-between">
				<Text variant="caption">
					{$translate('collection.gap.missingCount', { values: { count: missingCount } })}
				</Text>
				<Checkbox
					checked={missingOnly}
					onchange={(c) => (missingOnly = c)}
					label={$translate('collection.gap.missingOnly')}
				/>
			</div>
			<div class="flex-1 divide-y divide-stroke-subtle overflow-y-auto rounded-md border border-stroke">
				{#each displayed as gap (gap.item.id)}
					<div class="flex items-center gap-3 px-3 py-2">
						{#if gap.item.artworkUrl}
							<img src={gap.item.artworkUrl} alt="" loading="lazy" class="h-8 w-8 rounded object-cover" />
						{:else}
							<div class="h-8 w-8 rounded bg-surface-2"></div>
						{/if}
						<div class="flex min-w-0 flex-1 flex-col">
							<Text as="span" size="sm" weight="medium" truncate>
								{gap.item.title ?? $translate('common.untitled')}
							</Text>
							<Text as="span" variant="caption" truncate>
								{gap.item.artist ?? $translate('common.unknownArtist')}
							</Text>
						</div>
						<span
							class="shrink-0 rounded-full px-1.5 py-0.5 text-[10px] leading-none font-medium {gap.inLibrary
								? 'bg-emerald-500/15 text-emerald-500'
								: 'bg-orange-500/15 text-orange-500'}"
						>
							{$translate(gap.inLibrary ? 'collection.gap.inLibrary' : 'collection.gap.notInLibrary')}
						</span>
						<Button variant="ghost" size="sm" onclick={() => void openUrl(gap.item.url).catch(() => {})}>
							{$translate('discovery.openInBrowser')}
						</Button>
					</div>
				{/each}
			</div>
		{/if}
	</div>

	{#snippet footer()}
		<Button variant="secondary" onclick={onClose}>{$translate('common.close')}</Button>
	{/snippet}
</Modal>
