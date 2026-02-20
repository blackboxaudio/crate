<script lang="ts">
	import type { Track, PreviewInfo } from '$lib/types'
	import { getTrackDisplayName, getTrackDisplayArtist } from '$lib/utils'
	import { AlbumArt, AlbumArtModal, Text } from '$lib/components/common'
	import { translate } from '$lib/i18n'

	type Props = {
		track: Track | null
		previewInfo?: PreviewInfo | null
	}

	let { track, previewInfo = null }: Props = $props()

	let showArtworkModal = $state(false)

	const previewTrack = $derived(previewInfo ? previewInfo.release.tracks[previewInfo.trackIndex] : null)
	const previewArtworkPath = $derived(previewInfo?.release.artwork_path ?? null)
	const previewArtworkUrl = $derived(previewInfo?.release.artwork_url ?? null)

	function handleArtworkClick() {
		if (track?.artwork_path || previewArtworkPath || previewArtworkUrl) {
			showArtworkModal = true
		}
	}
</script>

<div class="flex min-w-0 items-center gap-3">
	<!-- Album art -->
	{#if previewInfo}
		<AlbumArt
			artworkPath={previewArtworkPath}
			artworkUrl={previewArtworkUrl}
			size="md"
			onclick={handleArtworkClick}
			class={previewArtworkPath || previewArtworkUrl ? 'cursor-zoom-in' : ''}
		/>
	{:else}
		<AlbumArt
			artworkPath={track?.artwork_path ?? null}
			size="md"
			onclick={handleArtworkClick}
			class={track?.artwork_path ? 'cursor-zoom-in' : ''}
		/>
	{/if}

	<!-- Track info -->
	<div class="min-w-0 flex-1">
		{#if previewInfo && previewTrack}
			<Text variant="body-2" truncate>
				{previewTrack.name}
			</Text>
			<Text variant="caption" as="p" color="secondary" truncate>
				{previewInfo.release.artist || previewInfo.release.title || ''}
			</Text>
		{:else if track}
			<Text variant="body-2" truncate>
				{getTrackDisplayName(track)}
			</Text>
			<Text variant="caption" as="p" color="secondary" truncate>
				{getTrackDisplayArtist(track)}
			</Text>
		{:else}
			<Text color="tertiary">{$translate('player.noTrackSelected')}</Text>
		{/if}
	</div>
</div>

{#if showArtworkModal && (track || previewInfo)}
	<AlbumArtModal
		open={showArtworkModal}
		artworkPath={previewInfo ? previewArtworkPath : (track?.artwork_path ?? null)}
		artworkUrl={previewInfo ? previewArtworkUrl : null}
		trackTitle={previewInfo && previewTrack ? previewTrack.name : track ? getTrackDisplayName(track) : ''}
		onClose={() => (showArtworkModal = false)}
	/>
{/if}
