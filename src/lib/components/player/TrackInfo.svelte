<script lang="ts">
	import type { Track } from '$lib/types'
	import { getTrackDisplayName, getTrackDisplayArtist } from '$lib/utils'

	type Props = {
		track: Track | null
	}

	let { track }: Props = $props()
</script>

<div class="flex min-w-0 items-center gap-3">
	<!-- Album art placeholder -->
	<div class="flex h-12 w-12 flex-shrink-0 items-center justify-center rounded bg-zinc-800">
		{#if track}
			<svg class="h-6 w-6 text-zinc-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="1.5"
					d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3"
				/>
			</svg>
		{/if}
	</div>

	<!-- Track info -->
	<div class="min-w-0 flex-1">
		{#if track}
			<p class="truncate text-sm font-medium text-zinc-100">
				{getTrackDisplayName(track)}
			</p>
			<p class="truncate text-xs text-zinc-400">
				{getTrackDisplayArtist(track)}
			</p>
		{:else}
			<p class="text-sm text-zinc-500">No track selected</p>
		{/if}
	</div>
</div>
