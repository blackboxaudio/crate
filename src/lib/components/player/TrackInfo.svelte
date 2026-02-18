<script lang="ts">
	import type { Track } from '$lib/types'
	import { getTrackDisplayName, getTrackDisplayArtist } from '$lib/utils'
	import { AlbumArt, AlbumArtModal, Text } from '$lib/components/common'
	import { translate } from '$lib/i18n'

	type Props = {
		track: Track | null
	}

	let { track }: Props = $props()

	let showArtworkModal = $state(false)

	function handleArtworkClick() {
		if (track?.artwork_path) {
			showArtworkModal = true
		}
	}
</script>

<div class="flex min-w-0 items-center gap-3">
	<!-- Album art -->
	<AlbumArt
		artworkPath={track?.artwork_path ?? null}
		size="md"
		onclick={handleArtworkClick}
		class={track?.artwork_path ? 'cursor-zoom-in' : ''}
	/>

	<!-- Track info -->
	<div class="min-w-0 flex-1">
		{#if track}
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

{#if showArtworkModal && track}
	<AlbumArtModal
		open={showArtworkModal}
		artworkPath={track.artwork_path}
		trackTitle={getTrackDisplayName(track)}
		onClose={() => (showArtworkModal = false)}
	/>
{/if}
