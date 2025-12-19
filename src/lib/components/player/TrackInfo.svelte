<script lang="ts">
	import type { Track } from '$lib/types'
	import { getTrackDisplayName, getTrackDisplayArtist } from '$lib/utils'
	import Icon from '$lib/components/common/Icon.svelte'

	type Props = {
		track: Track | null
	}

	let { track }: Props = $props()
</script>

<div class="flex min-w-0 items-center gap-3">
	<!-- Album art placeholder -->
	<div class="flex h-12 w-12 flex-shrink-0 items-center justify-center rounded bg-surface-2">
		{#if track}
			<Icon name="music-note" class="h-6 w-6 text-text-tertiary" />
		{/if}
	</div>

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
			<p class="text-sm text-text-tertiary">No track selected</p>
		{/if}
	</div>
</div>
