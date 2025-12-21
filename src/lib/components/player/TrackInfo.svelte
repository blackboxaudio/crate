<script lang="ts">
	import type { Track } from '$lib/types'
	import { getTrackDisplayName, getTrackDisplayArtist } from '$lib/utils'
	import { AlbumArt, AlbumArtModal } from '$lib/components/common'
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
			<p class="truncate text-sm font-medium text-text-primary">
				{getTrackDisplayName(track)}
			</p>
			<p class="truncate text-xs text-text-secondary">
				{getTrackDisplayArtist(track)}
			</p>
		{:else}
			<p class="text-sm text-text-tertiary">{$translate('player.noTrackSelected')}</p>
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
