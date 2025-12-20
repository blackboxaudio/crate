<script lang="ts">
	import { open } from '@tauri-apps/plugin-dialog'
	import AlbumArt from '$lib/components/common/AlbumArt.svelte'
	import Button from '$lib/components/common/Button.svelte'
	import type { ArtworkSource, BulkEditValue } from '$lib/types'

	type Props = {
		artworkPath: BulkEditValue<string>
		artworkSource: BulkEditValue<ArtworkSource>
		trackCount: number
		onAdd: (filePath: string) => void
		onRemove: () => void
		onReextract: () => void
	}

	let { artworkPath, artworkSource, trackCount, onAdd, onRemove, onReextract }: Props = $props()

	let hasArtwork = $derived(artworkPath.count > 0)
	let canReextract = $derived(trackCount === 1 && artworkSource.value === 'user_provided' && !artworkSource.mixed)
	let displayPath = $derived(artworkPath.mixed ? null : artworkPath.value)

	async function handleAdd() {
		const selected = await open({
			multiple: false,
			filters: [
				{
					name: 'Images',
					extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'],
				},
			],
		})

		if (selected && typeof selected === 'string') {
			onAdd(selected)
		}
	}
</script>

<div class="flex flex-col items-center gap-3">
	<div class="relative">
		<AlbumArt artworkPath={displayPath} size="lg" class="rounded-lg" />
		{#if artworkPath.mixed}
			<div
				class="absolute inset-0 flex items-center justify-center rounded-lg bg-surface-0/80 text-sm text-text-secondary"
			>
				Multiple artworks
			</div>
		{/if}
	</div>

	<div class="flex gap-2">
		<Button variant="secondary" size="sm" onclick={handleAdd}>
			{hasArtwork ? 'Replace' : 'Add'} Artwork
		</Button>
		{#if hasArtwork}
			<Button variant="ghost" size="sm" onclick={onRemove}>Remove</Button>
		{/if}
	</div>

	{#if canReextract}
		<button
			class="text-xs text-text-tertiary transition-colors hover:cursor-pointer hover:text-text-secondary"
			onclick={onReextract}
		>
			Re-extract from file
		</button>
	{/if}
</div>
