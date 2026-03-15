<script lang="ts">
	import type { Track, PreviewInfo } from '$shared/types'
	import { getTrackDisplayName, getTrackDisplayArtist } from '$shared/utils'
	import { AlbumArt, AlbumArtModal, Icon, Text } from '$lib/components/common'
	import { translate } from '$shared/i18n'

	type Props = {
		track: Track | null
		previewInfo?: PreviewInfo | null
		onLocate?: () => void
		onLikeToggle?: () => void
	}

	let { track, previewInfo = null, onLocate, onLikeToggle }: Props = $props()

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
			<button
				class="block max-w-full cursor-pointer truncate text-left text-sm font-medium text-text-primary hover:underline"
				onclick={() => onLocate?.()}
			>
				{previewTrack.name}
			</button>
			<Text variant="caption" as="p" color="secondary" truncate>
				{previewInfo.release.artist || previewInfo.release.title || ''}
			</Text>
		{:else if track}
			<button
				class="block max-w-full cursor-pointer truncate text-left text-sm font-medium text-text-primary hover:underline"
				onclick={() => onLocate?.()}
			>
				{getTrackDisplayName(track)}
			</button>
			<Text variant="caption" as="p" color="secondary" truncate>
				{getTrackDisplayArtist(track)}
			</Text>
		{:else}
			<Text color="tertiary">{$translate('player.noTrackSelected')}</Text>
		{/if}
	</div>

	<!-- Like button (preview only) -->
	{#if previewInfo && previewTrack}
		<button
			class="flex-shrink-0 cursor-pointer transition-colors {previewTrack.is_liked
				? 'text-brand-primary'
				: 'text-secondary hover:text-primary'}"
			onclick={(e) => {
				e.currentTarget.animate([{ transform: 'scale(1)' }, { transform: 'scale(1.35)' }, { transform: 'scale(1)' }], {
					duration: 300,
					easing: 'ease-out',
				})
				onLikeToggle?.()
			}}
		>
			<Icon name="heart" class="h-3.5 w-3.5" fill={previewTrack.is_liked} />
		</button>
	{/if}
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
